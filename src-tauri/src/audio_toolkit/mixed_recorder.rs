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

    /// Starts recording from the configured audio sources
    #[cfg(target_os = "macos")]
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if *self.is_recording.lock().unwrap() {
            return Ok(());
        }

        let sample_callback = self.sample_callback.clone();
        let mixed_samples = self.mixed_samples.clone();

        match &self.config {
            AudioSourceConfig::MicrophoneOnly => {
                // Just use the mic recorder with sample callback
                let mut recorder = AudioRecorder::new()?;
                if let Some(cb) = &sample_callback {
                    let cb = cb.clone();
                    let samples = mixed_samples.clone();
                    recorder = recorder.with_sample_callback(move |s| {
                        samples.lock().unwrap().extend_from_slice(&s);
                        cb(s);
                    });
                }
                recorder.open(None)?;
                recorder.start()?;
                self.mic_recorder = Some(recorder);
            }
            AudioSourceConfig::SystemOnly => {
                // Just use system audio recorder
                let mut system_recorder = SystemAudioRecorder::new()?;
                system_recorder.start()?;
                self.system_recorder = Some(system_recorder);

                // Start mixer thread to receive and forward system samples
                let is_recording = self.is_recording.clone();
                *is_recording.lock().unwrap() = true;

                // We need to poll the system recorder for samples
                // Since we can't move system_recorder into thread, we'll handle differently
            }
            AudioSourceConfig::Mixed => {
                // Start both recorders
                let (mic_tx, mic_rx) = mpsc::channel::<Vec<f32>>();
                let (_sys_tx, sys_rx) = mpsc::channel::<Vec<f32>>();

                // Mic recorder
                let mut mic_recorder = AudioRecorder::new()?;
                let mic_tx_clone = mic_tx.clone();
                mic_recorder = mic_recorder.with_sample_callback(move |s| {
                    let _ = mic_tx_clone.send(s);
                });
                mic_recorder.open(None)?;
                mic_recorder.start()?;
                self.mic_recorder = Some(mic_recorder);

                // System recorder
                let mut system_recorder = SystemAudioRecorder::new()?;
                system_recorder.start()?;

                // Start mixer thread
                let is_recording = self.is_recording.clone();
                let samples_clone = mixed_samples.clone();
                let callback = sample_callback.clone();

                let handle = thread::spawn(move || {
                    let mut mic_buffer: Vec<f32> = Vec::new();
                    let mut sys_buffer: Vec<f32> = Vec::new();

                    while *is_recording.lock().unwrap() {
                        // Collect mic samples
                        while let Ok(samples) = mic_rx.try_recv() {
                            mic_buffer.extend(samples);
                        }

                        // Collect system samples
                        while let Ok(samples) = sys_rx.try_recv() {
                            sys_buffer.extend(samples);
                        }

                        // Mix available samples
                        if !mic_buffer.is_empty() || !sys_buffer.is_empty() {
                            let mix_len = mic_buffer.len().max(sys_buffer.len());
                            let mut mixed = Vec::with_capacity(mix_len);

                            for i in 0..mix_len {
                                let mic = mic_buffer.get(i).copied().unwrap_or(0.0);
                                let sys = sys_buffer.get(i).copied().unwrap_or(0.0);
                                // Mix with equal weight, clamp to [-1, 1]
                                mixed.push(((mic + sys) * 0.5).clamp(-1.0, 1.0));
                            }

                            if !mixed.is_empty() {
                                samples_clone.lock().unwrap().extend_from_slice(&mixed);
                                if let Some(ref cb) = callback {
                                    cb(mixed);
                                }
                            }

                            mic_buffer.clear();
                            sys_buffer.clear();
                        }

                        thread::sleep(Duration::from_millis(10));
                    }
                });

                self.mixer_handle = Some(handle);
                self.system_recorder = Some(system_recorder);
            }
        }

        *self.is_recording.lock().unwrap() = true;
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
        let mixed_samples = self.mixed_samples.clone();

        let mut recorder = AudioRecorder::new()?;
        if let Some(cb) = &sample_callback {
            let cb = cb.clone();
            let samples = mixed_samples.clone();
            recorder = recorder.with_sample_callback(move |s| {
                samples.lock().unwrap().extend_from_slice(&s);
                cb(s);
            });
        }
        recorder.open(None)?;
        recorder.start()?;
        self.mic_recorder = Some(recorder);
        *self.is_recording.lock().unwrap() = true;
        Ok(())
    }

    /// Stops recording and returns all collected samples
    pub fn stop(&mut self) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
        *self.is_recording.lock().unwrap() = false;

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

        let samples = std::mem::take(&mut *self.mixed_samples.lock().unwrap());
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
        *self.is_recording.lock().unwrap()
    }
}

impl Drop for MixedAudioRecorder {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
