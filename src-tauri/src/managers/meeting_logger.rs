///! Structured logging utilities for meeting operations
///!
///! This module provides helpers for logging meeting-related events with consistent
///! structure and context, making it easier to debug and analyze meeting issues.
use log::{debug, error, info, warn};

/// Log context for meeting operations
#[derive(Debug, Clone)]
pub struct MeetingLogContext {
    pub session_id: String,
    pub operation: String,
}

impl MeetingLogContext {
    pub fn new(session_id: impl Into<String>, operation: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            operation: operation.into(),
        }
    }

    /// Log start of an operation
    pub fn log_start(&self) {
        info!(
            "[MEETING] [{}] {} - Started",
            self.session_id, self.operation
        );
    }

    /// Log successful completion
    pub fn log_success(&self, message: impl AsRef<str>) {
        info!(
            "[MEETING] [{}] {} - Success: {}",
            self.session_id,
            self.operation,
            message.as_ref()
        );
    }

    /// Log operation with timing
    pub fn log_success_with_duration(&self, duration_ms: u128, message: impl AsRef<str>) {
        info!(
            "[MEETING] [{}] {} - Success ({}ms): {}",
            self.session_id,
            self.operation,
            duration_ms,
            message.as_ref()
        );
    }

    /// Log error
    pub fn log_error(&self, error: impl AsRef<str>) {
        error!(
            "[MEETING] [{}] {} - Error: {}",
            self.session_id,
            self.operation,
            error.as_ref()
        );
    }

    /// Log warning
    pub fn log_warning(&self, warning: impl AsRef<str>) {
        warn!(
            "[MEETING] [{}] {} - Warning: {}",
            self.session_id,
            self.operation,
            warning.as_ref()
        );
    }

    /// Log debug info
    pub fn log_debug(&self, message: impl AsRef<str>) {
        debug!(
            "[MEETING] [{}] {} - {}",
            self.session_id,
            self.operation,
            message.as_ref()
        );
    }

    /// Log state transition
    pub fn log_state_transition(&self, from: impl AsRef<str>, to: impl AsRef<str>) {
        info!(
            "[MEETING] [{}] {} - State transition: {} -> {}",
            self.session_id,
            self.operation,
            from.as_ref(),
            to.as_ref()
        );
    }

    /// Log timing information
    pub fn log_timing(&self, label: impl AsRef<str>, duration_ms: u128) {
        debug!(
            "[MEETING] [{}] {} - Timing: {} = {}ms",
            self.session_id,
            self.operation,
            label.as_ref(),
            duration_ms
        );
    }

    /// Log file operation
    pub fn log_file_op(&self, file_path: impl AsRef<str>, size_bytes: Option<u64>) {
        if let Some(size) = size_bytes {
            debug!(
                "[MEETING] [{}] {} - File: {} ({} bytes)",
                self.session_id,
                self.operation,
                file_path.as_ref(),
                size
            );
        } else {
            debug!(
                "[MEETING] [{}] {} - File: {}",
                self.session_id,
                self.operation,
                file_path.as_ref()
            );
        }
    }
}

/// Macro for easily creating meeting log context
#[macro_export]
macro_rules! meeting_log {
    ($session_id:expr, $operation:expr) => {
        $crate::managers::meeting_logger::MeetingLogContext::new($session_id, $operation)
    };
}

/// Log meeting event with structured data
pub fn log_meeting_event(
    session_id: impl AsRef<str>,
    event: impl AsRef<str>,
    details: impl AsRef<str>,
) {
    info!(
        "[MEETING_EVENT] session={} event={} details={}",
        session_id.as_ref(),
        event.as_ref(),
        details.as_ref()
    );
}

/// Log performance metric
pub fn log_performance_metric(
    session_id: impl AsRef<str>,
    metric: impl AsRef<str>,
    value: f64,
    unit: impl AsRef<str>,
) {
    info!(
        "[MEETING_METRIC] session={} metric={} value={} unit={}",
        session_id.as_ref(),
        metric.as_ref(),
        value,
        unit.as_ref()
    );
}

/// Log audio statistics
pub fn log_audio_stats(
    session_id: impl AsRef<str>,
    sample_rate: u32,
    channels: u16,
    samples_written: u64,
    duration_sec: f64,
) {
    info!(
        "[MEETING_AUDIO] session={} sample_rate={} channels={} samples={} duration_sec={:.2}",
        session_id.as_ref(),
        sample_rate,
        channels,
        samples_written,
        duration_sec
    );
}

/// Timer utility for measuring operation duration
pub struct MeetingTimer {
    start: std::time::Instant,
}

impl MeetingTimer {
    pub fn start() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    pub fn elapsed_sec(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}
