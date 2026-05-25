//! System audio capture using ScreenCaptureKit (macOS 13.0+)
//!
//! This module provides system audio capture functionality for Meeting Mode,
//! allowing capture of audio from all applications (YouTube, Zoom, etc.)
//! in addition to microphone input.

use std::sync::{mpsc, Arc, Mutex};

#[cfg(target_os = "macos")]
use screencapturekit::prelude::*;

use super::constants;

/// Audio source configuration for meeting recording
#[derive(Clone, Debug)]
pub enum AudioSource {
    /// Only capture microphone input (default behavior)
    MicrophoneOnly,
    /// Only capture system audio (YouTube, Zoom, etc.)
    SystemOnly,
    /// Capture both microphone and system audio, mixed together
    Mixed,
}

impl Default for AudioSource {
    fn default() -> Self {
        AudioSource::MicrophoneOnly
    }
}

/// Checks if screen recording permission is granted (macOS only).
///
/// On macOS 13.0+, ScreenCaptureKit requires screen recording permission
/// to capture system audio. This function checks if the permission is granted.
///
/// # Returns
/// - `true` if permission is granted or on non-macOS platforms
/// - `false` if permission is denied or not yet requested
#[cfg(target_os = "macos")]
pub fn has_screen_recording_permission() -> bool {
    // Try to get shareable content - this will fail if permission is not granted
    match SCShareableContent::get() {
        Ok(content) => !content.displays().is_empty(),
        Err(_) => false,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn has_screen_recording_permission() -> bool {
    false // System audio capture not supported on non-macOS
}

/// Requests screen recording permission by attempting to access ScreenCaptureKit.
///
/// On macOS, this will trigger the system permission dialog if not already granted.
/// The user will need to grant permission in System Preferences > Privacy & Security > Screen Recording.
///
/// # Returns
/// - `Ok(true)` if permission is already granted
/// - `Ok(false)` if permission dialog was shown (user needs to grant and restart)
/// - `Err` if an error occurred
#[cfg(target_os = "macos")]
pub fn request_screen_recording_permission() -> Result<bool, Box<dyn std::error::Error>> {
    // Attempting to get shareable content triggers the permission dialog
    match SCShareableContent::get() {
        Ok(content) => Ok(!content.displays().is_empty()),
        Err(e) => {
            log::warn!("Screen recording permission check failed: {:?}", e);
            Ok(false)
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn request_screen_recording_permission() -> Result<bool, Box<dyn std::error::Error>> {
    Err("System audio capture is only supported on macOS".into())
}

/// Handler for receiving system audio samples from ScreenCaptureKit
#[cfg(target_os = "macos")]
struct SystemAudioHandler {
    sample_tx: mpsc::Sender<Vec<f32>>,
    sample_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
}

#[cfg(target_os = "macos")]
impl SCStreamOutputTrait for SystemAudioHandler {
    fn did_output_sample_buffer(&self, sample: CMSampleBuffer, output_type: SCStreamOutputType) {
        if output_type != SCStreamOutputType::Audio {
            return;
        }

        // Extract audio samples from CMSampleBuffer
        if let Some(audio_buffer_list) = sample.audio_buffer_list() {
            // Iterate over buffers using iter()
            for buffer in audio_buffer_list.iter() {
                let data = buffer.data();
                if !data.is_empty() {
                    // Convert raw bytes to f32 samples
                    // ScreenCaptureKit outputs 32-bit float audio
                    let samples: Vec<f32> = data
                        .chunks_exact(4)
                        .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
                        .collect();

                    if !samples.is_empty() {
                        let _ = self.sample_tx.send(samples.clone());
                        if let Some(ref cb) = self.sample_cb {
                            cb(samples);
                        }
                    }
                }
            }
        }
    }
}

/// System audio recorder using ScreenCaptureKit
#[cfg(target_os = "macos")]
pub struct SystemAudioRecorder {
    stream: Option<SCStream>,
    sample_rx: Option<mpsc::Receiver<Vec<f32>>>,
    is_recording: Arc<Mutex<bool>>,
    sample_cb: Option<Arc<dyn Fn(Vec<f32>) + Send + Sync + 'static>>,
}

#[cfg(target_os = "macos")]
impl SystemAudioRecorder {
    /// Creates a new SystemAudioRecorder
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            stream: None,
            sample_rx: None,
            is_recording: Arc::new(Mutex::new(false)),
            sample_cb: None,
        })
    }

    /// Sets a callback for receiving audio samples
    pub fn with_sample_callback<F>(mut self, cb: F) -> Self
    where
        F: Fn(Vec<f32>) + Send + Sync + 'static,
    {
        self.sample_cb = Some(Arc::new(cb));
        self
    }

    /// Starts capturing system audio
    ///
    /// This captures all audio output from the system (apps, browser, etc.)
    /// Returns a receiver that provides audio samples as Vec<f32>
    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) {
            return Ok(()); // Already recording
        }

        // Check screen recording permission first
        if !has_screen_recording_permission() {
            request_screen_recording_permission()?;
            return Err(
                "Screen recording permission is required for system audio capture. \
                 Please grant permission in System Settings > Privacy & Security > Screen Recording"
                    .into(),
            );
        }

        // Get shareable content (displays)
        let content = SCShareableContent::get()
            .map_err(|e| format!("Failed to get shareable content: {}", e))?;

        let displays = content.displays();
        if displays.is_empty() {
            return Err("No displays found".into());
        }

        // Create filter for the primary display (we only want audio, not video)
        let display = &displays[0];
        let filter = SCContentFilter::create()
            .with_display(display)
            .with_excluding_windows(&[])
            .build();

        // Configure stream for audio-only capture
        // Width/height must be >= 2 for ScreenCaptureKit to accept the configuration
        let config = SCStreamConfiguration::new()
            .with_width(2)
            .with_height(2)
            .with_captures_audio(true)
            .with_excludes_current_process_audio(false) // Include our app's audio if any
            .with_sample_rate(constants::WHISPER_SAMPLE_RATE as i32) // 16kHz for Whisper
            .with_channel_count(1); // Mono for Whisper

        // Create sample channel
        let (sample_tx, sample_rx) = mpsc::channel();

        // Create and configure stream
        let mut stream = SCStream::new(&filter, &config);

        // Add audio output handler with optional sample callback
        let handler = SystemAudioHandler {
            sample_tx,
            sample_cb: self.sample_cb.clone(),
        };
        if stream
            .add_output_handler(handler, SCStreamOutputType::Audio)
            .is_none()
        {
            return Err("Failed to add audio output handler to system audio stream".into());
        }

        // Start capture
        stream.start_capture().map_err(|e| {
            let msg = match &e {
                screencapturekit::error::SCError::SCStreamError { code, .. } => {
                    match code {
                        screencapturekit::error::SCStreamErrorCode::UserDeclined =>
                            "Screen recording permission denied by user".to_string(),
                        screencapturekit::error::SCStreamErrorCode::FailedToStart =>
                            "System audio stream failed to start (this may require restarting the app or granting screen recording permission in System Settings > Privacy & Security > Screen Recording)".to_string(),
                        screencapturekit::error::SCStreamErrorCode::MissingEntitlements =>
                            "App is missing required entitlements for screen/audio capture".to_string(),
                        _ => format!("System audio stream error: {}", code),
                    }
                }
                _ => format!("Failed to start capture: {}", e),
            };
            msg
        })?;

        self.stream = Some(stream);
        self.sample_rx = Some(sample_rx);
        *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) = true;

        log::info!("System audio capture started");
        Ok(())
    }

    /// Stops capturing system audio
    pub fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !*self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) {
            return Ok(()); // Not recording
        }

        if let Some(stream) = self.stream.take() {
            stream
                .stop_capture()
                .map_err(|e| format!("Failed to stop capture: {:?}", e))?;
        }

        self.sample_rx = None;
        *self.is_recording.lock().unwrap_or_else(|p| p.into_inner()) = false;

        log::info!("System audio capture stopped");
        Ok(())
    }

    /// Returns whether the recorder is currently capturing
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Tries to receive available audio samples (non-blocking)
    ///
    /// Returns None if no samples are available
    pub fn try_recv_samples(&self) -> Option<Vec<f32>> {
        self.sample_rx.as_ref()?.try_recv().ok()
    }

    /// Receives audio samples (blocking)
    ///
    /// Returns None if the channel is closed
    pub fn recv_samples(&self) -> Option<Vec<f32>> {
        self.sample_rx.as_ref()?.recv().ok()
    }
}

#[cfg(target_os = "macos")]
impl Drop for SystemAudioRecorder {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Stub implementation for non-macOS platforms
#[cfg(not(target_os = "macos"))]
pub struct SystemAudioRecorder;

#[cfg(not(target_os = "macos"))]
impl SystemAudioRecorder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Err("System audio capture is only supported on macOS".into())
    }

    pub fn with_sample_callback<F>(self, _cb: F) -> Self
    where
        F: Fn(Vec<f32>) + Send + Sync + 'static,
    {
        self
    }

    pub fn start(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Err("System audio capture is only supported on macOS".into())
    }

    pub fn stop(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    pub fn is_recording(&self) -> bool {
        false
    }

    pub fn try_recv_samples(&self) -> Option<Vec<f32>> {
        None
    }

    pub fn recv_samples(&self) -> Option<Vec<f32>> {
        None
    }
}

/// Mixes two audio buffers together
///
/// If buffers have different lengths, the shorter one is padded with zeros
pub fn mix_audio(mic_samples: &[f32], system_samples: &[f32]) -> Vec<f32> {
    let max_len = mic_samples.len().max(system_samples.len());
    let mut mixed = Vec::with_capacity(max_len);

    for i in 0..max_len {
        let mic = mic_samples.get(i).copied().unwrap_or(0.0);
        let sys = system_samples.get(i).copied().unwrap_or(0.0);

        // Simple mixing with 50/50 balance, then clamp to [-1.0, 1.0]
        let sample = ((mic + sys) * 0.5).clamp(-1.0, 1.0);
        mixed.push(sample);
    }

    mixed
}

/// Resamples audio from one sample rate to another
///
/// Uses linear interpolation for simplicity
pub fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio).ceil() as usize;
    let mut resampled = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let idx_floor = src_idx.floor() as usize;
        let idx_ceil = (idx_floor + 1).min(samples.len() - 1);
        let frac = src_idx - idx_floor as f64;

        let sample = if idx_floor < samples.len() {
            let a = samples[idx_floor];
            let b = samples.get(idx_ceil).copied().unwrap_or(a);
            a + (b - a) * frac as f32
        } else {
            0.0
        };

        resampled.push(sample);
    }

    resampled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mix_audio_same_length() {
        let mic = vec![0.5, -0.5, 0.0];
        let sys = vec![0.5, 0.5, 0.0];
        let mixed = mix_audio(&mic, &sys);
        assert_eq!(mixed.len(), 3);
        assert!((mixed[0] - 0.5).abs() < 0.001); // (0.5 + 0.5) * 0.5
        assert!((mixed[1] - 0.0).abs() < 0.001); // (-0.5 + 0.5) * 0.5
        assert!((mixed[2] - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_mix_audio_different_lengths() {
        let mic = vec![0.5, -0.5];
        let sys = vec![0.5, 0.5, 1.0, 1.0];
        let mixed = mix_audio(&mic, &sys);
        assert_eq!(mixed.len(), 4);
    }

    #[test]
    fn test_resample_same_rate() {
        let samples = vec![1.0, 2.0, 3.0];
        let resampled = resample(&samples, 16000, 16000);
        assert_eq!(resampled, samples);
    }

    #[test]
    fn test_resample_upsample() {
        let samples = vec![0.0, 1.0];
        let resampled = resample(&samples, 8000, 16000);
        assert!(resampled.len() >= 3); // Should at least double
    }
}
