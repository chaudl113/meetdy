//! Data models for meeting sessions.

use crate::audio_toolkit::MixedAudioRecorder;
use serde::{Deserialize, Serialize};
use specta::Type;
use super::wav_writer::WavWriterHandle;

/// Represents the lifecycle status of a meeting session.
///
/// The state machine follows this flow:
/// - Idle -> Recording (start meeting)
/// - Recording -> Processing (stop meeting, begin transcription)
/// - Recording -> Interrupted (app closed during recording)
/// - Processing -> Completed (transcription success)
/// - Processing -> Failed (transcription failure)
/// - Failed -> Processing (retry transcription)
/// - Interrupted -> Processing (resume transcription on next launch)
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
    /// Meeting was interrupted (app closed during recording), audio preserved
    Interrupted,
}

impl Default for MeetingStatus {
    fn default() -> Self {
        MeetingStatus::Idle
    }
}

/// Audio source configuration for meeting recording
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioSourceType {
    /// Only capture microphone input (default)
    MicrophoneOnly,
    /// Only capture system audio (YouTube, Zoom, etc.) - macOS 13.0+ only
    SystemOnly,
    /// Capture both microphone and system audio mixed together - macOS 13.0+ only
    Mixed,
}

impl Default for AudioSourceType {
    fn default() -> Self {
        AudioSourceType::MicrophoneOnly
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

    /// Audio source configuration for this meeting
    pub audio_source: AudioSourceType,

    /// Relative path to the AI-generated summary file within the meetings directory
    /// e.g., "{session-id}/summary.md"
    pub summary_path: Option<String>,

    /// Template ID if this meeting was created from a template
    #[serde(default)]
    pub template_id: Option<String>,
}

impl MeetingSession {
    /// Creates a new meeting session with a unique ID and default title.
    ///
    /// The title is generated from the current timestamp in a human-readable format.
    #[allow(dead_code)]
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
            audio_source: AudioSourceType::default(),
            summary_path: None,
            template_id: None,
        }
    }

    /// Creates a new meeting session with a specified audio source.
    pub fn new_with_audio_source(
        id: String,
        title: String,
        created_at: i64,
        audio_source: AudioSourceType,
    ) -> Self {
        Self {
            id,
            title,
            created_at,
            duration: None,
            status: MeetingStatus::Idle,
            audio_path: None,
            transcript_path: None,
            error_message: None,
            audio_source,
            summary_path: None,
            template_id: None,
        }
    }

    /// Creates a new meeting session with audio source and template.
    pub fn new_with_template(
        id: String,
        title: String,
        created_at: i64,
        audio_source: AudioSourceType,
        template_id: Option<String>,
    ) -> Self {
        Self {
            id,
            title,
            created_at,
            duration: None,
            status: MeetingStatus::Idle,
            audio_path: None,
            transcript_path: None,
            error_message: None,
            audio_source,
            summary_path: None,
            template_id,
        }
    }
}

/// Internal state for the MeetingSessionManager.
///
/// This is wrapped in Arc<Mutex<>> for thread-safe access.
pub(crate) struct MeetingManagerState {
    pub current_session: Option<MeetingSession>,
    pub mixed_recorder: Option<MixedAudioRecorder>,
    pub wav_writer: Option<WavWriterHandle>,
}

impl Default for MeetingManagerState {
    fn default() -> Self {
        Self {
            current_session: None,
            mixed_recorder: None,
            wav_writer: None,
        }
    }
}
