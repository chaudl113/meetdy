//! Mixed audio recorder that captures both microphone and system audio
//!
//! This module provides a unified recorder that combines:
//! - Microphone input via cpal (AudioRecorder)
//! - System audio via ScreenCaptureKit (SystemAudioRecorder)

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use super::system_audio::SystemAudioRecorder;
use super::AudioRecorder;

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

    /// Starts recording from the configured audio sources
    #[cfg(target_os = "macos")]
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) {
            return Ok(());
        }

        let sample_callback = self.sample_callback.clone();
        let error_callback = self.error_callback.clone();
        let mixed_samples = self.mixed_samples.clone();

        match &self.config {
            AudioSourceConfig::MicrophoneOnly => {
                let mut recorder = AudioRecorder::new()?;
                if let Some(cb) = &sample_callback {
                    let cb = cb.clone();
                    let samples = mixed_samples.clone();
                    recorder = recorder.with_sample_callback(move |s| {
                        samples.lock().unwrap_or_else(|p| p.into_inner()).extend_from_slice(&s);
                        cb(s);
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
            }
            AudioSourceConfig::SystemOnly => {
                let mut system_recorder = SystemAudioRecorder::new()?;
                if let Some(cb) = &sample_callback {
                    let cb = cb.clone();
                    let samples = mixed_samples.clone();
                    system_recorder = system_recorder.with_sample_callback(move |s| {
                        samples.lock().unwrap_or_else(|p| p.into_inner()).extend_from_slice(&s);
                        cb(s);
                    });
                }
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
                let mut system_recorder = SystemAudioRecorder::new()?
                    .with_sample_callback(move |s| {
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

                    let mix_aligned = |mic: &mut VecDeque<f32>, sys: &mut VecDeque<f32>|
                        -> Vec<f32>
                    {
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
        let mixed_samples = self.mixed_samples.clone();

        let mut recorder = AudioRecorder::new()?;
        if let Some(cb) = &sample_callback {
            let cb = cb.clone();
            let samples = mixed_samples.clone();
            recorder = recorder.with_sample_callback(move |s| {
                samples.lock().unwrap_or_else(|p| p.into_inner()).extend_from_slice(&s);
                cb(s);
            });
        }
        // Wire error callback
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

        let samples = std::mem::take(&mut *self.mixed_samples.lock().unwrap_or_else(|p| p.into_inner()));
        log::info!(
            "MixedAudioRecorder stopped, collected {} samples",
            samples.len()
        );
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
