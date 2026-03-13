//! Core MeetingSessionManager implementation.
//!
//! Contains the manager struct, recording lifecycle (start/stop),
//! mic disconnect handling, transcription, and app shutdown cleanup.

use anyhow::Result;
use chrono::{DateTime, Local};
use hound::{WavReader, WavSpec, WavWriter};
use log::{debug, error, info};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::audio_toolkit::{AudioSourceConfig, MixedAudioRecorder};
use crate::managers::meeting_logger::{
    log_meeting_event, log_performance_metric, MeetingLogContext, MeetingTimer,
};

use super::db::init_meeting_database;
use super::models::{AudioSourceType, MeetingManagerState, MeetingSession, MeetingStatus};
use super::wav_writer::WavWriterHandle;


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
    /// Transcription manager for STT processing
    transcription_manager: Arc<crate::managers::transcription::TranscriptionManager>,
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
    /// * `transcription_manager` - Reference to the TranscriptionManager
    ///
    /// # Returns
    /// * `Ok(Self)` - Successfully initialized manager
    /// * `Err` - Failed to create directories or initialize database
    ///
    /// # Example
    /// ```ignore
    /// let manager = MeetingSessionManager::new(&app_handle, &transcription_manager)?;
    /// ```
    pub fn new(
        app_handle: &AppHandle,
        transcription_manager: Arc<crate::managers::transcription::TranscriptionManager>,
    ) -> Result<Self> {
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
            transcription_manager,
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
    #[allow(dead_code)]
    pub fn get_db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Gets the current session status atomically.
    ///
    /// # Returns
    /// * `Some(MeetingStatus)` - The current session status if a session exists
    /// * `None` - If no session is active
    pub fn get_current_status(&self) -> Option<MeetingStatus> {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.current_session.as_ref().map(|s| s.status.clone())
    }

    /// Gets the current session from in-memory state.
    ///
    /// # Returns
    /// * `Some(MeetingSession)` - Clone of the current session if one exists
    /// * `None` - If no session is active
    pub fn get_current_session(&self) -> Option<MeetingSession> {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.current_session.clone()
    }

    /// Updates the title of a meeting session.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to update
    /// * `title` - The new title for the session
    ///
    /// # Returns
    /// * `Ok(())` - If the title was updated successfully
    /// * `Err` - If session not found or database update fails
    pub fn update_session_title(&self, session_id: &str, title: &str) -> Result<()> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE meeting_sessions SET title = ?1 WHERE id = ?2",
            params![title, session_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }

        // Update in-memory state if this is the current session
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(session) = state.current_session.as_mut() {
                if session.id == session_id {
                    session.title = title.to_string();
                }
            }
        }

        info!(
            "Updated meeting title for session {}: {}",
            session_id, title
        );
        Ok(())
    }

    /// Updates the template_id for a meeting session.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to update
    /// * `template_id` - The template ID to associate with this session
    ///
    /// # Returns
    /// * `Ok(())` - If the template_id was updated successfully
    /// * `Err` - If session not found or database update fails
    pub fn update_session_template_id(&self, session_id: &str, template_id: &str) -> Result<()> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE meeting_sessions SET template_id = ?1 WHERE id = ?2",
            params![template_id, session_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }

        // Update in-memory state if this is the current session
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(session) = state.current_session.as_mut() {
                if session.id == session_id {
                    session.template_id = Some(template_id.to_string());
                }
            }
        }

        info!(
            "Updated template_id for session {}: {}",
            session_id, template_id
        );
        Ok(())
    }

    /// Updates the summary path for a meeting session.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to update
    /// * `summary_path` - The relative path to the summary file
    ///
    /// # Returns
    /// * `Ok(())` - If the summary path was updated successfully
    /// * `Err` - If session not found or database update fails
    pub fn update_session_summary_path(&self, session_id: &str, summary_path: &str) -> Result<()> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE meeting_sessions SET summary_path = ?1 WHERE id = ?2",
            params![summary_path, session_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }

        // Update in-memory state if this is the current session
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(session) = state.current_session.as_mut() {
                if session.id == session_id {
                    session.summary_path = Some(summary_path.to_string());
                }
            }
        }

        info!(
            "Updated summary path for session {}: {}",
            session_id, summary_path
        );
        Ok(())
    }

    /// Retries transcription for a failed or interrupted session.
    ///
    /// This method:
    /// 1. Validates the session exists and has an audio file
    /// 2. Updates status to Processing
    /// 3. Spawns background transcription task
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to retry
    /// * `app_handle` - The Tauri app handle for emitting events
    ///
    /// # Returns
    /// * `Ok(())` - If retry was initiated successfully
    /// * `Err` - If session not found, no audio file, or retry fails
    pub fn retry_transcription_for_session(&self, session_id: &str) -> Result<String> {
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // Get audio path
        let audio_path = session
            .audio_path
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Session has no audio file to transcribe"))?;

        // Update status to Processing
        self.update_session_status(session_id, MeetingStatus::Processing)?;

        // Update in-memory state
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(current_session) = state.current_session.as_mut() {
                if current_session.id == session_id {
                    current_session.status = MeetingStatus::Processing;
                    current_session.error_message = None;
                }
            } else {
                // Set this as current session if none active
                let mut updated_session = session.clone();
                updated_session.status = MeetingStatus::Processing;
                updated_session.error_message = None;
                state.current_session = Some(updated_session);
            }
        }

        Ok(audio_path)
    }

    /// Saves the transcript and updates status to Completed (public wrapper).
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session
    /// * `transcript_text` - The transcribed text to save
    ///
    /// # Returns
    /// * `Ok(())` - If the transcript was saved and status updated successfully
    /// * `Err` - If file writing or database update fails
    pub fn save_transcript(&self, session_id: &str, transcript_text: &str) -> Result<()> {
        self.save_transcript_and_update_status(session_id, transcript_text)
    }

    /// Updates the in-memory state with error message for a failed session.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session
    /// * `error_message` - The error message to store
    pub fn set_session_error(&self, session_id: &str, error_message: &str) {
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(session) = state.current_session.as_mut() {
            if session.id == session_id {
                session.status = MeetingStatus::Failed;
                session.error_message = Some(error_message.to_string());
            }
        }
    }

    /// Handles a transcription failure by updating the database, emitting events,
    /// and updating in-memory state. Consolidates the repeated error handling pattern
    /// used in the background transcription task.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session that failed
    /// * `error_msg` - The error message describing the failure
    fn handle_transcription_failure(&self, session_id: &str, error_msg: &str) {
        // Update status to Failed in database
        if let Err(update_err) = self.update_session_status_with_error(
            session_id,
            MeetingStatus::Failed,
            error_msg,
        ) {
            error!(
                "Failed to update session {} status to Failed: {}",
                session_id, update_err
            );
            return;
        }

        // Emit meeting_failed event
        if let Ok(Some(session_data)) = self.get_session(session_id) {
            if let Err(emit_err) = self.app_handle.emit("meeting_failed", session_data.clone()) {
                error!("Failed to emit meeting_failed event: {}", emit_err);
            } else {
                info!("Emitted meeting_failed event for session {}", session_id);
            }
        }

        // Update in-memory state with error message
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(mut session) = state.current_session.take() {
            if session.id == session_id {
                session.status = MeetingStatus::Failed;
                session.error_message = Some(error_msg.to_string());
                state.current_session = Some(session);
            }
        }
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
    #[allow(dead_code)]
    pub fn create_session(&self) -> Result<MeetingSession> {
        self.create_session_with_audio_source(AudioSourceType::default())
    }

    /// Creates a new meeting session with a specified audio source.
    ///
    /// # Arguments
    /// * `audio_source` - The audio source configuration for this meeting
    ///
    /// # Returns
    /// * `Ok(MeetingSession)` - The newly created session
    /// * `Err` - If folder creation or database insertion fails
    pub fn create_session_with_audio_source(
        &self,
        audio_source: AudioSourceType,
    ) -> Result<MeetingSession> {
        let id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().timestamp();
        let title = self.format_meeting_title(created_at);

        // Create the session folder
        let session_dir = self.meetings_dir.join(&id);
        fs::create_dir_all(&session_dir)?;
        debug!("Created session folder: {:?}", session_dir);

        // Create the session object
        let session = MeetingSession::new_with_audio_source(
            id.clone(),
            title.clone(),
            created_at,
            audio_source.clone(),
        );

        // Insert into database
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO meeting_sessions (id, title, created_at, status, audio_source, template_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                session.id,
                session.title,
                session.created_at,
                self.status_to_string(&session.status),
                self.audio_source_to_string(&audio_source),
                session.template_id
            ],
        )?;

        info!(
            "Created new meeting session: {} - {} (audio: {:?})",
            session.id, session.title, audio_source
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
                "SELECT id, title, created_at, duration, status, audio_path, transcript_path, error_message, audio_source, summary_path, template_id
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

    /// Updates the status of a meeting session with an error message.
    ///
    /// This method updates both the status and the error_message field.
    /// Used primarily when setting status to Failed to record what went wrong.
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to update
    /// * `status` - The new status to set
    /// * `error_message` - The error message to store
    ///
    /// # Returns
    /// * `Ok(())` - If the update succeeded
    /// * `Err` - If the session doesn't exist or database update fails
    pub fn update_session_status_with_error(
        &self,
        session_id: &str,
        status: MeetingStatus,
        error_message: &str,
    ) -> Result<()> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE meeting_sessions SET status = ?1, error_message = ?2 WHERE id = ?3",
            params![self.status_to_string(&status), error_message, session_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }

        debug!(
            "Updated session {} status to {:?} with error: {}",
            session_id, status, error_message
        );
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
            "SELECT id, title, created_at, duration, status, audio_path, transcript_path, error_message, audio_source, summary_path, template_id
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

    /// Deletes a meeting session and its associated files.
    ///
    /// This method:
    /// 1. Retrieves the session from the database
    /// 2. Deletes the session folder (containing audio and transcript files)
    /// 3. Removes the session record from the database
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session to delete
    ///
    /// # Returns
    /// * `Ok(())` if the session was deleted successfully
    /// * `Err` if session not found or deletion fails
    pub fn delete_session(&self, session_id: &str) -> Result<()> {
        info!("Deleting meeting session: {}", session_id);

        // Verify session exists before deleting
        let _session = self
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        // Delete session folder if it exists
        let session_folder = self.meetings_dir.join(session_id);
        if session_folder.exists() {
            fs::remove_dir_all(&session_folder)?;
            info!("Deleted session folder: {:?}", session_folder);
        }

        // Delete from database
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "DELETE FROM meeting_sessions WHERE id = ?1",
            params![session_id],
        )?;

        if rows_affected == 0 {
            return Err(anyhow::anyhow!(
                "Session not found in database: {}",
                session_id
            ));
        }

        info!("Deleted meeting session from database: {}", session_id);
        Ok(())
    }

    /// Converts a MeetingStatus enum to its string representation for database storage.
    fn status_to_string(&self, status: &MeetingStatus) -> String {
        match status {
            MeetingStatus::Idle => "idle".to_string(),
            MeetingStatus::Recording => "recording".to_string(),
            MeetingStatus::Processing => "processing".to_string(),
            MeetingStatus::Completed => "completed".to_string(),
            MeetingStatus::Failed => "failed".to_string(),
            MeetingStatus::Interrupted => "interrupted".to_string(),
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
            "interrupted" => MeetingStatus::Interrupted,
            _ => MeetingStatus::Idle, // Default fallback
        }
    }

    /// Validates that a state transition is allowed.
    ///
    /// Allowed transitions:
    /// - Idle -> Recording (start recording)
    /// - Recording -> Processing (stop recording)
    /// - Recording -> Failed (mic disconnect or critical error)
    /// - Recording -> Interrupted (app closed during recording)
    /// - Processing -> Completed (transcription success)
    /// - Processing -> Failed (transcription failure)
    /// - Failed -> Processing (retry transcription)
    /// - Interrupted -> Processing (resume transcription on next launch)
    ///
    /// # Arguments
    /// * `from` - The current state
    /// * `to` - The proposed new state
    ///
    /// # Returns
    /// * `Ok(())` if the transition is valid
    /// * `Err` if the transition is not allowed
    fn validate_state_transition(&self, from: &MeetingStatus, to: &MeetingStatus) -> Result<()> {
        match (from, to) {
            // Allowed transitions
            (MeetingStatus::Idle, MeetingStatus::Recording) => Ok(()),
            (MeetingStatus::Recording, MeetingStatus::Processing) => Ok(()),
            (MeetingStatus::Recording, MeetingStatus::Failed) => Ok(()), // Mic disconnect
            (MeetingStatus::Recording, MeetingStatus::Interrupted) => Ok(()), // App shutdown
            (MeetingStatus::Processing, MeetingStatus::Completed) => Ok(()),
            (MeetingStatus::Processing, MeetingStatus::Failed) => Ok(()),
            (MeetingStatus::Failed, MeetingStatus::Processing) => Ok(()),
            (MeetingStatus::Interrupted, MeetingStatus::Processing) => Ok(()), // Resume

            // Disallowed transitions
            _ => Err(anyhow::anyhow!(
                "Invalid state transition: {:?} -> {:?}",
                from,
                to
            )),
        }
    }

    /// Converts a database row to a MeetingSession struct.
    fn row_to_session(&self, row: &rusqlite::Row) -> rusqlite::Result<MeetingSession> {
        let status_str: String = row.get("status")?;
        let audio_source_str: String = row
            .get("audio_source")
            .unwrap_or_else(|_| "microphone_only".to_string());
        let summary_path: Option<String> = row.get("summary_path")?;
        let template_id: Option<String> = row.get("template_id").unwrap_or(None);
        Ok(MeetingSession {
            id: row.get("id")?,
            title: row.get("title")?,
            created_at: row.get("created_at")?,
            duration: row.get("duration")?,
            status: self.string_to_status(&status_str),
            audio_path: row.get("audio_path")?,
            transcript_path: row.get("transcript_path")?,
            error_message: row.get("error_message")?,
            audio_source: self.string_to_audio_source(&audio_source_str),
            summary_path,
            template_id,
        })
    }

    /// Converts an AudioSourceType to database string.
    fn audio_source_to_string(&self, source: &AudioSourceType) -> &'static str {
        match source {
            AudioSourceType::MicrophoneOnly => "microphone_only",
            AudioSourceType::SystemOnly => "system_only",
            AudioSourceType::Mixed => "mixed",
        }
    }

    /// Converts a database string to AudioSourceType.
    fn string_to_audio_source(&self, s: &str) -> AudioSourceType {
        match s {
            "microphone_only" => AudioSourceType::MicrophoneOnly,
            "system_only" => AudioSourceType::SystemOnly,
            "mixed" => AudioSourceType::Mixed,
            _ => AudioSourceType::MicrophoneOnly, // Default fallback
        }
    }

    /// Starts recording for a new meeting session.
    ///
    /// This method:
    /// 1. Validates no active session is in Recording/Processing state
    /// 2. Creates a new meeting session with UUID and folder
    /// 3. Initializes the MixedAudioRecorder with the specified audio source
    /// 4. Creates and opens a WAV file for incremental writing
    /// 5. Starts audio capture from the selected source(s)
    /// 6. Updates the session status to Recording atomically
    ///
    /// # Arguments
    /// * `audio_source` - The audio source configuration (MicrophoneOnly, SystemOnly, or Mixed)
    ///
    /// # Returns
    /// * `Ok(MeetingSession)` - The newly created and active session
    /// * `Err` - If state guard fails, session creation, recorder initialization, or audio capture fails
    pub fn start_recording(&self, audio_source: AudioSourceType) -> Result<MeetingSession> {
        let timer = MeetingTimer::start();

        // State machine guard: validate transition from Idle -> Recording
        // Cannot start recording if already recording or processing
        let current_status = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.current_session.as_ref().map(|s| s.status.clone())
        };

        if let Some(status) = current_status {
            match status {
                MeetingStatus::Recording => {
                    error!("[MEETING_START] Rejected: already recording");
                    return Err(anyhow::anyhow!(
                        "Cannot start recording: already recording an active session"
                    ));
                }
                MeetingStatus::Processing => {
                    error!("[MEETING_START] Rejected: session being processed");
                    return Err(anyhow::anyhow!(
                        "Cannot start recording: another session is currently being processed"
                    ));
                }
                _ => {
                    // Completed, Failed, or Idle status - can start new recording
                    debug!(
                        "[MEETING_START] Previous session status: {:?}, proceeding",
                        status
                    );
                }
            }
        }

        // Convert AudioSourceType to AudioSourceConfig for MixedAudioRecorder
        let audio_config = match &audio_source {
            AudioSourceType::MicrophoneOnly => AudioSourceConfig::MicrophoneOnly,
            AudioSourceType::SystemOnly => AudioSourceConfig::SystemOnly,
            AudioSourceType::Mixed => AudioSourceConfig::Mixed,
        };

        info!(
            "[MEETING_START] Creating session with audio source: {:?}",
            audio_source
        );

        // Create a new session with the specified audio source
        let session = self.create_session_with_audio_source(audio_source.clone())?;

        let log_ctx = MeetingLogContext::new(&session.id, "start_recording");
        log_ctx.log_start();

        // Create audio file path: {session-id}/audio.wav
        let audio_filename = format!("{}/audio.wav", session.id);
        let audio_path = self.meetings_dir.join(&audio_filename);

        log_ctx.log_file_op(&audio_path.display().to_string(), None);

        // Initialize WAV writer for incremental writing
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        debug!(
            "[MEETING_START] [{}] WAV spec: {}Hz, {} channel(s), {}bit",
            session.id, spec.sample_rate, spec.channels, spec.bits_per_sample
        );

        let audio_file = File::create(&audio_path).map_err(|e| {
            log_ctx.log_error(&format!("Failed to create audio file: {}", e));
            anyhow::anyhow!("Failed to create audio file: {}", e)
        })?;

        let wav_writer = WavWriter::new(audio_file, spec).map_err(|e| {
            log_ctx.log_error(&format!("Failed to create WAV writer: {}", e));
            anyhow::anyhow!("Failed to create WAV writer: {}", e)
        })?;

        // Wrap in WavWriterHandle for timeout-based finalization
        let wav_handle = WavWriterHandle::new(wav_writer);

        // Add sample callback for incremental WAV writing
        let wav_handle_clone = wav_handle.clone();
        let sample_callback = move |samples: Vec<f32>| {
            if let Err(e) = wav_handle_clone.write_samples(&samples) {
                error!("Failed to write audio samples: {}", e);
            }
        };

        debug!(
            "[MEETING_START] [{}] Initializing MixedAudioRecorder with {:?}",
            session.id, audio_config
        );

        // Initialize MixedAudioRecorder with the configured audio source
        let mut mixed_recorder = MixedAudioRecorder::new(audio_config.clone()).map_err(|e| {
            log_ctx.log_error(&format!("Failed to create recorder: {}", e));
            anyhow::anyhow!("Failed to create mixed audio recorder: {}", e)
        })?;

        mixed_recorder = mixed_recorder.with_sample_callback(sample_callback);

        // Add error callback to detect mic disconnect
        let manager_clone = self.clone();
        let fired = Arc::new(AtomicBool::new(false));
        mixed_recorder = mixed_recorder.with_error_callback({
            let fired = Arc::clone(&fired);
            move |error| {
                // Only fire once (debounce)
                if fired.swap(true, Ordering::SeqCst) {
                    return;
                }

                // Spawn async task to avoid blocking audio thread
                let manager = manager_clone.clone();
                let error_msg = error.clone();
                tauri::async_runtime::spawn(async move {
                    manager.handle_mic_disconnect(&error_msg);
                });
            }
        });

        let recorder_timer = MeetingTimer::start();

        // Start audio capture
        mixed_recorder.start().map_err(|e| {
            log_ctx.log_error(&format!("Failed to start audio capture: {}", e));
            anyhow::anyhow!("Failed to start audio capture: {}", e)
        })?;

        log_ctx.log_timing("recorder_start", recorder_timer.elapsed_ms());

        // Update session with audio path
        let mut session_with_audio = session.clone();
        session_with_audio.audio_path = Some(audio_filename.clone());

        // Update database with audio path
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE meeting_sessions SET audio_path = ?1 WHERE id = ?2",
            params![audio_filename, session.id],
        )?;

        // Update state with mixed_recorder, wav_handle, and session
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.mixed_recorder = Some(mixed_recorder);
            state.wav_writer = Some(wav_handle);
            state.current_session = Some(session_with_audio.clone());
        }

        log_ctx.log_state_transition("Idle", "Recording");

        // Update session status to Recording in database
        self.update_session_status(&session.id, MeetingStatus::Recording)?;

        // Emit meeting_started event
        let session_clone = session_with_audio.clone();
        if let Err(e) = self
            .app_handle
            .emit("meeting_started", session_clone.clone())
        {
            log_ctx.log_error(&format!("Failed to emit meeting_started event: {}", e));
        } else {
            log_ctx.log_debug("Emitted meeting_started event");
        }

        // Update current session in state with Recording status
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let mut recording_session = session_with_audio.clone();
            recording_session.status = MeetingStatus::Recording;
            state.current_session = Some(recording_session);
        }

        let total_time = timer.elapsed_ms();
        log_ctx.log_success_with_duration(
            total_time,
            &format!(
                "Session started - audio: {:?}, path: {}",
                audio_source,
                audio_path.display()
            ),
        );

        log_meeting_event(
            &session.id,
            "session_started",
            &format!("source={:?} path={}", audio_source, audio_filename),
        );

        Ok(session_with_audio)
    }

    /// Stops recording for the current meeting session.
    ///
    /// This method:
    /// 1. Validates current session is in Recording state
    /// 2. Stops audio capture from the AudioRecorder
    /// 3. Finalizes the WAV file (flush and close)
    /// 4. Calculates the recording duration
    /// 5. Updates the session status to Processing atomically
    /// 6. Returns the audio file path
    ///
    /// # Returns
    /// * `Ok(String)` - The relative path to the audio file (e.g., "{session-id}/audio.wav")
    /// * `Err` - If no recording is active, invalid state, or if stopping/finalization fails
    pub fn stop_recording(&self) -> Result<String> {
        let timer = MeetingTimer::start();

        // State machine guard: validate transition from Recording -> Processing
        // Cannot stop if no active session or not in Recording state
        let (session_id, audio_path_opt) = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            let session = state.current_session.as_ref().ok_or_else(|| {
                error!("[MEETING_STOP] Rejected: no active session");
                anyhow::anyhow!("Cannot stop recording: no active session")
            })?;

            match session.status {
                MeetingStatus::Recording => {
                    // Valid transition
                    let audio_path = session.audio_path.as_ref().ok_or_else(|| {
                        anyhow::anyhow!(
                            "Cannot stop recording: no audio path set for session {}",
                            session.id
                        )
                    })?;
                    (session.id.clone(), audio_path.clone())
                }
                MeetingStatus::Idle => {
                    error!("[MEETING_STOP] Rejected: session is Idle");
                    return Err(anyhow::anyhow!(
                        "Cannot stop recording: no recording in progress (session is Idle)"
                    ));
                }
                MeetingStatus::Processing => {
                    error!("[MEETING_STOP] Rejected: session already processing");
                    return Err(anyhow::anyhow!(
                        "Cannot stop recording: session is already being processed"
                    ));
                }
                MeetingStatus::Completed => {
                    error!("[MEETING_STOP] Rejected: session already completed");
                    return Err(anyhow::anyhow!(
                        "Cannot stop recording: session has already been completed"
                    ));
                }
                MeetingStatus::Failed => {
                    error!("[MEETING_STOP] Rejected: session has failed");
                    return Err(anyhow::anyhow!("Cannot stop recording: session has failed"));
                }
                MeetingStatus::Interrupted => {
                    error!("[MEETING_STOP] Rejected: session was interrupted");
                    return Err(anyhow::anyhow!(
                        "Cannot stop recording: session was interrupted"
                    ));
                }
            }
        };

        let log_ctx = MeetingLogContext::new(&session_id, "stop_recording");
        log_ctx.log_start();

        // Stop audio capture
        let recorder_timer = MeetingTimer::start();
        let mixed_recorder_opt = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.mixed_recorder.take()
        };

        if let Some(mut mixed_recorder) = mixed_recorder_opt {
            mixed_recorder.stop().map_err(|e| {
                log_ctx.log_error(&format!("Failed to stop recorder: {}", e));
                anyhow::anyhow!("Failed to stop mixed audio recorder: {}", e)
            })?;

            log_ctx.log_timing("recorder_stop", recorder_timer.elapsed_ms());

            // Close recorder to release resources
            mixed_recorder.close().map_err(|e| {
                log_ctx.log_error(&format!("Failed to close recorder: {}", e));
                anyhow::anyhow!("Failed to close mixed audio recorder: {}", e)
            })?;

            log_ctx.log_debug("Audio capture stopped and closed");
        }

        // Finalize WAV file with timeout
        let wav_timer = MeetingTimer::start();
        let wav_writer_opt = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.wav_writer.take()
        };

        if let Some(wav_handle) = wav_writer_opt {
            // Try to finalize with 5 second timeout
            if let Err(e) = wav_handle.finalize_with_timeout(Duration::from_secs(5)) {
                log_ctx.log_warning(&format!("WAV finalization failed: {}", e));
                // Continue anyway - partial audio is saved
                // Don't return error, just log it
            } else {
                log_ctx.log_timing("wav_finalize", wav_timer.elapsed_ms());
                log_ctx.log_debug("WAV file finalized successfully");
            }
        }

        // Calculate duration
        let current_session = self.get_session(&session_id)?.ok_or_else(|| {
            anyhow::anyhow!("Session {} not found after stopping recording", session_id)
        })?;

        let duration = chrono::Utc::now().timestamp() - current_session.created_at;
        if duration < 0 {
            log_ctx.log_error(&format!(
                "Invalid duration: created_at {} > now {}",
                current_session.created_at,
                chrono::Utc::now().timestamp()
            ));
            return Err(anyhow::anyhow!(
                "Invalid duration calculated for session {}: created_at {} > now {}",
                session_id,
                current_session.created_at,
                chrono::Utc::now().timestamp()
            ));
        }

        log_performance_metric(
            &session_id,
            "recording_duration",
            duration as f64,
            "seconds",
        );

        // Validate state transition before updating
        {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(session) = &state.current_session {
                self.validate_state_transition(&session.status, &MeetingStatus::Processing)
                    .map_err(|e| {
                        log_ctx.log_error(&format!("State transition validation failed: {}", e));
                        anyhow::anyhow!("State transition validation failed: {}", e)
                    })?;
            }
        }

        log_ctx.log_state_transition("Recording", "Processing");

        // Emit meeting_stopped event with session details
        let session_for_event = self.get_session(&session_id)?.ok_or_else(|| {
            anyhow::anyhow!(
                "Session {} not found when emitting meeting_stopped",
                session_id
            )
        })?;

        if let Err(e) = self
            .app_handle
            .emit("meeting_stopped", session_for_event.clone())
        {
            log_ctx.log_error(&format!("Failed to emit meeting_stopped event: {}", e));
        } else {
            log_ctx.log_debug("Emitted meeting_stopped event");
        }

        // Update database with duration and status
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE meeting_sessions SET duration = ?1, status = ?2 WHERE id = ?3",
            params![
                duration,
                self.status_to_string(&MeetingStatus::Processing),
                session_id
            ],
        )?;

        // Update in-memory state atomically
        let updated_session = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(mut session) = state.current_session.take() {
                session.status = MeetingStatus::Processing;
                session.duration = Some(duration);
                state.current_session = Some(session.clone());
                session
            } else {
                return Err(anyhow::anyhow!("No current session found"));
            }
        };

        // Emit meeting_processing event after status update
        if let Err(e) = self
            .app_handle
            .emit("meeting_processing", updated_session.clone())
        {
            log_ctx.log_error(&format!("Failed to emit meeting_processing event: {}", e));
        } else {
            log_ctx.log_debug("Emitted meeting_processing event");
        }

        let total_time = timer.elapsed_ms();
        log_ctx.log_success_with_duration(
            total_time,
            &format!(
                "Recording stopped - duration={}s, audio={}",
                duration, audio_path_opt
            ),
        );

        log_meeting_event(
            &session_id,
            "recording_stopped",
            &format!("duration={}s path={}", duration, audio_path_opt),
        );

        // Spawn background task for transcription to avoid blocking UI
        let manager_clone = self.clone();
        let session_id_clone = session_id.clone();
        let audio_path_clone = audio_path_opt.clone();

        thread::spawn(move || {
            debug!(
                "Background transcription task started for session {}",
                session_id_clone
            );

            // Process transcription in background
            match manager_clone.process_transcription(&audio_path_clone) {
                Ok(transcription_text) => {
                    debug!(
                        "Background transcription succeeded for session {}: {} bytes",
                        session_id_clone,
                        transcription_text.len()
                    );

                    // Save transcript and update status to Completed
                    if let Err(e) = manager_clone
                        .save_transcript_and_update_status(&session_id_clone, &transcription_text)
                    {
                        let error_msg = format!("Failed to save transcript: {}", e);
                        error!(
                            "Failed to save transcript for session {}: {}",
                            session_id_clone, error_msg
                        );
                        manager_clone.handle_transcription_failure(&session_id_clone, &error_msg);
                    } else {
                        info!(
                            "Session {} transcription completed successfully",
                            session_id_clone
                        );

                        // Emit meeting_completed event
                        if let Ok(Some(session_data)) = manager_clone.get_session(&session_id_clone) {
                            if let Err(emit_err) = manager_clone
                                .app_handle
                                .emit("meeting_completed", session_data.clone())
                            {
                                error!("Failed to emit meeting_completed event: {}", emit_err);
                            } else {
                                info!(
                                    "Emitted meeting_completed event for session {}",
                                    session_id_clone
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!("Transcription failed: {}", e);
                    error!(
                        "Background transcription failed for session {}: {}",
                        session_id_clone, error_msg
                    );
                    manager_clone.handle_transcription_failure(&session_id_clone, &error_msg);
                }
            }
        });

        Ok(audio_path_opt)
    }

    /// Handles microphone disconnect or audio stream error during recording.
    ///
    /// This method:
    /// 1. Logs the error
    /// 2. Stops any ongoing recording and finalizes the WAV file
    /// 3. Updates the session status to Failed with an error message
    /// 4. Emits a meeting_failed event
    /// 5. Preserves any partial audio that was captured
    ///
    /// This method is designed to be called from an error callback in the audio stream.
    /// It gracefully handles the disconnect while preserving any data that was recorded.
    ///
    /// # Arguments
    /// * `error_message` - Description of the error that occurred
    #[allow(dead_code)]
    pub fn handle_mic_disconnect(&self, error_message: &str) {
        let timer = MeetingTimer::start();
        error!("[MIC_DISCONNECT] Detected: {}", error_message);

        // Get current session info
        let session_info = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .current_session
                .as_ref()
                .map(|s| (s.id.clone(), s.status.clone()))
        };

        let (session_id, status) = match session_info {
            Some((id, status)) => (id, status),
            None => {
                debug!("[MIC_DISCONNECT] No active session - ignoring");
                return;
            }
        };

        let log_ctx = MeetingLogContext::new(&session_id, "handle_mic_disconnect");
        log_ctx.log_start();
        log_ctx.log_error(error_message);

        // Only handle if we're currently recording
        if status != MeetingStatus::Recording {
            log_ctx.log_debug(&format!(
                "Session not recording (status: {:?}) - ignoring",
                status
            ));
            return;
        }

        // Stop the recorder if it exists (don't fail if stop errors)
        let recorder_timer = MeetingTimer::start();
        let mixed_recorder_opt = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.mixed_recorder.take()
        };

        if let Some(mut mixed_recorder) = mixed_recorder_opt {
            if let Err(e) = mixed_recorder.stop() {
                log_ctx.log_warning(&format!("Failed to stop recorder: {}", e));
                // Continue anyway - we want to save partial audio
            } else {
                log_ctx.log_timing("recorder_stop", recorder_timer.elapsed_ms());
            }
            // Close recorder to release resources
            if let Err(e) = mixed_recorder.close() {
                log_ctx.log_warning(&format!("Failed to close recorder: {}", e));
            }
        }

        // Finalize the WAV file to ensure partial audio is saved
        let wav_timer = MeetingTimer::start();
        let wav_writer_opt = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.wav_writer.take()
        };

        if let Some(wav_handle) = wav_writer_opt {
            // Try to finalize with 5 second timeout
            if let Err(e) = wav_handle.finalize_with_timeout(Duration::from_secs(5)) {
                log_ctx.log_error(&format!("Failed to finalize WAV: {}", e));
                // Continue anyway - we still want to update status
            } else {
                log_ctx.log_timing("wav_finalize", wav_timer.elapsed_ms());
                log_ctx.log_debug("Successfully finalized partial audio");
            }
        }

        // Calculate partial duration
        let duration = {
            if let Ok(Some(session)) = self.get_session(&session_id) {
                let now = chrono::Utc::now().timestamp();
                let partial_duration = now - session.created_at;
                if partial_duration > 0 {
                    Some(partial_duration)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(dur) = duration {
            log_performance_metric(
                &session_id,
                "partial_recording_duration",
                dur as f64,
                "seconds",
            );
        }

        log_ctx.log_state_transition("Recording", "Failed");

        // Update database with Failed status, error message, and partial duration
        let error_msg = format!("Microphone disconnected: {}", error_message);
        if let Ok(conn) = self.get_connection() {
            let update_result = if let Some(dur) = duration {
                conn.execute(
                    "UPDATE meeting_sessions SET status = ?1, error_message = ?2, duration = ?3 WHERE id = ?4",
                    params![
                        self.status_to_string(&MeetingStatus::Failed),
                        &error_msg,
                        dur,
                        &session_id
                    ],
                )
            } else {
                conn.execute(
                    "UPDATE meeting_sessions SET status = ?1, error_message = ?2 WHERE id = ?3",
                    params![
                        self.status_to_string(&MeetingStatus::Failed),
                        &error_msg,
                        &session_id
                    ],
                )
            };

            if let Err(e) = update_result {
                log_ctx.log_error(&format!("Failed to update database: {}", e));
            }
        }

        // Update in-memory state
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(mut session) = state.current_session.take() {
                if session.id == session_id {
                    session.status = MeetingStatus::Failed;
                    session.error_message = Some(error_msg.clone());
                    session.duration = duration;
                    state.current_session = Some(session);
                }
            }
        }

        // Emit meeting_failed event
        if let Ok(Some(session_data)) = self.get_session(&session_id) {
            if let Err(e) = self.app_handle.emit("meeting_failed", session_data.clone()) {
                log_ctx.log_error(&format!("Failed to emit meeting_failed event: {}", e));
            } else {
                log_ctx.log_debug("Emitted meeting_failed event");
            }
        }

        // Also emit a specific mic_disconnected event for the frontend
        #[derive(Clone, Serialize)]
        struct MicDisconnectEvent {
            session_id: String,
            error_message: String,
            partial_audio_saved: bool,
        }

        let disconnect_event = MicDisconnectEvent {
            session_id: session_id.clone(),
            error_message: error_msg.clone(),
            partial_audio_saved: true, // WAV writer should have saved partial data
        };

        if let Err(e) = self.app_handle.emit("mic_disconnected", disconnect_event) {
            log_ctx.log_error(&format!("Failed to emit mic_disconnected event: {}", e));
        } else {
            log_ctx.log_debug("Emitted mic_disconnected event");
        }

        let total_time = timer.elapsed_ms();
        log_ctx.log_success_with_duration(
            total_time,
            &format!(
                "Mic disconnect handled - partial_duration={}s",
                duration.unwrap_or(0)
            ),
        );

        log_meeting_event(
            &session_id,
            "mic_disconnected",
            &format!(
                "error={} duration={}s",
                error_message,
                duration.unwrap_or(0)
            ),
        );
    }

    /// Saves the transcript to a file and updates the session status.
    ///
    /// This method:
    /// 1. Creates the transcript file in the session's folder
    /// 2. Updates the session status (Completed on success, Failed on error)
    /// 3. Stores the transcript path and optional error message
    ///
    /// # Arguments
    /// * `session_id` - The unique ID of the session
    /// * `transcript_text` - The transcribed text to save
    ///
    /// # Returns
    /// * `Ok(())` - If the transcript was saved and status updated successfully
    /// * `Err` - If file writing or database update fails
    fn save_transcript_and_update_status(
        &self,
        session_id: &str,
        transcript_text: &str,
    ) -> Result<()> {
        debug!(
            "Saving transcript for session {}: {} bytes",
            session_id,
            transcript_text.len()
        );

        // Create transcript file path: {session-id}/transcript.txt
        let transcript_filename = format!("{}/transcript.txt", session_id);
        let transcript_path = self.meetings_dir.join(&transcript_filename);

        // Write transcript to file
        fs::write(&transcript_path, transcript_text).map_err(|e| {
            anyhow::anyhow!(
                "Failed to write transcript file {:?}: {}",
                transcript_path,
                e
            )
        })?;

        info!(
            "Saved transcript to {:?} for session {}",
            transcript_path, session_id
        );

        // Update database with transcript path and Completed status
        let conn = self.get_connection()?;
        conn.execute(
            "UPDATE meeting_sessions SET transcript_path = ?1, status = ?2 WHERE id = ?3",
            params![
                transcript_filename,
                self.status_to_string(&MeetingStatus::Completed),
                session_id
            ],
        )?;

        // Update in-memory state
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(mut session) = state.current_session.take() {
                if session.id == session_id {
                    session.transcript_path = Some(transcript_filename.clone());
                    session.status = MeetingStatus::Completed;
                    state.current_session = Some(session);
                }
            }
        }

        info!(
            "Updated session {} status to Completed, transcript saved",
            session_id
        );

        Ok(())
    }

    /// Processes transcription for a meeting session.
    ///
    /// This method:
    /// 1. Reads the audio file at the given path
    /// 2. Converts WAV i16 samples to f32 format
    /// 3. Calls TranscriptionManager to perform STT
    /// 4. Returns the raw transcription text
    ///
    /// # Arguments
    /// * `audio_path` - Relative path to the audio file (e.g., "{session-id}/audio.wav")
    ///
    /// # Returns
    /// * `Ok(String)` - The transcribed text
    /// * `Err` - If file not found, reading fails, or transcription fails (including model not loaded)
    pub fn process_transcription(&self, audio_path: &str) -> Result<String> {
        debug!("Processing transcription for audio: {}", audio_path);

        // Build full path to audio file
        let full_audio_path = self.meetings_dir.join(audio_path);

        // Check if audio file exists
        if !full_audio_path.exists() {
            return Err(anyhow::anyhow!(
                "Audio file not found: {:?}",
                full_audio_path
            ));
        }

        // Read WAV file and convert to f32 samples
        let reader = WavReader::open(&full_audio_path).map_err(|e| {
            anyhow::anyhow!("Failed to open audio file {:?}: {}", full_audio_path, e)
        })?;

        // Verify audio format matches expectations (16-bit, 16000 Hz)
        let spec = reader.spec();
        if spec.bits_per_sample != 16 || spec.sample_rate != 16000 {
            return Err(anyhow::anyhow!(
                "Audio format mismatch: expected 16-bit/16000Hz, got {}/{}Hz",
                spec.bits_per_sample,
                spec.sample_rate
            ));
        }

        // Read samples and convert from i16 to f32
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .filter_map(Result::ok)
            .map(|sample| sample as f32 / i16::MAX as f32)
            .collect();

        debug!(
            "Read {} audio samples from {:?}",
            samples.len(),
            full_audio_path
        );

        if samples.is_empty() {
            return Err(anyhow::anyhow!(
                "Audio file contains no samples: {:?}",
                full_audio_path
            ));
        }

        // Call TranscriptionManager to process audio
        let transcription_text = self
            .transcription_manager
            .transcribe(samples)
            .map_err(|e| {
                anyhow::anyhow!("Transcription failed for {:?}: {}", full_audio_path, e)
            })?;

        debug!(
            "Transcription completed: {} characters",
            transcription_text.len()
        );

        Ok(transcription_text)
    }

    /// Handles app shutdown cleanup for meeting sessions.
    ///
    /// This method is called when the app is about to close. If a recording is
    /// in progress, it:
    /// 1. Stops the audio recorder gracefully
    /// 2. Finalizes the WAV file to preserve any recorded audio
    /// 3. Updates the session status to Interrupted
    /// 4. Calculates and saves the partial duration
    ///
    /// This ensures that audio is not lost on unexpected termination and the
    /// session can be recovered on next launch.
    ///
    /// # Returns
    /// * `true` if there was an active recording that was interrupted
    /// * `false` if no recording was in progress
    pub fn handle_app_shutdown(&self) -> bool {
        let timer = MeetingTimer::start();
        info!("[APP_SHUTDOWN] Handling app shutdown for meeting sessions");

        // Get current session info
        let session_info = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .current_session
                .as_ref()
                .map(|s| (s.id.clone(), s.status.clone()))
        };

        let (session_id, status) = match session_info {
            Some((id, status)) => (id, status),
            None => {
                debug!("[APP_SHUTDOWN] No active session");
                return false;
            }
        };

        let log_ctx = MeetingLogContext::new(&session_id, "handle_app_shutdown");
        log_ctx.log_start();

        // Only handle if we're currently recording
        if status != MeetingStatus::Recording {
            log_ctx.log_debug(&format!(
                "Session not recording (status: {:?}) - no cleanup needed",
                status
            ));
            return false;
        }

        log_ctx.log_warning("Interrupting active recording due to app shutdown");

        // Stop the recorder if it exists
        let recorder_timer = MeetingTimer::start();
        let mixed_recorder_opt = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.mixed_recorder.take()
        };

        if let Some(mut mixed_recorder) = mixed_recorder_opt {
            if let Err(e) = mixed_recorder.stop() {
                log_ctx.log_error(&format!("Failed to stop recorder: {}", e));
                // Continue anyway - we want to save partial audio
            } else {
                log_ctx.log_timing("recorder_stop", recorder_timer.elapsed_ms());
            }
            // Close recorder to release resources
            if let Err(e) = mixed_recorder.close() {
                log_ctx.log_warning(&format!("Failed to close recorder: {}", e));
            }
        }

        // Finalize the WAV file to ensure partial audio is saved
        let wav_timer = MeetingTimer::start();
        let wav_writer_opt = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.wav_writer.take()
        };

        if let Some(wav_handle) = wav_writer_opt {
            // Try to finalize with 5 second timeout
            if let Err(e) = wav_handle.finalize_with_timeout(Duration::from_secs(5)) {
                log_ctx.log_error(&format!("Failed to finalize WAV: {}", e));
                // Continue anyway - we still want to update status
            } else {
                log_ctx.log_timing("wav_finalize", wav_timer.elapsed_ms());
                log_ctx.log_debug("Successfully finalized partial audio");
            }
        }

        // Calculate partial duration
        let duration = {
            if let Ok(Some(session)) = self.get_session(&session_id) {
                let now = chrono::Utc::now().timestamp();
                let partial_duration = now - session.created_at;
                if partial_duration > 0 {
                    Some(partial_duration)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(dur) = duration {
            log_performance_metric(
                &session_id,
                "interrupted_recording_duration",
                dur as f64,
                "seconds",
            );
        }

        log_ctx.log_state_transition("Recording", "Interrupted");

        // Update database with Interrupted status and partial duration
        if let Ok(conn) = self.get_connection() {
            let update_result = if let Some(dur) = duration {
                conn.execute(
                    "UPDATE meeting_sessions SET status = ?1, duration = ?2, error_message = ?3 WHERE id = ?4",
                    params![
                        self.status_to_string(&MeetingStatus::Interrupted),
                        dur,
                        "Session interrupted due to app shutdown",
                        &session_id
                    ],
                )
            } else {
                conn.execute(
                    "UPDATE meeting_sessions SET status = ?1, error_message = ?2 WHERE id = ?3",
                    params![
                        self.status_to_string(&MeetingStatus::Interrupted),
                        "Session interrupted due to app shutdown",
                        &session_id
                    ],
                )
            };

            if let Err(e) = update_result {
                log_ctx.log_error(&format!("Failed to update database: {}", e));
            } else {
                log_ctx.log_debug(&format!(
                    "Updated session to Interrupted status (duration: {:?}s)",
                    duration
                ));
            }
        }

        // Clear the in-memory state
        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.current_session = None;
            state.mixed_recorder = None;
            state.wav_writer = None;
        }

        let total_time = timer.elapsed_ms();
        log_ctx.log_success_with_duration(
            total_time,
            &format!(
                "App shutdown handled - session interrupted, duration={}s",
                duration.unwrap_or(0)
            ),
        );

        log_meeting_event(
            &session_id,
            "app_shutdown_interrupted",
            &format!("duration={}s", duration.unwrap_or(0)),
        );

        true
    }

    /// Checks for interrupted sessions from previous app runs.
    ///
    /// This method queries the database for any sessions in Recording or
    /// Interrupted status (which indicate the app was closed during an
    /// active recording) and returns them for potential recovery.
    ///
    /// On startup, sessions found in Recording status are transitioned to
    /// Interrupted status since they were not properly closed.
    ///
    /// # Returns
    /// * `Ok(Vec<MeetingSession>)` - Sessions that were interrupted
    /// * `Err` - If database query fails
    pub fn check_interrupted_sessions(&self) -> Result<Vec<MeetingSession>> {
        info!("Checking for interrupted sessions from previous runs");

        let conn = self.get_connection()?;

        // First, transition any sessions in Recording status to Interrupted
        // (they were interrupted by an unclean shutdown)
        let rows_updated = conn.execute(
            "UPDATE meeting_sessions SET status = ?1, error_message = ?2 WHERE status = ?3",
            params![
                self.status_to_string(&MeetingStatus::Interrupted),
                "Session interrupted due to app shutdown (recovered on next launch)",
                self.status_to_string(&MeetingStatus::Recording),
            ],
        )?;

        if rows_updated > 0 {
            info!(
                "Transitioned {} sessions from Recording to Interrupted status",
                rows_updated
            );
        }

        // Query for all interrupted sessions
        let mut stmt = conn.prepare(
            "SELECT id, title, created_at, duration, status, audio_path, transcript_path, error_message, audio_source, summary_path, template_id
             FROM meeting_sessions WHERE status = ?1 ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map(
            params![self.status_to_string(&MeetingStatus::Interrupted)],
            |row| self.row_to_session(row),
        )?;

        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }

        if !sessions.is_empty() {
            info!(
                "Found {} interrupted session(s) that may need recovery",
                sessions.len()
            );
            for session in &sessions {
                debug!(
                    "Interrupted session: {} - {} (audio: {:?})",
                    session.id, session.title, session.audio_path
                );
            }
        } else {
            debug!("No interrupted sessions found");
        }

        Ok(sessions)
    }
}

