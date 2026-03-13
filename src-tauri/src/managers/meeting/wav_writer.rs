//! Thread-safe WAV file writer with timeout-based finalization.

use anyhow::Result;
use hound::WavWriter;
use log::{debug, error, info};
use std::fs::File;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Thread-safe wrapper for WavWriter that supports timeout-based finalization.
///
/// This struct solves the race condition where `Arc::try_unwrap` fails because
/// the audio callback thread still holds a reference to the WAV writer.
///
/// Key features:
/// - Uses `AtomicBool` to signal when finalization starts
/// - Callback checks `closed` flag before writing samples
/// - `finalize_with_timeout` retries with exponential backoff
pub(crate) struct WavWriterHandle {
    inner: Arc<Mutex<Option<WavWriter<File>>>>,
    closed: Arc<AtomicBool>,
}

impl WavWriterHandle {
    pub fn new(writer: WavWriter<File>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(writer))),
            closed: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn write_samples(&self, samples: &[f32]) -> Result<()> {
        // Check if closed - skip writes after finalize starts
        if self.closed.load(Ordering::Relaxed) {
            return Ok(()); // Silently ignore writes after close
        }

        if let Ok(mut guard) = self.inner.lock() {
            if let Some(writer) = guard.as_mut() {
                for sample in samples {
                    let sample_i16 = (*sample * i16::MAX as f32) as i16;
                    writer
                        .write_sample(sample_i16)
                        .map_err(|e| anyhow::anyhow!("Failed to write sample: {}", e))?;
                }
                writer
                    .flush()
                    .map_err(|e| anyhow::anyhow!("Failed to flush WAV writer: {}", e))?;
            }
        }
        Ok(())
    }

    pub fn finalize_with_timeout(&self, timeout: Duration) -> Result<()> {
        let timer = Instant::now();
        let mut retry_count = 0;

        // 1. Signal callback to stop writing
        self.closed.store(true, Ordering::SeqCst);
        debug!(
            "[WAV_FINALIZE] Closed flag set, starting finalization with timeout {:?}",
            timeout
        );

        let deadline = Instant::now() + timeout;

        // 2. Retry loop with exponential backoff
        loop {
            if let Ok(mut guard) = self.inner.try_lock() {
                if let Some(writer) = guard.take() {
                    let elapsed_ms = timer.elapsed().as_millis();
                    debug!(
                        "[WAV_FINALIZE] Lock acquired after {} retries ({elapsed_ms}ms), finalizing...",
                        retry_count
                    );

                    let result = writer
                        .finalize()
                        .map_err(|e| anyhow::anyhow!("WAV finalize failed: {}", e));

                    if result.is_ok() {
                        info!(
                            "[WAV_FINALIZE] Success - finalized in {}ms with {} retries",
                            elapsed_ms, retry_count
                        );
                    } else {
                        error!(
                            "[WAV_FINALIZE] Failed after {}ms with {} retries: {:?}",
                            elapsed_ms, retry_count, result
                        );
                    }

                    return result;
                }
                // Already finalized
                debug!("[WAV_FINALIZE] Already finalized (empty Option)");
                return Ok(());
            }

            retry_count += 1;

            if Instant::now() >= deadline {
                let elapsed_ms = timer.elapsed().as_millis();
                error!(
                    "[WAV_FINALIZE] Timeout after {:?} ({elapsed_ms}ms) with {} retries; partial audio saved",
                    timeout, retry_count
                );
                return Err(anyhow::anyhow!(
                    "Timeout finalizing WAV file after {:?}; partial audio saved",
                    timeout
                ));
            }

            // Sleep briefly before retry
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Clone for WavWriterHandle {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            closed: Arc::clone(&self.closed),
        }
    }
}
