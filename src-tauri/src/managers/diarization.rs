//! Speaker diarization manager.
//!
//! Supports two engines selected automatically by which model files are present:
//! 1. **Sortformer v2.1** (preferred) — NVIDIA end-to-end ONNX, ≤4 speakers,
//!    no clustering needed. Model file: `sortformer-diar-v2.1-int8.onnx` (~141 MB).
//! 2. **Pyannote + sherpa-onnx** (fallback) — segmentation + 3D-Speaker embedding
//!    with FastClustering. Model files: `pyannote-segmentation-int8.onnx` + embedding.

use anyhow::Result;
use log::{debug, info};
use ort::session::Session;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

use crate::managers::sortformer;

/// A single "who spoke when" interval.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct DiarizationSegment {
    pub start_sec: f32,
    pub end_sec: f32,
    pub speaker_id: usize,
}

/// Returns the diarization segment index with the greatest millisecond overlap
/// with the interval `[ts_start_ms, ts_end_ms)`.
/// Returns `None` if `diar_segments` is empty or no overlap exists.
pub fn best_speaker_for_segment(
    ts_start_ms: i64,
    ts_end_ms: i64,
    diar_segments: &[DiarizationSegment],
) -> Option<usize> {
    diar_segments
        .iter()
        .enumerate()
        .filter_map(|(i, d)| {
            let d_start = (d.start_sec * 1000.0) as i64;
            let d_end = (d.end_sec * 1000.0) as i64;
            let overlap = (ts_end_ms.min(d_end) - ts_start_ms.max(d_start)).max(0);
            if overlap > 0 { Some((i, overlap)) } else { None }
        })
        .max_by_key(|&(_, overlap)| overlap)
        .map(|(i, _)| i)
}

/// Active diarization backend.
enum DiarizationEngine {
    /// Sortformer v2.1 — preferred when model file is present.
    Sortformer { session: Session },
    /// Pyannote segmentation + 3D-Speaker embedding via sherpa-onnx.
    #[allow(dead_code)]
    Pyannote {
        segmentation_path: PathBuf,
        embedding_path: PathBuf,
    },
}

#[derive(Clone)]
pub struct SpeakerDiarizationManager {
    app_handle: AppHandle,
    /// Active engine, lazily initialised on first `process()` call.
    /// `None` means no model files were found.
    engine: Arc<Mutex<Option<DiarizationEngine>>>,
    /// Sortformer model path (if present).
    sortformer_path: Arc<Mutex<Option<PathBuf>>>,
    /// Pyannote segmentation model path (if present).
    segmentation_model_path: Arc<Mutex<Option<PathBuf>>>,
    /// Speaker embedding model path (if present).
    embedding_model_path: Arc<Mutex<Option<PathBuf>>>,
}

impl SpeakerDiarizationManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let models_dir = app_handle
            .path()
            .app_data_dir()
            .map_err(|e| anyhow::anyhow!("Failed to get app data dir: {}", e))?
            .join("models");

        let sortformer_path = models_dir.join("sortformer-diar-v2.1-int8.onnx");
        let segmentation_path = models_dir.join("pyannote-segmentation-int8.onnx");
        let embedding_path = models_dir.join("3dspeaker-eres2net.onnx");

        let sf_opt = if sortformer_path.exists() { Some(sortformer_path.clone()) } else { None };
        let seg_opt =
            if segmentation_path.exists() { Some(segmentation_path.clone()) } else { None };
        let emb_opt = if embedding_path.exists() { Some(embedding_path.clone()) } else { None };

        if sf_opt.is_some() {
            info!("Sortformer diarization model found — using Sortformer engine");
        } else if seg_opt.is_some() && emb_opt.is_some() {
            info!("Pyannote diarization models found — using sherpa-onnx engine");
        } else {
            info!("No diarization models found. Download a diarization model to enable speaker labeling.");
        }

        Ok(Self {
            app_handle: app_handle.clone(),
            engine: Arc::new(Mutex::new(None)), // loaded lazily
            sortformer_path: Arc::new(Mutex::new(sf_opt)),
            segmentation_model_path: Arc::new(Mutex::new(seg_opt)),
            embedding_model_path: Arc::new(Mutex::new(emb_opt)),
        })
    }

    pub fn is_available(&self) -> bool {
        let sf = self.sortformer_path.lock().unwrap_or_else(|p| p.into_inner());
        if sf.is_some() {
            return true;
        }
        let seg = self.segmentation_model_path.lock().unwrap_or_else(|p| p.into_inner());
        let emb = self.embedding_model_path.lock().unwrap_or_else(|p| p.into_inner());
        seg.is_some() && emb.is_some()
    }

    /// Re-check whether model files exist on disk. Call after a model download completes.
    pub fn reload_availability(&self) {
        let models_dir = self
            .app_handle
            .path()
            .app_data_dir()
            .ok()
            .map(|d| d.join("models"));

        let Some(models_dir) = models_dir else { return };

        let sortformer_path = models_dir.join("sortformer-diar-v2.1-int8.onnx");
        let segmentation_path = models_dir.join("pyannote-segmentation-int8.onnx");
        let embedding_path = models_dir.join("3dspeaker-eres2net.onnx");

        {
            let mut sf = self.sortformer_path.lock().unwrap_or_else(|p| p.into_inner());
            *sf =
                if sortformer_path.exists() { Some(sortformer_path) } else { None };
        }
        {
            let mut seg =
                self.segmentation_model_path.lock().unwrap_or_else(|p| p.into_inner());
            *seg = if segmentation_path.exists() { Some(segmentation_path) } else { None };
        }
        {
            let mut emb =
                self.embedding_model_path.lock().unwrap_or_else(|p| p.into_inner());
            *emb = if embedding_path.exists() { Some(embedding_path) } else { None };
        }

        // Invalidate cached engine so it is re-initialised on next process() call.
        *self.engine.lock().unwrap_or_else(|p| p.into_inner()) = None;

        if self.is_available() {
            info!("Diarization models reloaded — diarization now available");
        } else {
            info!("Diarization models not yet complete after reload");
        }
    }

    /// Run speaker diarization on a WAV file (16 kHz mono 16-bit PCM).
    /// Returns speaker-labelled time segments sorted by start time.
    pub fn process(&self, wav_path: &Path) -> Result<Vec<DiarizationSegment>> {
        if !self.is_available() {
            return Err(anyhow::anyhow!(
                "No diarization models found. Download sortformer-diar-v2.1-int8.onnx \
                 or pyannote-segmentation-int8.onnx + embedding model."
            ));
        }

        info!("Running speaker diarization on {:?}", wav_path);

        // Prefer Sortformer when available.
        let sf_path = self
            .sortformer_path
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();

        if let Some(ref sf_path) = sf_path {
            return self.run_sortformer(wav_path, sf_path);
        }

        self.run_pyannote(wav_path)
    }

    // ─── Sortformer engine ──────────────────────────────────────────────────

    fn run_sortformer(
        &self,
        wav_path: &Path,
        model_path: &Path,
    ) -> Result<Vec<DiarizationSegment>> {
        // Load (or reuse cached) session.
        let mut engine_guard = self.engine.lock().unwrap_or_else(|p| p.into_inner());
        if engine_guard.is_none() {
            let session = sortformer::load_session(model_path)?;
            *engine_guard = Some(DiarizationEngine::Sortformer { session });
        }

        let session = match engine_guard.as_mut() {
            Some(DiarizationEngine::Sortformer { session }) => session,
            _ => unreachable!(),
        };

        let samples = read_wav_samples(wav_path)?;
        let segments = sortformer::diarize_audio(&samples, session)?;

        info!(
            "Sortformer diarization complete: {} segments, {} unique speakers",
            segments.len(),
            {
                let mut ids: Vec<usize> = segments.iter().map(|s| s.speaker_id).collect();
                ids.sort_unstable();
                ids.dedup();
                ids.len()
            }
        );

        debug!(
            "Sortformer segments: {:?}",
            segments
                .iter()
                .map(|s| format!("Spk{}: {:.1}s-{:.1}s", s.speaker_id, s.start_sec, s.end_sec))
                .collect::<Vec<_>>()
        );

        Ok(segments)
    }

    // ─── Pyannote / sherpa-onnx engine ─────────────────────────────────────

    fn run_pyannote(&self, wav_path: &Path) -> Result<Vec<DiarizationSegment>> {
        let seg_path = self
            .segmentation_model_path
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Segmentation model not found"))?;
        let emb_path = self
            .embedding_model_path
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Embedding model not found"))?;

        let config = sherpa_onnx::OfflineSpeakerDiarizationConfig {
            segmentation: sherpa_onnx::OfflineSpeakerSegmentationModelConfig {
                pyannote: sherpa_onnx::OfflineSpeakerSegmentationPyannoteModelConfig {
                    model: Some(seg_path.to_string_lossy().to_string()),
                },
                ..Default::default()
            },
            embedding: sherpa_onnx::SpeakerEmbeddingExtractorConfig {
                model: Some(emb_path.to_string_lossy().to_string()),
                ..Default::default()
            },
            clustering: sherpa_onnx::FastClusteringConfig {
                num_clusters: -1, // auto-detect
                threshold: 0.5,
            },
            ..Default::default()
        };

        let sd = sherpa_onnx::OfflineSpeakerDiarization::create(&config)
            .ok_or_else(|| anyhow::anyhow!("Failed to create sherpa-onnx diarization engine"))?;

        let wave_path_str = wav_path.to_string_lossy().to_string();
        let wave = sherpa_onnx::Wave::read(&wave_path_str).ok_or_else(|| {
            anyhow::anyhow!("Failed to read WAV for diarization: {:?}", wav_path)
        })?;

        let result = sd
            .process(wave.samples())
            .ok_or_else(|| anyhow::anyhow!("Diarization failed: no result"))?;

        info!(
            "Pyannote diarization complete: {} speakers, {} segments",
            result.num_speakers(),
            result.num_segments()
        );

        let segments: Vec<DiarizationSegment> = result
            .sort_by_start_time()
            .iter()
            .map(|s| DiarizationSegment {
                start_sec: s.start,
                end_sec: s.end,
                speaker_id: s.speaker as usize,
            })
            .collect();

        debug!(
            "Pyannote segments: {:?}",
            segments
                .iter()
                .map(|s| format!("Spk{}: {:.1}s-{:.1}s", s.speaker_id, s.start_sec, s.end_sec))
                .collect::<Vec<_>>()
        );

        Ok(segments)
    }
}

// ─── WAV reader helper ───────────────────────────────────────────────────────

/// Read a 16 kHz mono WAV file into a `Vec<f32>` normalised to [-1, 1].
fn read_wav_samples(wav_path: &Path) -> Result<Vec<f32>> {
    use hound::WavReader;
    let mut reader =
        WavReader::open(wav_path).map_err(|e| anyhow::anyhow!("WavReader error: {}", e))?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow::anyhow!("WAV read error: {}", e))?,
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow::anyhow!("WAV read error: {}", e))?,
    };
    Ok(samples)
}
