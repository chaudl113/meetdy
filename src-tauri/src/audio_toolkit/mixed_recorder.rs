//! Mixed audio recorder that captures both microphone and system audio
//!
//! This module provides a unified recorder that combines:
//! - Microphone input via cpal (AudioRecorder)
//! - System audio via ScreenCaptureKit (SystemAudioRecorder)

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Running state for live audio-level statistics computed from the mixed
/// output buffer. Mirrors the smoothing used by `AudioRecorder` so the UI
/// behaves identically regardless of source.
struct StatsAccumulator {
    smoothed_rms: f32,
    noise_floor: f32,
    last_emit: Instant,
    interval: Duration,
}

impl StatsAccumulator {
    fn new() -> Self {
        Self {
            smoothed_rms: 0.0,
            noise_floor: 1e-4, // ~ -80 dBFS
            last_emit: Instant::now(),
            interval: Duration::from_millis(100),
        }
    }

    /// Feed a buffer of f32 samples. If the throttle interval has elapsed
    /// and a callback is provided, invokes it with the current `AudioStats`.
    fn feed(
        &mut self,
        samples: &[f32],
        cb: &Option<Arc<dyn Fn(AudioStats) + Send + Sync + 'static>>,
    ) {
        if samples.is_empty() {
            return;
        }

        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;
        for &s in samples {
            sum_sq += s * s;
            let a = s.abs();
            if a > peak {
                peak = a;
            }
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();
        // One-pole smoothing (~200ms time constant at 30ms frames).
        self.smoothed_rms = 0.85 * self.smoothed_rms + 0.15 * rms;

        // Noise floor: fast attack toward smoothed_rms when it drops,
        // slow rise when it grows.
        if self.smoothed_rms < self.noise_floor {
            self.noise_floor = 0.95 * self.noise_floor + 0.05 * self.smoothed_rms;
        } else {
            self.noise_floor = 0.999 * self.noise_floor + 0.001 * self.smoothed_rms;
        }
        let nf = self.noise_floor.max(1e-6);
        let rms_log = self.smoothed_rms.max(1e-6);
        let noise_floor_db = 20.0 * nf.log10();
        let snr_db = (20.0 * rms_log.log10() - noise_floor_db).max(0.0);

        if let Some(cb) = cb {
            if self.last_emit.elapsed() >= self.interval {
                self.last_emit = Instant::now();
                cb(AudioStats {
                    rms: self.smoothed_rms,
                    peak,
                    snr_db,
                    noise_floor_db,
                });
            }
        }
    }
}

#[cfg(target_os = "macos")]
use super::system_audio::SystemAudioRecorder;
use super::{AudioRecorder, AudioStats};

/// Configuration for audio source selection
#[derive(Clone, Debug, PartialEq)]
pub enum AudioSourceConfig {
    /// Only capture microphone input
    MicrophoneOnly,
    /// Only capture system audio (requires macOS 13.0+)
    SystemOnly,
    /// Capture both and mix them together
    Mixed,
}

impl Default for AudioSourceConfig {
    fn default() -> Self {
        AudioSourceConfig::MicrophoneOnly
    }
}

/// Mixed audio recorder that can capture mic, system, or both
pub struct MixedAudioRecorder {
    config: AudioSourceConfig,
    mic_recorder: Option<AudioRecorder>,
    #[cfg(target_os = "macos")]
    system_recorder: Option<SystemAudioRecorder>,
    mixed_samples: Arc<Mutex<Vec<f32>>>,
    sample_callback: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
    error_callback: Option<Arc<dyn Fn(String) + Send + Sync + 'static>>,
    audio_stats_callback: Option<Arc<dyn Fn(AudioStats) + Send + Sync + 'static>>,
    is_recording: Arc<Mutex<bool>>,
    mixer_handle: Option<thread::JoinHandle<()>>,
}

impl MixedAudioRecorder {
    /// Creates a new MixedAudioRecorder with the specified configuration
    pub fn new(config: AudioSourceConfig) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            config,
            mic_recorder: None,
            #[cfg(target_os = "macos")]
            system_recorder: None,
            mixed_samples: Arc::new(Mutex::new(Vec::new())),
            sample_callback: None,
            error_callback: None,
            audio_stats_callback: None,
            is_recording: Arc::new(Mutex::new(false)),
            mixer_handle: None,
        })
    }

    /// Sets a callback for receiving mixed audio samples
    pub fn with_sample_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(Vec<f32>) + Send + Sync + 'static,
    {
        self.sample_callback = Some(Arc::new(cb));
        self
    }

    /// Sets a callback for receiving audio stream errors (e.g., mic disconnect)
    pub fn with_error_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        self.error_callback = Some(Arc::new(cb));
        self
    }

    /// Sets a callback for receiving scalar audio statistics (RMS / peak /
    /// SNR / noise floor) derived from the **mixed output buffer**, so it
    /// works uniformly for mic / system / mixed configurations. Emitted at
    /// ~10 Hz.
    pub fn with_audio_stats_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(AudioStats) + Send + Sync + 'static,
    {
        self.audio_stats_callback = Some(Arc::new(cb));
        self
    }

    /// Starts recording from the configured audio sources
    #[cfg(target_os = "macos")]
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) {
            return Ok(());
        }

        let sample_callback = self.sample_callback.clone();
        let error_callback = self.error_callback.clone();
        let audio_stats_callback = self.audio_stats_callback.clone();
        let mixed_samples = self.mixed_samples.clone();

        // Shared stats accumulator: stats are computed off the **delivered
        // sample buffer** (post-VAD / post-mix), so the UI always reflects
        // what's actually being recorded regardless of source.
        let stats_acc: Arc<Mutex<StatsAccumulator>> = Arc::new(Mutex::new(StatsAccumulator::new()));

        match &self.config {
            AudioSourceConfig::MicrophoneOnly => {
                let mut recorder = AudioRecorder::new()?;
                // Use AudioRecorder's built-in stats path (computed from raw
                // pre-VAD samples, always fires regardless of recording state).
                if let Some(stats_fn) = audio_stats_callback.clone() {
                    recorder = recorder.with_audio_stats_callback(move |s| stats_fn(s));
                }
                let cb_outer = sample_callback.clone();
                let samples = mixed_samples.clone();
                recorder = recorder.with_sample_callback(move |s| {
                    samples
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .extend_from_slice(&s);
                    if let Some(cb) = &cb_outer {
                        cb(s);
                    }
                });
                if let Some(err_cb) = &error_callback {
                    let err_cb = err_cb.clone();
                    recorder = recorder.with_error_callback(move |error| {
                        err_cb(error);
                    });
                }
                recorder.open(None)?;
                recorder.start()?;
                self.mic_recorder = Some(recorder);
            }
            AudioSourceConfig::SystemOnly => {
                let mut system_recorder = SystemAudioRecorder::new()?;
                let cb_outer = sample_callback.clone();
                let samples = mixed_samples.clone();
                let stats_acc_cb = stats_acc.clone();
                let stats_cb = audio_stats_callback.clone();
                system_recorder = system_recorder.with_sample_callback(move |s| {
                    stats_acc_cb
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .feed(&s, &stats_cb);
                    samples
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .extend_from_slice(&s);
                    if let Some(cb) = &cb_outer {
                        cb(s);
                    }
                });
                system_recorder.start()?;
                self.system_recorder = Some(system_recorder);
            }
            AudioSourceConfig::Mixed => {
                // Both mic (via AudioRecorder) and system audio (via ScreenCaptureKit)
                // are configured to deliver 16kHz mono f32 samples. We mix them by
                // consuming equal-length prefixes from each stream, keeping any extra
                // samples buffered until the slower stream catches up. This preserves
                // time alignment instead of mixing by raw index across uneven chunks.
                let (mic_tx, mic_rx) = mpsc::channel::<Vec<f32>>();
                let (sys_tx, sys_rx) = mpsc::channel::<Vec<f32>>();

                // Start mic recorder
                let mut mic_recorder = AudioRecorder::new()?;
                // Attach stats to mic recorder so UI gets real-time levels from
                // raw mic audio (pre-VAD, always fires even when silent).
                if let Some(stats_fn) = audio_stats_callback.clone() {
                    mic_recorder = mic_recorder.with_audio_stats_callback(move |s| stats_fn(s));
                }
                let mic_tx_clone = mic_tx.clone();
                mic_recorder = mic_recorder.with_sample_callback(move |s| {
                    let _ = mic_tx_clone.send(s);
                });
                if let Some(err_cb) = &error_callback {
                    let err_cb = err_cb.clone();
                    mic_recorder = mic_recorder.with_error_callback(move |error| {
                        err_cb(error);
                    });
                }
                mic_recorder.open(None)?;
                mic_recorder.start()?;

                // Start system recorder with callback that sends to mixer channel
                let sys_tx_clone = sys_tx.clone();
                let mut system_recorder =
                    SystemAudioRecorder::new()?.with_sample_callback(move |s| {
                        let _ = sys_tx_clone.send(s);
                    });
                if let Err(e) = system_recorder.start() {
                    // Clean up mic recorder if system audio fails
                    let _ = mic_recorder.close();
                    return Err(e);
                }

                // Start mixer thread
                let is_recording = self.is_recording.clone();
                let samples_clone = mixed_samples.clone();
                let callback = sample_callback.clone();
                let stats_acc_thread = stats_acc.clone();
                let stats_cb_thread = audio_stats_callback.clone();
                let handle = thread::spawn(move || {
                    use std::collections::VecDeque;

                    // If one stream lags too far behind the other, drop samples from
                    // the faster stream to avoid unbounded drift if e.g. system
                    // audio is silent / not delivering. Allow up to ~2 seconds.
                    const MAX_DRIFT_SAMPLES: usize = 16_000 * 2;

                    let mut mic_buffer: VecDeque<f32> = VecDeque::new();
                    let mut sys_buffer: VecDeque<f32> = VecDeque::new();

                    let drain_rx = |rx: &mpsc::Receiver<Vec<f32>>, buf: &mut VecDeque<f32>| {
                        while let Ok(samples) = rx.try_recv() {
                            buf.extend(samples);
                        }
                    };

                    let mix_aligned =
                        |mic: &mut VecDeque<f32>, sys: &mut VecDeque<f32>| -> Vec<f32> {
                            let pair_len = mic.len().min(sys.len());
                            if pair_len == 0 {
                                return Vec::new();
                            }

                            let mut out = Vec::with_capacity(pair_len);
                            for _ in 0..pair_len {
                                let m = mic.pop_front().unwrap_or(0.0);
                                let s = sys.pop_front().unwrap_or(0.0);
                                out.push(((m + s) * 0.5).clamp(-1.0, 1.0));
                            }
                            out
                        };

                    while *is_recording.lock().unwrap_or_else(|p| p.into_inner()) {
                        drain_rx(&mic_rx, &mut mic_buffer);
                        drain_rx(&sys_rx, &mut sys_buffer);

                        // Mix the overlapping prefix in time-aligned fashion.
                        let mixed = mix_aligned(&mut mic_buffer, &mut sys_buffer);

                        if !mixed.is_empty() {
                            stats_acc_thread
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .feed(&mixed, &stats_cb_thread);
                            samples_clone
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .extend_from_slice(&mixed);
                            if let Some(ref cb) = callback {
                                cb(mixed);
                            }
                        }

                        // Prevent runaway drift if one source stalls (e.g. no system
                        // audio playing). Trim the faster buffer down to the drift
                        // limit so we resync once the other side resumes.
                        if mic_buffer.len() > MAX_DRIFT_SAMPLES {
                            let drop = mic_buffer.len() - MAX_DRIFT_SAMPLES;
                            mic_buffer.drain(..drop);
                            log::warn!(
                                "Mixer dropping {} mic samples due to drift (system audio lagging)",
                                drop
                            );
                        }
                        if sys_buffer.len() > MAX_DRIFT_SAMPLES {
                            let drop = sys_buffer.len() - MAX_DRIFT_SAMPLES;
                            sys_buffer.drain(..drop);
                            log::warn!(
                                "Mixer dropping {} system samples due to drift (mic lagging)",
                                drop
                            );
                        }

                        thread::sleep(Duration::from_millis(10));
                    }

                    // Final flush: drain anything still in the channels and mix the
                    // remaining aligned tail. Any unmatched samples on one side are
                    // emitted as-is so we don't lose audio at the end of the session.
                    drain_rx(&mic_rx, &mut mic_buffer);
                    drain_rx(&sys_rx, &mut sys_buffer);
                    let tail = mix_aligned(&mut mic_buffer, &mut sys_buffer);
                    let mut remainder: Vec<f32> = Vec::new();
                    remainder.extend(mic_buffer.drain(..));
                    remainder.extend(sys_buffer.drain(..));

                    if !tail.is_empty() {
                        samples_clone
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .extend_from_slice(&tail);
                        if let Some(ref cb) = callback {
                            cb(tail);
                        }
                    }
                    if !remainder.is_empty() {
                        // Scale by 0.5 to match the mixed gain level.
                        let scaled: Vec<f32> = remainder
                            .into_iter()
                            .map(|v| (v * 0.5).clamp(-1.0, 1.0))
                            .collect();
                        samples_clone
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .extend_from_slice(&scaled);
                        if let Some(ref cb) = callback {
                            cb(scaled);
                        }
                    }
                });

                self.mic_recorder = Some(mic_recorder);
                self.system_recorder = Some(system_recorder);
                self.mixer_handle = Some(handle);
            }
        }

        *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) = true;
        log::info!("MixedAudioRecorder started with config: {:?}", self.config);
        Ok(())
    }

    /// Non-macOS stub
    #[cfg(not(target_os = "macos"))]
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if matches!(
            self.config,
            AudioSourceConfig::SystemOnly | AudioSourceConfig::Mixed
        ) {
            return Err("System audio capture is only supported on macOS".into());
        }

        let sample_callback = self.sample_callback.clone();
        let error_callback = self.error_callback.clone();
        let audio_stats_callback = self.audio_stats_callback.clone();
        let mixed_samples = self.mixed_samples.clone();

        let mut recorder = AudioRecorder::new()?;
        if let Some(stats_fn) = audio_stats_callback.clone() {
            recorder = recorder.with_audio_stats_callback(move |s| stats_fn(s));
        }
        {
            let cb_outer = sample_callback.clone();
            let samples = mixed_samples.clone();
            recorder = recorder.with_sample_callback(move |s| {
                samples
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .extend_from_slice(&s);
                if let Some(cb) = &cb_outer {
                    cb(s);
                }
            });
        }
        if let Some(err_cb) = &error_callback {
            let err_cb = err_cb.clone();
            recorder = recorder.with_error_callback(move |error| {
                err_cb(error);
            });
        }
        recorder.open(None)?;
        recorder.start()?;
        self.mic_recorder = Some(recorder);
        *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) = true;
        Ok(())
    }

    /// Stops recording and returns all collected samples
    pub fn stop(&mut self) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        let was_recording = self.is_recording();
        *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) = false;

        // Stop mic recorder
        if let Some(ref recorder) = self.mic_recorder {
            let _ = recorder.stop();
        }

        // Stop system recorder
        #[cfg(target_os = "macos")]
        if let Some(ref mut system_recorder) = self.system_recorder {
            let _ = system_recorder.stop();
        }

        // Wait for mixer thread
        if let Some(handle) = self.mixer_handle.take() {
            let _ = handle.join();
        }

        let samples =
            std::mem::take(&mut *self.mixed_samples.lock().unwrap_or_else(|p| p.into_inner()));
        if was_recording || !samples.is_empty() {
            log::info!(
                "MixedAudioRecorder stopped, collected {} samples",
                samples.len()
            );
        }
        Ok(samples)
    }

    /// Closes the recorder and releases resources
    pub fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.stop()?;

        if let Some(ref mut recorder) = self.mic_recorder {
            let _ = recorder.close();
        }
        self.mic_recorder = None;

        #[cfg(target_os = "macos")]
        {
            self.system_recorder = None;
        }

        Ok(())
    }

    /// Returns whether recording is currently active
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl Drop for MixedAudioRecorder {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
