//! Meeting session management for Meeting Mode.
//!
//! This module provides the core data structures for meeting sessions,
//! which are completely separate from the existing Quick Dictation functionality.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Represents the lifecycle status of a meeting session.
///
/// The state machine follows this flow:
/// - Idle -> Recording (start meeting)
/// - Recording -> Processing (stop meeting, begin transcription)
/// - Processing -> Completed (transcription success)
/// - Processing -> Failed (transcription failure)
/// - Failed -> Processing (retry transcription)
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MeetingStatus {
    /// No active meeting session
    Idle,
    /// Meeting is currently being recorded
    Recording,
    /// Recording stopped, transcription in progress
    Processing,
    /// Meeting completed successfully with transcript
    Completed,
    /// Meeting failed (e.g., transcription error), audio preserved
    Failed,
}

impl Default for MeetingStatus {
    fn default() -> Self {
        MeetingStatus::Idle
    }
}

/// Represents a meeting session with its metadata and file references.
///
/// Each meeting session has a unique ID and is stored in a dedicated folder
/// under the app's data directory: `{app_data}/meetings/{session-id}/`
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingSession {
    /// Unique identifier for the session (UUID format)
    pub id: String,

    /// User-editable title, defaults to timestamp format like
    /// "Meeting - January 15, 2025 3:30 PM"
    pub title: String,

    /// Unix timestamp (seconds) when the meeting was created/started
    pub created_at: i64,

    /// Duration of the recording in seconds (set after recording stops)
    pub duration: Option<i64>,

    /// Current status of the meeting session
    pub status: MeetingStatus,

    /// Relative path to the audio file within the meetings directory
    /// e.g., "{session-id}/audio.wav"
    pub audio_path: Option<String>,

    /// Relative path to the transcript file within the meetings directory
    /// e.g., "{session-id}/transcript.txt"
    pub transcript_path: Option<String>,

    /// Error message if the meeting failed
    pub error_message: Option<String>,
}

impl MeetingSession {
    /// Creates a new meeting session with a unique ID and default title.
    ///
    /// The title is generated from the current timestamp in a human-readable format.
    pub fn new(id: String, title: String, created_at: i64) -> Self {
        Self {
            id,
            title,
            created_at,
            duration: None,
            status: MeetingStatus::Idle,
            audio_path: None,
            transcript_path: None,
            error_message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meeting_status_default() {
        let status = MeetingStatus::default();
        assert_eq!(status, MeetingStatus::Idle);
    }

    #[test]
    fn test_meeting_session_new() {
        let session = MeetingSession::new(
            "test-uuid-123".to_string(),
            "Meeting - January 15, 2025 3:30 PM".to_string(),
            1705340400,
        );

        assert_eq!(session.id, "test-uuid-123");
        assert_eq!(session.title, "Meeting - January 15, 2025 3:30 PM");
        assert_eq!(session.created_at, 1705340400);
        assert_eq!(session.duration, None);
        assert_eq!(session.status, MeetingStatus::Idle);
        assert_eq!(session.audio_path, None);
        assert_eq!(session.transcript_path, None);
        assert_eq!(session.error_message, None);
    }

    #[test]
    fn test_meeting_status_serialization() {
        // Test that MeetingStatus serializes to snake_case as expected
        let status = MeetingStatus::Recording;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"recording\"");

        let status = MeetingStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");
    }

    #[test]
    fn test_meeting_session_serialization() {
        let session = MeetingSession::new(
            "uuid-abc".to_string(),
            "Test Meeting".to_string(),
            1705340400,
        );

        let json = serde_json::to_string(&session).unwrap();
        assert!(json.contains("\"id\":\"uuid-abc\""));
        assert!(json.contains("\"status\":\"idle\""));
    }
}
