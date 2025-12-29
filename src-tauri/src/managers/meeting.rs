//! Meeting session management for Meeting Mode.
//!
//! This module provides the core data structures and manager for meeting sessions,
//! which are completely separate from the existing Quick Dictation functionality.

use anyhow::Result;
use chrono::{DateTime, Local};
use hound::{WavSpec, WavWriter};
use log::{debug, error, info};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use uuid::Uuid;

// Import AudioRecorder from audio_toolkit for recording functionality
use crate::audio_toolkit::AudioRecorder;

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
    debug!(
        "Meeting database version before migration: {}",
        version_before
    );

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
        debug!(
            "Meeting database already at latest version {}",
            version_after
        );
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

/// Internal state for the MeetingSessionManager.
///
/// This is wrapped in Arc<Mutex<>> for thread-safe access.
#[derive(Debug)]
struct MeetingManagerState {
    /// The currently active meeting session, if any
    current_session: Option<MeetingSession>,
    /// Audio recorder for capturing meeting audio
    recorder: Option<AudioRecorder>,
    /// WAV file writer for incremental audio writing
    wav_writer: Option<WavWriter<File>>,
}

impl Default for MeetingManagerState {
    fn default() -> Self {
        Self {
            current_session: None,
            recorder: None,
            wav_writer: None,
        }
    }
}

/// Manager for meeting sessions.
///
/// Handles the lifecycle of meeting sessions including:
/// - Session creation and persistence
/// - Audio recording coordination (future phases)
/// - Transcription triggering (future phases)
/// - File storage management
///
/// This manager follows the same patterns as `AudioRecordingManager` and `HistoryManager`:
/// - Uses `Arc<Mutex<>>` for thread-safe state management
/// - Implements `Clone` for sharing across Tauri state
/// - Stores `AppHandle` for accessing app resources
#[derive(Clone)]
pub struct MeetingSessionManager {
    /// Thread-safe internal state
    state: Arc<Mutex<MeetingManagerState>>,
    /// Tauri app handle for accessing paths and emitting events
    app_handle: AppHandle,
    /// Directory for storing meeting session folders
    /// e.g., `{app_data}/meetings/`
    meetings_dir: PathBuf,
    /// Path to the SQLite database for meeting sessions
    /// e.g., `{app_data}/meetings.db`
    db_path: PathBuf,
}

impl MeetingSessionManager {
    /// Creates a new MeetingSessionManager.
    ///
    /// This constructor:
    /// 1. Resolves the app data directory from the AppHandle
    /// 2. Creates the meetings directory if it doesn't exist
    /// 3. Initializes the SQLite database and runs migrations
    ///
    /// # Arguments
    /// * `app_handle` - Reference to the Tauri AppHandle
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully initialized manager
    /// * `Err` - Failed to create directories or initialize database
    ///
    /// # Example
    /// ```ignore
    /// let manager = MeetingSessionManager::new(&app_handle)?;
    /// ```
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        // Get the app data directory from the Tauri path resolver
        let app_data_dir = app_handle.path().app_data_dir()?;

        // Set up the meetings directory under app data
        let meetings_dir = app_data_dir.join("meetings");
        let db_path = app_data_dir.join("meetings.db");

        // Ensure the meetings directory exists
        if !meetings_dir.exists() {
            fs::create_dir_all(&meetings_dir)?;
            info!("Created meetings directory: {:?}", meetings_dir);
        }

        // Initialize the database and run migrations
        init_meeting_database(&db_path)?;

        let manager = Self {
            state: Arc::new(Mutex::new(MeetingManagerState::default())),
            app_handle: app_handle.clone(),
            meetings_dir,
            db_path,
        };

        info!("MeetingSessionManager initialized successfully");
        debug!(
            "Meetings directory: {:?}, Database: {:?}",
            manager.meetings_dir, manager.db_path
        );

        Ok(manager)
    }

    /// Returns the path to the meetings directory.
    pub fn get_meetings_dir(&self) -> &PathBuf {
        &self.meetings_dir
    }

    /// Returns the path to the database file.
    pub fn get_db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Gets a connection to the meetings database.
    fn get_connection(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    /// Formats a Unix timestamp into a human-readable meeting title.
    ///
    /// # Arguments
    /// * `timestamp` - Unix timestamp in seconds
    ///
    /// # Returns
    /// A formatted string like "Meeting - January 15, 2025 3:30 PM"
    fn format_meeting_title(&self, timestamp: i64) -> String {
        if let Some(utc_datetime) = DateTime::from_timestamp(timestamp, 0) {
            let local_datetime = utc_datetime.with_timezone(&Local);
            format!(
                "Meeting - {}",
                local_datetime
                    .format("%B %e, %Y %l:%M %p")
                    .to_string()
                    .trim()
            )
        } else {
            format!("Meeting {}", timestamp)
        }
    }

    /// Creates a new meeting session with a unique UUID and dedicated folder.
    ///
    /// This method:
    /// 1. Generates a unique UUID for the session
    /// 2. Creates a dedicated folder under `meetings/{session-id}/`
    /// 3. Inserts the session into the database
    /// 4. Returns the created session
    ///
    /// # Returns
    /// * `Ok(MeetingSession)` - The newly created session
    /// * `Err` - If folder creation or database insertion fails
    pub fn create_session(&self) -> Result<MeetingSession> {
        let id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().timestamp();
        let title = self.format_meeting_title(created_at);

        // Create the session folder
        let session_dir = self.meetings_dir.join(&id);
        fs::create_dir_all(&session_dir)?;
        debug!("Created session folder: {:?}", session_dir);

        // Create the session object
        let session = MeetingSession::new(id.clone(), title.clone(), created_at);

        // Insert into database
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO meeting_sessions (id, title, created_at, status) VALUES (?1, ?2, ?3, ?4)",
            params![
                session.id,
                session.title,
                session.created_at,
                self.status_to_string(&session.status)
            ],
        )?;

        info!(
            "Created new meeting session: {} - {}",
            session.id, session.title
        );

        Ok(session)
    }

    /// Retrieves a meeting session by its ID.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to retrieve
    ///
    /// # Returns
    /// * `Ok(Some(MeetingSession))` - The session if found
    /// * `Ok(None)` - If no session with the given ID exists
    /// * `Err` - If database query fails
    pub fn get_session(&self, session_id: &str) -> Result<Option<MeetingSession>> {
        let conn = self.get_connection()?;
        let session = conn
            .query_row(
                "SELECT id, title, created_at, duration, status, audio_path, transcript_path, error_message
                 FROM meeting_sessions WHERE id = ?1",
                params![session_id],
                |row| self.row_to_session(row),
            )
            .optional()?;

        Ok(session)
    }

    /// Updates the status of a meeting session.
    ///
    /// This method updates the status and optionally the error message if the
    /// new status is `Failed`.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to update
    /// * `status` - The new status to set
    ///
    /// # Returns
    /// * `Ok(())` - If the update succeeded
    /// * `Err` - If the session doesn't exist or database update fails
    pub fn update_session_status(&self, session_id: &str, status: MeetingStatus) -> Result<()> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE meeting_sessions SET status = ?1 WHERE id = ?2",
            params![self.status_to_string(&status), session_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }

        debug!("Updated session {} status to {:?}", session_id, status);
        Ok(())
    }

    /// Lists all meeting sessions, ordered by creation time (newest first).
    ///
    /// # Returns
    /// * `Ok(Vec<MeetingSession>)` - All sessions in the database
    /// * `Err` - If database query fails
    pub fn list_sessions(&self) -> Result<Vec<MeetingSession>> {
        let conn = self.get_connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, duration, status, audio_path, transcript_path, error_message
             FROM meeting_sessions ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| self.row_to_session(row))?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }

        debug!("Listed {} meeting sessions", sessions.len());
        Ok(sessions)
    }

    /// Converts a MeetingStatus enum to its string representation for database storage.
    fn status_to_string(&self, status: &MeetingStatus) -> String {
        match status {
            MeetingStatus::Idle => "idle".to_string(),
            MeetingStatus::Recording => "recording".to_string(),
            MeetingStatus::Processing => "processing".to_string(),
            MeetingStatus::Completed => "completed".to_string(),
            MeetingStatus::Failed => "failed".to_string(),
        }
    }

    /// Converts a string from the database to a MeetingStatus enum.
    fn string_to_status(&self, s: &str) -> MeetingStatus {
        match s {
            "idle" => MeetingStatus::Idle,
            "recording" => MeetingStatus::Recording,
            "processing" => MeetingStatus::Processing,
            "completed" => MeetingStatus::Completed,
            "failed" => MeetingStatus::Failed,
            _ => MeetingStatus::Idle, // Default fallback
        }
    }

    /// Converts a database row to a MeetingSession struct.
    fn row_to_session(&self, row: &rusqlite::Row) -> rusqlite::Result<MeetingSession> {
        let status_str: String = row.get("status")?;
        Ok(MeetingSession {
            id: row.get("id")?,
            title: row.get("title")?,
            created_at: row.get("created_at")?,
            duration: row.get("duration")?,
            status: self.string_to_status(&status_str),
            audio_path: row.get("audio_path")?,
            transcript_path: row.get("transcript_path")?,
            error_message: row.get("error_message")?,
        })
    }

    /// Starts recording for a new meeting session.
    ///
    /// This method:
    /// 1. Creates a new meeting session with UUID and folder
    /// 2. Initializes the AudioRecorder
    /// 3. Creates and opens a WAV file for incremental writing
    /// 4. Starts audio capture from the microphone
    /// 5. Updates the session status to Recording
    ///
    /// # Returns
    /// * `Ok(MeetingSession)` - The newly created and active session
    /// * `Err` - If session creation, recorder initialization, or audio capture fails
    pub fn start_recording(&self) -> Result<MeetingSession> {
        // Check if already recording
        {
            let state = self.state.lock().unwrap();
            if let Some(session) = &state.current_session {
                if session.status == MeetingStatus::Recording {
                    return Err(anyhow::anyhow!(
                        "Cannot start recording: already recording session {}",
                        session.id
                    ));
                }
            }
        }

        // Create a new session
        let session = self.create_session()?;

        // Create audio file path: {session-id}/audio.wav
        let audio_filename = format!("{}/audio.wav", session.id);
        let audio_path = self.meetings_dir.join(&audio_filename);

        // Initialize WAV writer for incremental writing
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let audio_file = File::create(&audio_path)
            .map_err(|e| anyhow::anyhow!("Failed to create audio file: {}", e))?;

        let wav_writer = WavWriter::new(audio_file, spec)
            .map_err(|e| anyhow::anyhow!("Failed to create WAV writer: {}", e))?;

        // Initialize audio recorder
        let mut recorder = AudioRecorder::new()
            .map_err(|e| anyhow::anyhow!("Failed to create audio recorder: {}", e))?;

        // Add sample callback for incremental WAV writing
        let wav_writer_clone = wav_writer.clone();
        let sample_callback = move |samples: Vec<f32>| {
            let mut writer = wav_writer_clone;
            // Convert f32 samples to i16 and write incrementally
            for sample in &samples {
                let sample_i16 = (sample * i16::MAX as f32) as i16;
                if let Err(e) = writer.write_sample(sample_i16) {
                    error!("Failed to write audio sample: {}", e);
                }
            }
            // Flush periodically for crash resilience
            if let Err(e) = writer.flush() {
                error!("Failed to flush WAV file: {}", e);
            }
        };

        recorder = recorder.with_sample_callback(sample_callback);

        // Open recorder with default device
        recorder
            .open(None)
            .map_err(|e| anyhow::anyhow!("Failed to open audio recorder: {}", e))?;

        // Start audio capture
        recorder
            .start()
            .map_err(|e| anyhow::anyhow!("Failed to start audio capture: {}", e))?;

        // Update session with audio path
        let mut session_with_audio = session.clone();
        session_with_audio.audio_path = Some(audio_filename.clone());

        // Update database with audio path
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE meeting_sessions SET audio_path = ?1 WHERE id = ?2",
            params![audio_filename, session.id],
        )?;

        // Update state with recorder, wav_writer, and session
        {
            let mut state = self.state.lock().unwrap();
            state.recorder = Some(recorder);
            state.wav_writer = Some(wav_writer);
            state.current_session = Some(session_with_audio.clone());
        }

        // Update session status to Recording in database
        self.update_session_status(&session.id, MeetingStatus::Recording)?;

        // Update current session in state with Recording status
        {
            let mut state = self.state.lock().unwrap();
            let mut recording_session = session_with_audio.clone();
            recording_session.status = MeetingStatus::Recording;
            state.current_session = Some(recording_session);
        }

        info!(
            "Started recording for meeting session: {} - {} (audio: {:?})",
            session.id, session.title, audio_path
        );

        Ok(session_with_audio)
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

        assert!(
            table_exists,
            "meeting_sessions table should exist after multiple inits"
        );
    }

    /// Helper struct for testing CRUD operations without a full Tauri AppHandle.
    /// This mimics the relevant parts of MeetingSessionManager for unit testing.
    struct TestMeetingManager {
        meetings_dir: PathBuf,
        db_path: PathBuf,
        // Note: We don't include recorder in TestMeetingManager as it's for testing
        // CRUD operations, not audio recording functionality
    }

    impl TestMeetingManager {
        fn new(temp_dir: &std::path::Path) -> Self {
            let meetings_dir = temp_dir.join("meetings");
            let db_path = temp_dir.join("meetings.db");
            fs::create_dir_all(&meetings_dir).expect("Failed to create meetings dir");
            init_meeting_database(&db_path).expect("Failed to init database");
            Self {
                meetings_dir,
                db_path,
            }
        }

        fn get_connection(&self) -> Result<Connection> {
            Ok(Connection::open(&self.db_path)?)
        }

        fn status_to_string(&self, status: &MeetingStatus) -> String {
            match status {
                MeetingStatus::Idle => "idle".to_string(),
                MeetingStatus::Recording => "recording".to_string(),
                MeetingStatus::Processing => "processing".to_string(),
                MeetingStatus::Completed => "completed".to_string(),
                MeetingStatus::Failed => "failed".to_string(),
            }
        }

        fn string_to_status(&self, s: &str) -> MeetingStatus {
            match s {
                "idle" => MeetingStatus::Idle,
                "recording" => MeetingStatus::Recording,
                "processing" => MeetingStatus::Processing,
                "completed" => MeetingStatus::Completed,
                "failed" => MeetingStatus::Failed,
                _ => MeetingStatus::Idle,
            }
        }

        fn row_to_session(&self, row: &rusqlite::Row) -> rusqlite::Result<MeetingSession> {
            let status_str: String = row.get("status")?;
            Ok(MeetingSession {
                id: row.get("id")?,
                title: row.get("title")?,
                created_at: row.get("created_at")?,
                duration: row.get("duration")?,
                status: self.string_to_status(&status_str),
                audio_path: row.get("audio_path")?,
                transcript_path: row.get("transcript_path")?,
                error_message: row.get("error_message")?,
            })
        }

        fn create_session(&self) -> Result<MeetingSession> {
            let id = Uuid::new_v4().to_string();
            let created_at = chrono::Utc::now().timestamp();
            let title = format!("Test Meeting - {}", created_at);

            let session_dir = self.meetings_dir.join(&id);
            fs::create_dir_all(&session_dir)?;

            let session = MeetingSession::new(id.clone(), title.clone(), created_at);

            let conn = self.get_connection()?;
            conn.execute(
                "INSERT INTO meeting_sessions (id, title, created_at, status) VALUES (?1, ?2, ?3, ?4)",
                params![
                    session.id,
                    session.title,
                    session.created_at,
                    self.status_to_string(&session.status)
                ],
            )?;

            Ok(session)
        }

        fn get_session(&self, session_id: &str) -> Result<Option<MeetingSession>> {
            let conn = self.get_connection()?;
            let session = conn
                .query_row(
                    "SELECT id, title, created_at, duration, status, audio_path, transcript_path, error_message
                     FROM meeting_sessions WHERE id = ?1",
                    params![session_id],
                    |row| self.row_to_session(row),
                )
                .optional()?;

            Ok(session)
        }

        fn update_session_status(&self, session_id: &str, status: MeetingStatus) -> Result<()> {
            let conn = self.get_connection()?;
            let rows_affected = conn.execute(
                "UPDATE meeting_sessions SET status = ?1 WHERE id = ?2",
                params![self.status_to_string(&status), session_id],
            )?;

            if rows_affected == 0 {
                return Err(anyhow::anyhow!("Session not found: {}", session_id));
            }

            Ok(())
        }

        fn list_sessions(&self) -> Result<Vec<MeetingSession>> {
            let conn = self.get_connection()?;
            let mut stmt = conn.prepare(
                "SELECT id, title, created_at, duration, status, audio_path, transcript_path, error_message
                 FROM meeting_sessions ORDER BY created_at DESC",
            )?;

            let rows = stmt.query_map([], |row| self.row_to_session(row))?;

            let mut sessions = Vec::new();
            for row in rows {
                sessions.push(row?);
            }

            Ok(sessions)
        }
    }

    #[test]
    fn test_create_session() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        let session = manager.create_session().expect("Failed to create session");

        // Verify session has valid properties
        assert!(!session.id.is_empty(), "Session ID should not be empty");
        assert!(
            !session.title.is_empty(),
            "Session title should not be empty"
        );
        assert!(session.created_at > 0, "Created at should be positive");
        assert_eq!(session.status, MeetingStatus::Idle);
        assert!(session.duration.is_none());
        assert!(session.audio_path.is_none());
        assert!(session.transcript_path.is_none());

        // Verify session folder was created
        let session_dir = manager.meetings_dir.join(&session.id);
        assert!(session_dir.exists(), "Session folder should exist");
    }

    #[test]
    fn test_create_session_unique_ids() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        let session1 = manager
            .create_session()
            .expect("Failed to create session 1");
        let session2 = manager
            .create_session()
            .expect("Failed to create session 2");
        let session3 = manager
            .create_session()
            .expect("Failed to create session 3");

        // Verify all IDs are unique
        assert_ne!(session1.id, session2.id, "Session IDs should be unique");
        assert_ne!(session2.id, session3.id, "Session IDs should be unique");
        assert_ne!(session1.id, session3.id, "Session IDs should be unique");

        // Verify UUID format (8-4-4-4-12 hex format)
        let uuid_pattern = regex::Regex::new(
            r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
        )
        .unwrap();
        assert!(
            uuid_pattern.is_match(&session1.id),
            "Session ID should be valid UUID v4"
        );
        assert!(
            uuid_pattern.is_match(&session2.id),
            "Session ID should be valid UUID v4"
        );
    }

    #[test]
    fn test_get_session() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        // Create a session
        let created_session = manager.create_session().expect("Failed to create session");

        // Retrieve the session
        let retrieved = manager
            .get_session(&created_session.id)
            .expect("Failed to get session");

        assert!(retrieved.is_some(), "Session should be found");
        let retrieved = retrieved.unwrap();

        assert_eq!(retrieved.id, created_session.id);
        assert_eq!(retrieved.title, created_session.title);
        assert_eq!(retrieved.created_at, created_session.created_at);
        assert_eq!(retrieved.status, MeetingStatus::Idle);
    }

    #[test]
    fn test_get_session_not_found() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        // Try to get a non-existent session
        let result = manager
            .get_session("non-existent-id")
            .expect("Query should succeed");

        assert!(result.is_none(), "Non-existent session should return None");
    }

    #[test]
    fn test_update_session_status() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        // Create a session
        let session = manager.create_session().expect("Failed to create session");
        assert_eq!(session.status, MeetingStatus::Idle);

        // Update to Recording
        manager
            .update_session_status(&session.id, MeetingStatus::Recording)
            .expect("Failed to update status");

        let updated = manager
            .get_session(&session.id)
            .expect("Failed to get session")
            .expect("Session should exist");
        assert_eq!(updated.status, MeetingStatus::Recording);

        // Update to Processing
        manager
            .update_session_status(&session.id, MeetingStatus::Processing)
            .expect("Failed to update status");

        let updated = manager
            .get_session(&session.id)
            .expect("Failed to get session")
            .expect("Session should exist");
        assert_eq!(updated.status, MeetingStatus::Processing);

        // Update to Completed
        manager
            .update_session_status(&session.id, MeetingStatus::Completed)
            .expect("Failed to update status");

        let updated = manager
            .get_session(&session.id)
            .expect("Failed to get session")
            .expect("Session should exist");
        assert_eq!(updated.status, MeetingStatus::Completed);
    }

    #[test]
    fn test_update_session_status_not_found() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        // Try to update a non-existent session
        let result = manager.update_session_status("non-existent-id", MeetingStatus::Recording);

        assert!(result.is_err(), "Should fail for non-existent session");
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Session not found"),
            "Error should mention session not found"
        );
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        // Initially empty
        let sessions = manager.list_sessions().expect("Failed to list sessions");
        assert!(sessions.is_empty(), "Initially should have no sessions");

        // Create some sessions
        let session1 = manager
            .create_session()
            .expect("Failed to create session 1");
        std::thread::sleep(std::time::Duration::from_millis(10)); // Ensure different timestamps
        let session2 = manager
            .create_session()
            .expect("Failed to create session 2");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let session3 = manager
            .create_session()
            .expect("Failed to create session 3");

        // List sessions
        let sessions = manager.list_sessions().expect("Failed to list sessions");
        assert_eq!(sessions.len(), 3, "Should have 3 sessions");

        // Verify order (newest first)
        assert_eq!(
            sessions[0].id, session3.id,
            "Newest session should be first"
        );
        assert_eq!(sessions[1].id, session2.id);
        assert_eq!(sessions[2].id, session1.id, "Oldest session should be last");
    }

    #[test]
    fn test_list_sessions_with_different_statuses() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let manager = TestMeetingManager::new(temp_dir.path());

        // Create sessions with different statuses
        let session1 = manager
            .create_session()
            .expect("Failed to create session 1");
        manager
            .update_session_status(&session1.id, MeetingStatus::Completed)
            .expect("Failed to update status");

        let session2 = manager
            .create_session()
            .expect("Failed to create session 2");
        manager
            .update_session_status(&session2.id, MeetingStatus::Failed)
            .expect("Failed to update status");

        let session3 = manager
            .create_session()
            .expect("Failed to create session 3");
        // session3 stays as Idle

        // List sessions and verify statuses are preserved
        let sessions = manager.list_sessions().expect("Failed to list sessions");
        assert_eq!(sessions.len(), 3);

        // Find sessions by ID and check their statuses
        let s1 = sessions.iter().find(|s| s.id == session1.id).unwrap();
        let s2 = sessions.iter().find(|s| s.id == session2.id).unwrap();
        let s3 = sessions.iter().find(|s| s.id == session3.id).unwrap();

        assert_eq!(s1.status, MeetingStatus::Completed);
        assert_eq!(s2.status, MeetingStatus::Failed);
        assert_eq!(s3.status, MeetingStatus::Idle);
    }
}
