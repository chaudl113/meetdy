//! Data models for meeting sessions.

use super::wav_writer::WavWriterHandle;
use crate::audio_toolkit::MixedAudioRecorder;
use serde::{Deserialize, Serialize};
use specta::Type;

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

    #[serde(default = "default_stt_engine")]
    pub stt_engine: String,

    #[serde(default)]
    pub funasr_base_url: Option<String>,

    #[serde(default)]
    pub funasr_model: Option<String>,

    #[serde(default)]
    pub transcription_language: Option<String>,
}

fn default_stt_engine() -> String {
    "whisper".to_string()
}

/// A short, timestamped note attached to a meeting session.
///
/// Notes are captured during a recording (via the "Add Note" button) or
/// after the fact in the Notes tab. They are stored in their own table
/// keyed by `session_id` with `ON DELETE CASCADE` so removing a session
/// also removes its notes.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingNote {
    /// Unique identifier (UUID).
    pub id: String,
    /// Session this note belongs to.
    pub session_id: String,
    /// Offset from the start of the recording, in seconds.
    /// May be `0` for notes added before recording starts or after it stops.
    pub timestamp_seconds: i64,
    /// Free-form note content (plain text, may contain newlines).
    pub content: String,
    /// Optional author label. Reserved for future multi-user support;
    /// currently always `None` from the UI.
    pub author: Option<String>,
    /// Unix timestamp (seconds) when the note was created.
    pub created_at: i64,
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
            stt_engine: default_stt_engine(),
            funasr_base_url: None,
            funasr_model: None,
            transcription_language: None,
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
            stt_engine: default_stt_engine(),
            funasr_base_url: None,
            funasr_model: None,
            transcription_language: None,
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
            stt_engine: default_stt_engine(),
            funasr_base_url: None,
            funasr_model: None,
            transcription_language: None,
        }
    }
}

/// Status of an action item.
#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionItemStatus {
    Todo,
    InProgress,
    Done,
    Blocked,
}

impl Default for ActionItemStatus {
    fn default() -> Self {
        ActionItemStatus::Todo
    }
}

/// A task extracted from (or added to) a meeting.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct ActionItem {
    pub id: String,
    pub session_id: String,
    pub task: String,
    pub assignee: Option<String>,
    /// Free-form due date string (e.g. "Cuối tháng", "Tuần này", "2025-06-01").
    /// Kept as text so AI-extracted relative dates are preserved.
    pub due_date: Option<String>,
    pub status: ActionItemStatus,
    pub sort_order: i64,
    pub created_at: i64,
}

/// A discussion bullet/point. `category` groups multiple points under a heading.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct KeyPoint {
    pub id: String,
    pub session_id: String,
    pub category: Option<String>,
    pub content: String,
    pub sort_order: i64,
    pub created_at: i64,
}

/// A person attached to a meeting (extracted from transcript or added manually).
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Participant {
    pub id: String,
    pub session_id: String,
    pub name: String,
    /// Free-form role/team label, e.g. "Marketing", "Sales".
    pub role: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    /// Index into the SPEAKER_COLORS palette (0-7). -1 = unassigned.
    #[serde(default = "default_color_index")]
    pub color_index: i64,
}

fn default_color_index() -> i64 {
    -1
}

/// A structured transcript segment with optional speaker attribution.
/// Stored in the `transcript_segments` table.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct TranscriptSegment {
    pub id: String,
    pub meeting_id: String,
    /// Start time in milliseconds from the beginning of the recording.
    pub start_ms: i64,
    /// End time in milliseconds from the beginning of the recording.
    pub end_ms: i64,
    /// The transcribed text for this segment.
    pub text: String,
    /// Optional participant ID of the speaker. None = unassigned.
    pub speaker_id: Option<String>,
    /// Monotonically increasing sequence number for ordering.
    pub sequence: i64,
    /// Unix timestamp (ms) when the segment was created.
    pub created_at: i64,
}

/// A tag for filtering/organizing meetings.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct Tag {
    pub id: String,
    pub session_id: String,
    pub label: String,
    /// Optional Tailwind-compatible color token (e.g. "blue", "amber").
    pub color: Option<String>,
    pub created_at: i64,
}

/// Bundle of AI-extracted structured data for a meeting.
/// Used as the return type of `extract_meeting_insights`.
#[derive(Clone, Debug, Serialize, Deserialize, Type, Default)]
pub struct MeetingInsights {
    pub key_points: Vec<KeyPoint>,
    pub action_items: Vec<ActionItem>,
    pub participants: Vec<Participant>,
    pub tags: Vec<Tag>,
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
