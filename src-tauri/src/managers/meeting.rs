//! Meeting session management for Meeting Mode.
//!
//! This module provides the core data structures for meeting sessions,
//! which are completely separate from the existing Quick Dictation functionality.

use anyhow::Result;
use log::{debug, info};
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::PathBuf;

/// Database migrations for meeting sessions.
/// Each migration is applied in order. The library tracks which migrations
/// have been applied using SQLite's user_version pragma.
///
/// Note: This uses a separate database file from transcription history
/// to maintain complete separation between Meeting Mode and Quick Dictation.
static MIGRATIONS: &[M] = &[M::up(
    "CREATE TABLE IF NOT EXISTS meeting_sessions (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        duration INTEGER,
        status TEXT NOT NULL DEFAULT 'idle',
        audio_path TEXT,
        transcript_path TEXT,
        error_message TEXT
    );",
)];

/// Initialize the meeting sessions database and run any pending migrations.
///
/// This function opens (or creates) the database at the specified path and
/// applies all pending migrations. It follows the same pattern as HistoryManager.
///
/// # Arguments
/// * `db_path` - Path to the SQLite database file
///
/// # Returns
/// * `Ok(())` if the database was initialized successfully
/// * `Err` if the database could not be opened or migrations failed
pub fn init_meeting_database(db_path: &PathBuf) -> Result<()> {
    info!("Initializing meeting database at {:?}", db_path);

    let mut conn = Connection::open(db_path)?;

    // Create migrations object and run to latest version
    let migrations = Migrations::new(MIGRATIONS.to_vec());

    // Validate migrations in debug builds
    #[cfg(debug_assertions)]
    migrations.validate().expect("Invalid migrations");

    // Get current version before migration
    let version_before: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    debug!("Meeting database version before migration: {}", version_before);

    // Apply any pending migrations
    migrations.to_latest(&mut conn)?;

    // Get version after migration
    let version_after: i32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if version_after > version_before {
        info!(
            "Meeting database migrated from version {} to {}",
            version_before, version_after
        );
    } else {
        debug!("Meeting database already at latest version {}", version_after);
    }

    Ok(())
}

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
    use tempfile::tempdir;

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

    #[test]
    fn test_init_meeting_database_creates_table() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_meetings.db");

        // Initialize the database
        init_meeting_database(&db_path).expect("Failed to initialize database");

        // Verify the database file was created
        assert!(db_path.exists(), "Database file should exist");

        // Open the database and check the table exists
        let conn = Connection::open(&db_path).expect("Failed to open database");
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='meeting_sessions'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query for table");

        assert!(table_exists, "meeting_sessions table should exist");

        // Verify the table has the correct columns
        let mut stmt = conn
            .prepare("PRAGMA table_info(meeting_sessions)")
            .expect("Failed to prepare statement");
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get(1))
            .expect("Failed to query columns")
            .filter_map(|r| r.ok())
            .collect();

        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"title".to_string()));
        assert!(columns.contains(&"created_at".to_string()));
        assert!(columns.contains(&"duration".to_string()));
        assert!(columns.contains(&"status".to_string()));
        assert!(columns.contains(&"audio_path".to_string()));
        assert!(columns.contains(&"transcript_path".to_string()));
        assert!(columns.contains(&"error_message".to_string()));
    }

    #[test]
    fn test_init_meeting_database_is_idempotent() {
        // Create a temporary directory for the test database
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_meetings_idempotent.db");

        // Initialize the database multiple times - should not fail
        init_meeting_database(&db_path).expect("First init should succeed");
        init_meeting_database(&db_path).expect("Second init should succeed");
        init_meeting_database(&db_path).expect("Third init should succeed");

        // Verify the database is still functional
        let conn = Connection::open(&db_path).expect("Failed to open database");
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='meeting_sessions'",
                [],
                |row| row.get(0),
            )
            .expect("Failed to query for table");

        assert!(table_exists, "meeting_sessions table should exist after multiple inits");
    }
}
