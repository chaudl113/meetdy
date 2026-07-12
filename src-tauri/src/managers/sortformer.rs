//! Sortformer v2.1 speaker diarization engine.
//!
//! Implements offline diarization using NVIDIA's Sortformer model exported to ONNX.
//! The model is end-to-end: raw audio → per-frame speaker activity (≤4 speakers).
//! No separate clustering step is needed.
//!
//! Reference: <https://github.com/altunenes/parakeet-rs>
//! Model: christopherthompson81/sortformer_parakeet_onnx (INT8, 141 MB)

use anyhow::{anyhow, Result};
use log::info;
use ndarray::{Array1, Array2, Array3, Axis};
use ort::{
    session::{Session, SessionOutputs},
    value::Tensor,
};
use realfft::RealFftPlanner;
use std::f32::consts::PI;
use std::path::Path;

use crate::managers::diarization::DiarizationSegment;

// ─── Model hyperparameters (read from ONNX metadata at load-time ideally,
//     but these defaults match the published altunenes export) ────────────────
const CHUNK_LEN: usize = 124; // subsampled frames per chunk
const FIFO_LEN: usize = 124; // FIFO context length in subsampled frames
const SPKCACHE_LEN: usize = 188; // speaker cache length in subsampled frames
const EMB_DIM: usize = 512;
const N_MELS: usize = 128;
const SUBSAMPLING: usize = 8; // FastConformer 8× downsampling
const MEL_FRAMES_PER_CHUNK: usize = CHUNK_LEN * SUBSAMPLING; // 992 mel frames

// ─── Audio preprocessing parameters ────────────────────────────────────────
const SAMPLE_RATE: usize = 16_000;
const N_FFT: usize = 512;
const WIN_LENGTH: usize = 400;
const HOP_LENGTH: usize = 160;
const F_MIN: f32 = 0.0;
const F_MAX: f32 = 8000.0;
const LOG_GUARD: f32 = 5.960_464e-8; // matches NeMo default

// ─── Post-processing thresholds (CallHome config) ───────────────────────────
const ONSET: f32 = 0.641;
const OFFSET: f32 = 0.561;
const PAD_ONSET_FRAMES: usize = 3; // 0.229s / (HOP_LENGTH/SR) ≈ 3 output frames
const PAD_OFFSET_FRAMES: usize = 1;
const MIN_DUR_ON_FRAMES: usize = 6; // 0.511s
const MIN_DUR_OFF_FRAMES: usize = 4; // 0.296s
const MEDIAN_FILTER: usize = 11;

/// Offline Sortformer diarization over a complete audio buffer.
///
/// Returns `DiarizationSegment`s sorted by start time (same format as the
/// pyannote/sherpa-onnx path so callers are interchangeable).
pub fn diarize_audio(samples: &[f32], session: &mut Session) -> Result<Vec<DiarizationSegment>> {
    // ── 1. Log-mel spectrogram ──────────────────────────────────────────────
    let mel_frames = log_mel_spectrogram(samples)?;
    // mel_frames: [T_mel, N_MELS]

    // ── 2. Chunk-wise inference ─────────────────────────────────────────────
    let n_mel_total = mel_frames.nrows();
    let n_chunks = (n_mel_total + MEL_FRAMES_PER_CHUNK - 1) / MEL_FRAMES_PER_CHUNK;

    // Rolling state tensors.
    let mut spkcache: Array3<f32> = Array3::zeros([1, 0, EMB_DIM]);
    let mut fifo: Array3<f32> = Array3::zeros([1, 0, EMB_DIM]);
    let mut all_preds: Vec<f32> = Vec::new(); // flat [T_out, 4]
    let mut total_valid_frames: usize = 0;

    for c in 0..n_chunks {
        let start_mel = c * MEL_FRAMES_PER_CHUNK;
        let end_mel = ((c + 1) * MEL_FRAMES_PER_CHUNK).min(n_mel_total);
        let chunk_mel_len = end_mel - start_mel;
        // Subsampled length for this chunk.
        let chunk_sub_len = (chunk_mel_len + SUBSAMPLING - 1) / SUBSAMPLING;

        // Build chunk tensor: pad to MEL_FRAMES_PER_CHUNK if last chunk.
        let mut chunk_data = vec![0.0f32; MEL_FRAMES_PER_CHUNK * N_MELS];
        for (row, mel_row) in mel_frames
            .rows()
            .into_iter()
            .skip(start_mel)
            .take(chunk_mel_len)
            .enumerate()
        {
            let dst = &mut chunk_data[row * N_MELS..(row + 1) * N_MELS];
            dst.copy_from_slice(mel_row.as_slice().unwrap());
        }
        let chunk_arr =
            Array3::from_shape_vec([1, MEL_FRAMES_PER_CHUNK, N_MELS], chunk_data)?;
        let chunk_lengths = ndarray::array![[chunk_mel_len as i64]];

        let spk_len = spkcache.shape()[1] as i64;
        let fifo_len_val = fifo.shape()[1] as i64;

        let chunk_t = Tensor::from_array(chunk_arr)?;
        let chunk_lengths_t =
            Tensor::from_array(Array1::from_vec(vec![chunk_mel_len as i64]))?;
        let spkcache_t = Tensor::from_array(spkcache.clone())?;
        let spkcache_lengths_t =
            Tensor::from_array(Array1::from_vec(vec![spk_len]))?;
        let fifo_t = Tensor::from_array(fifo.clone())?;
        let fifo_lengths_t =
            Tensor::from_array(Array1::from_vec(vec![fifo_len_val]))?;

        let outputs: SessionOutputs = session.run(ort::inputs![
            "chunk"              => chunk_t,
            "chunk_lengths"      => chunk_lengths_t,
            "spkcache"           => spkcache_t,
            "spkcache_lengths"   => spkcache_lengths_t,
            "fifo"               => fifo_t,
            "fifo_lengths"       => fifo_lengths_t
        ])?;

        // Extract predictions: [1, spkcache_len + fifo_len + chunk_sub, 4]
        let (preds_shape, preds_data) = outputs["spkcache_fifo_chunk_preds"]
            .try_extract_tensor::<f32>()?;
        // preds_shape = [1, total_ctx + chunk_sub, 4]
        let total_t = preds_shape[1] as usize;
        let ctx = spk_len as usize + fifo_len_val as usize;
        let pred_start = ctx.min(total_t);
        let pred_end = (pred_start + chunk_sub_len).min(total_t);
        // preds_data is flat row-major: index = t * 4 + spk (batch=0 assumed)
        for t in pred_start..pred_end {
            for spk in 0..4 {
                all_preds.push(preds_data[t * 4 + spk]);
            }
        }
        total_valid_frames += pred_end - pred_start;

        // Update FIFO with new pre-encoder embeddings.
        let (embs_shape, embs_data) = outputs["chunk_pre_encode_embs"]
            .try_extract_tensor::<f32>()?;
        let embs_t = embs_shape[1] as usize; // subsampled frames for this chunk
        // Rebuild as ndarray [embs_t, EMB_DIM] for concatenation.
        let embs_2d = Array2::from_shape_vec(
            [embs_t, EMB_DIM],
            embs_data[..embs_t * EMB_DIM].to_vec(),
        )?;

        // Concatenate to fifo and trim to FIFO_LEN.
        let new_fifo = ndarray::concatenate(
            Axis(0),
            &[fifo.index_axis(Axis(0), 0), embs_2d.view()],
        )?;
        // Overflow: move excess from fifo head into spkcache.
        if new_fifo.nrows() > FIFO_LEN {
            let overflow = new_fifo.nrows() - FIFO_LEN;
            let overflow_rows = new_fifo.slice(ndarray::s![..overflow, ..]);
            let new_spkcache = ndarray::concatenate(
                Axis(0),
                &[spkcache.index_axis(Axis(0), 0), overflow_rows],
            )?;
            // Trim spkcache to SPKCACHE_LEN.
            let trimmed_spk = if new_spkcache.nrows() > SPKCACHE_LEN {
                new_spkcache.slice(ndarray::s![new_spkcache.nrows() - SPKCACHE_LEN.., ..]).to_owned()
            } else {
                new_spkcache.to_owned()
            };
            spkcache = trimmed_spk.insert_axis(Axis(0));
            let kept_fifo = new_fifo.slice(ndarray::s![overflow.., ..]).to_owned();
            fifo = kept_fifo.insert_axis(Axis(0));
        } else {
            fifo = new_fifo.insert_axis(Axis(0));
        }
    }

    // ── 3. Build predictions array [T, 4] ───────────────────────────────────
    if total_valid_frames == 0 {
        return Ok(vec![]);
    }
    let preds_2d = Array2::from_shape_vec(
        [total_valid_frames, 4],
        all_preds[..total_valid_frames * 4].to_vec(),
    )?;

    // ── 4. Per-speaker post-processing → DiarizationSegment ─────────────────
    let frame_dur_sec =
        (HOP_LENGTH * SUBSAMPLING) as f32 / SAMPLE_RATE as f32; // ~0.08 s / output frame

    let mut segments: Vec<DiarizationSegment> = Vec::new();
    for spk in 0..4 {
        let raw: Vec<f32> = (0..total_valid_frames)
            .map(|t| preds_2d[[t, spk]])
            .collect();
        let smoothed = median_filter(&raw, MEDIAN_FILTER);
        let spk_segs = binarize_speaker(
            &smoothed,
            frame_dur_sec,
            spk,
            ONSET,
            OFFSET,
            PAD_ONSET_FRAMES,
            PAD_OFFSET_FRAMES,
            MIN_DUR_ON_FRAMES,
            MIN_DUR_OFF_FRAMES,
        );
        segments.extend(spk_segs);
    }

    segments.sort_by(|a, b| a.start_sec.partial_cmp(&b.start_sec).unwrap());
    Ok(segments)
}

// ─── Mel spectrogram ────────────────────────────────────────────────────────

fn log_mel_spectrogram(samples: &[f32]) -> Result<Array2<f32>> {
    // Pre-emphasis filter: y[n] = x[n] - 0.97 * x[n-1]
    let mut preemph = Vec::with_capacity(samples.len());
    preemph.push(samples[0]);
    for i in 1..samples.len() {
        preemph.push(samples[i] - 0.97 * samples[i - 1]);
    }

    // Hann window (size WIN_LENGTH, zero-padded to N_FFT).
    let hann: Vec<f32> = (0..WIN_LENGTH)
        .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (WIN_LENGTH - 1) as f32).cos()))
        .collect();

    // STFT: center-padded, step = HOP_LENGTH.
    let pad = N_FFT / 2;
    let mut padded = vec![0.0f32; preemph.len() + 2 * pad];
    padded[pad..pad + preemph.len()].copy_from_slice(&preemph);

    let n_frames =
        (padded.len().saturating_sub(N_FFT)) / HOP_LENGTH + 1;
    let n_bins = N_FFT / 2 + 1;

    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N_FFT);
    let mut spectrum = fft.make_output_vec();
    let mut power = vec![vec![0.0f32; n_bins]; n_frames];

    for frame_idx in 0..n_frames {
        let offset = frame_idx * HOP_LENGTH;
        let mut buf = vec![0.0f32; N_FFT];
        // Copy WIN_LENGTH samples and apply window; rest stays zero (zero-padding).
        let copy_len = WIN_LENGTH.min(padded.len().saturating_sub(offset));
        for i in 0..copy_len {
            buf[i] = padded[offset + i] * hann[i];
        }
        fft.process(&mut buf, &mut spectrum)
            .map_err(|e| anyhow!("FFT error: {:?}", e))?;
        for b in 0..n_bins {
            let re = spectrum[b].re;
            let im = spectrum[b].im;
            power[frame_idx][b] = re * re + im * im;
        }
    }

    // Mel filterbank: 128 bands.
    let mel_fb = build_mel_filterbank(N_FFT, SAMPLE_RATE, N_MELS, F_MIN, F_MAX);

    let mut mel_frames = Array2::<f32>::zeros([n_frames, N_MELS]);
    for t in 0..n_frames {
        for m in 0..N_MELS {
            let mut val = 0.0f32;
            for b in 0..n_bins {
                val += power[t][b] * mel_fb[m][b];
            }
            mel_frames[[t, m]] = (val + LOG_GUARD).ln();
        }
    }

    Ok(mel_frames)
}

fn hz_to_mel(hz: f32) -> f32 {
    // Slaney (linear + log), not HTK.
    const F_SP: f32 = 200.0 / 3.0;
    const MIN_LOG_HZ: f32 = 1000.0;
    const LOGSTEP: f32 = 0.068_751_777; // 6.4.ln() / 27.0
    if hz < MIN_LOG_HZ {
        hz / F_SP
    } else {
        15.0 + (hz / MIN_LOG_HZ).ln() / LOGSTEP
    }
}

fn mel_to_hz(mel: f32) -> f32 {
    const F_SP: f32 = 200.0 / 3.0;
    const MIN_LOG_MEL: f32 = 15.0;
    const MIN_LOG_HZ: f32 = 1000.0;
    const LOGSTEP: f32 = 0.068_751_777;
    if mel < MIN_LOG_MEL {
        mel * F_SP
    } else {
        MIN_LOG_HZ * ((mel - MIN_LOG_MEL) * LOGSTEP).exp()
    }
}

fn build_mel_filterbank(
    n_fft: usize,
    sample_rate: usize,
    n_mels: usize,
    f_min: f32,
    f_max: f32,
) -> Vec<Vec<f32>> {
    let n_bins = n_fft / 2 + 1;
    let mel_min = hz_to_mel(f_min);
    let mel_max = hz_to_mel(f_max);

    // n_mels + 2 equally spaced mel points.
    let mel_points: Vec<f32> = (0..=n_mels + 1)
        .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (n_mels + 1) as f32)
        .collect();
    let hz_points: Vec<f32> = mel_points.iter().map(|&m| mel_to_hz(m)).collect();
    // Convert Hz → FFT bin index.
    let bin_points: Vec<usize> = hz_points
        .iter()
        .map(|&hz| ((n_fft + 1) as f32 * hz / sample_rate as f32).floor() as usize)
        .collect();

    let mut fb = vec![vec![0.0f32; n_bins]; n_mels];
    for m in 0..n_mels {
        let left = bin_points[m];
        let center = bin_points[m + 1];
        let right = bin_points[m + 2];
        // Rising slope.
        for k in left..center {
            if k < n_bins && center > left {
                fb[m][k] = (k - left) as f32 / (center - left) as f32;
            }
        }
        // Falling slope.
        for k in center..right {
            if k < n_bins && right > center {
                fb[m][k] = (right - k) as f32 / (right - center) as f32;
            }
        }
    }

    // Slaney normalisation: divide by bandwidth in Hz so each filter sums to 1.
    for m in 0..n_mels {
        let left_hz = hz_points[m];
        let right_hz = hz_points[m + 2];
        let bw = right_hz - left_hz;
        if bw > 0.0 {
            for k in 0..n_bins {
                fb[m][k] *= 2.0 / bw;
            }
        }
    }

    fb
}

// ─── Post-processing ─────────────────────────────────────────────────────────

fn median_filter(signal: &[f32], window: usize) -> Vec<f32> {
    let half = window / 2;
    let n = signal.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(n);
        let mut window_vals: Vec<f32> = signal[lo..hi].to_vec();
        window_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
        out.push(window_vals[window_vals.len() / 2]);
    }
    out
}

fn binarize_speaker(
    probs: &[f32],
    frame_dur_sec: f32,
    speaker_id: usize,
    onset: f32,
    offset: f32,
    pad_onset: usize,
    pad_offset: usize,
    min_dur_on: usize,
    min_dur_off: usize,
) -> Vec<DiarizationSegment> {
    let n = probs.len();
    // Hysteresis binarization.
    let mut active = vec![false; n];
    let mut in_speech = false;
    for i in 0..n {
        if !in_speech && probs[i] >= onset {
            in_speech = true;
        } else if in_speech && probs[i] < offset {
            in_speech = false;
        }
        active[i] = in_speech;
    }

    // Pad onset/offset.
    if pad_onset > 0 || pad_offset > 0 {
        let original = active.clone();
        for i in 0..n {
            if original[i] {
                let lo = i.saturating_sub(pad_onset);
                let hi = (i + pad_offset + 1).min(n);
                for j in lo..hi {
                    active[j] = true;
                }
            }
        }
    }

    // Collect raw on/off intervals.
    let mut intervals: Vec<(usize, usize)> = Vec::new();
    let mut seg_start: Option<usize> = None;
    for i in 0..n {
        match (seg_start, active[i]) {
            (None, true) => seg_start = Some(i),
            (Some(s), false) => {
                intervals.push((s, i));
                seg_start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = seg_start {
        intervals.push((s, n));
    }

    // Remove short on segments.
    intervals.retain(|(s, e)| e - s >= min_dur_on);

    // Merge short off gaps.
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for &(s, e) in &intervals {
        if let Some(last) = merged.last_mut() {
            if s - last.1 < min_dur_off {
                last.1 = e;
                continue;
            }
        }
        merged.push((s, e));
    }

    // Remove short on segments again after merging.
    merged.retain(|(s, e)| e - s >= min_dur_on);

    merged
        .into_iter()
        .map(|(s, e)| DiarizationSegment {
            start_sec: s as f32 * frame_dur_sec,
            end_sec: e as f32 * frame_dur_sec,
            speaker_id,
        })
        .collect()
}

// ─── Session loader ──────────────────────────────────────────────────────────

/// Initialise the `ort` API using the ONNX Runtime already linked into the
/// binary via sherpa-onnx's static `libonnxruntime.a`.
///
/// Must be called once before any `Session` is created.
/// Safe to call multiple times — subsequent calls are no-ops.
fn init_ort_api() -> Result<()> {
    use ort::sys as ort_sys;
    // SAFETY: OrtGetApiBase is provided by sherpa-onnx's statically linked
    // libonnxruntime.a. The symbol is present in the final binary.
    let api = unsafe {
        let base = ort_sys::OrtGetApiBase();
        if base.is_null() {
            return Err(anyhow!("OrtGetApiBase returned null"));
        }
        let api_ptr = ((*base).GetApi)(ort_sys::ORT_API_VERSION);
        if api_ptr.is_null() {
            return Err(anyhow!(
                "OrtApi v{} not supported by the linked ONNX Runtime",
                ort_sys::ORT_API_VERSION
            ));
        }
        *api_ptr
    };
    ort::set_api(api);
    Ok(())
}

pub fn load_session(model_path: &Path) -> Result<Session> {
    init_ort_api()?;
    let session = Session::builder()?
        .with_intra_threads(2)?
        .commit_from_file(model_path)
        .map_err(|e| anyhow!("Failed to load Sortformer ONNX: {}", e))?;
    info!("Sortformer session loaded from {:?}", model_path);
    Ok(session)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mel_filterbank_shape() {
        let fb = build_mel_filterbank(512, 16_000, 128, 0.0, 8000.0);
        assert_eq!(fb.len(), 128);
        assert_eq!(fb[0].len(), 257); // N_FFT/2 + 1
    }

    #[test]
    fn test_median_filter_identity_on_constant() {
        let signal = vec![0.5f32; 20];
        let out = median_filter(&signal, 11);
        assert_eq!(out.len(), 20);
        for v in &out {
            assert!((v - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn test_binarize_empty() {
        let segs = binarize_speaker(&[], 0.08, 0, 0.641, 0.561, 3, 1, 6, 4);
        assert!(segs.is_empty());
    }

    #[test]
    fn test_binarize_single_spike() {
        // Single frame spike (1 frame) with NO padding → stays 1 frame < min_dur_on=6 → filtered.
        let mut probs = vec![0.0f32; 20];
        probs[10] = 0.9;
        let segs = binarize_speaker(&probs, 0.08, 0, 0.641, 0.561, 0, 0, 6, 4);
        assert!(segs.is_empty(), "short spike should be filtered");
    }

    #[test]
    fn test_binarize_sustained_speech() {
        // 10 frames of high probability → one segment.
        let mut probs = vec![0.0f32; 30];
        for i in 5..15 {
            probs[i] = 0.9;
        }
        let segs = binarize_speaker(&probs, 0.08, 1, 0.641, 0.561, 0, 0, 1, 100);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].speaker_id, 1);
        assert!((segs[0].start_sec - 5.0 * 0.08).abs() < 0.01);
    }

    #[test]
    fn test_log_mel_short_audio() {
        // 1 second of silence.
        let samples = vec![0.0f32; 16_000];
        let result = log_mel_spectrogram(&samples);
        assert!(result.is_ok());
        let mel = result.unwrap();
        assert_eq!(mel.ncols(), 128);
        assert!(mel.nrows() > 0);
    }
}
