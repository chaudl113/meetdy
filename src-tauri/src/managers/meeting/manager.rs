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
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{path::BaseDirectory, AppHandle, Emitter, Manager};
use tempfile::NamedTempFile;
use uuid::Uuid;

use crate::audio_toolkit::{
    vad::{SmoothedVad, VadFrame},
    AudioSourceConfig, MixedAudioRecorder, SileroVad, VoiceActivityDetector,
};
use crate::managers::diarization::SpeakerDiarizationManager;
use crate::managers::meeting_logger::{
    log_meeting_event, log_performance_metric, MeetingLogContext, MeetingTimer,
};
use crate::settings::get_settings;

use super::db::{get_connection, init_meeting_database};
use super::models::{
    ActionItem, ActionItemStatus, AudioSourceType, KeyPoint, MeetingManagerState, MeetingNote,
    MeetingSession, MeetingStatus, Participant, Tag, TranscriptSegment,
};
use super::wav_writer::WavWriterHandle;

const MEETING_SESSION_SELECT: &str = "id, title, created_at, duration, status, audio_path, transcript_path, error_message, audio_source, summary_path, template_id, stt_engine, funasr_base_url, funasr_model, transcription_language";

#[derive(Default)]
struct MeetingPauseState {
    is_paused: bool,
    paused_started_at: Option<i64>,
    total_paused_secs: i64,
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
    /// Transcription manager for STT processing
    transcription_manager: Arc<crate::managers::transcription::TranscriptionManager>,
    /// Cumulative live transcripts keyed by session id. Used to complete a
    /// recording immediately on stop without re-running full-file STT.
    live_transcripts: Arc<Mutex<HashMap<String, String>>>,
    pause_state: Arc<Mutex<MeetingPauseState>>,
    /// Currently active speaker: (participant_id, timestamp_secs).
    /// None = no speaker assigned. Updated by set_active_speaker command.
    active_speaker: Arc<Mutex<Option<(String, f32)>>>,
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
            live_transcripts: Arc::new(Mutex::new(HashMap::new())),
            pause_state: Arc::new(Mutex::new(MeetingPauseState::default())),
            active_speaker: Arc::new(Mutex::new(None)),
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

    /// Returns `true` if a recording is currently in progress.
    pub fn is_recording(&self) -> bool {
        let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state
            .current_session
            .as_ref()
            .map(|s| s.status == MeetingStatus::Recording)
            .unwrap_or(false)
    }

    /// Sets the active speaker for the current recording session.
    /// All transcript segments emitted after this call will be attributed to the
    /// given participant until another call changes the active speaker.
    ///
    /// `timestamp_secs` should be the current recording duration in seconds.
    pub fn set_active_speaker(&self, participant_id: String, timestamp_secs: f32) {
        if let Ok(mut speaker) = self.active_speaker.lock() {
            *speaker = Some((participant_id, timestamp_secs));
        }
    }

    /// Clears the active speaker (marks subsequent segments as unassigned).
    pub fn clear_active_speaker(&self) {
        if let Ok(mut speaker) = self.active_speaker.lock() {
            *speaker = None;
        }
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

    /// Stores the audio file path for an imported/external session.
    pub fn set_audio_path_for_session(&self, session_id: &str, audio_path: &str) -> Result<()> {
        let conn = self.get_connection()?;
        let rows_affected = conn.execute(
            "UPDATE meeting_sessions SET audio_path = ?1 WHERE id = ?2",
            params![audio_path, session_id],
        )?;
        if rows_affected == 0 {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }
        info!("Set audio path for session {}: {}", session_id, audio_path);
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
        if let Err(update_err) =
            self.update_session_status_with_error(session_id, MeetingStatus::Failed, error_msg)
        {
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
        get_connection(&self.db_path)
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
        self.create_session_with_stt_config(audio_source, "whisper".to_string(), None, None, None)
    }

    fn create_session_with_stt_config(
        &self,
        audio_source: AudioSourceType,
        stt_engine: String,
        funasr_base_url: Option<String>,
        funasr_model: Option<String>,
        transcription_language: Option<String>,
    ) -> Result<MeetingSession> {
        let id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().timestamp();
        let title = self.format_meeting_title(created_at);

        // Create the session folder
        let session_dir = self.meetings_dir.join(&id);
        fs::create_dir_all(&session_dir)?;
        debug!("Created session folder: {:?}", session_dir);

        // Create the session object
        let mut session = MeetingSession::new_with_audio_source(
            id.clone(),
            title.clone(),
            created_at,
            audio_source.clone(),
        );
        session.stt_engine = stt_engine;
        session.funasr_base_url = funasr_base_url;
        session.funasr_model = funasr_model;
        session.transcription_language = transcription_language;

        // Insert into database
        let conn = self.get_connection()?;
        conn.execute(
            "INSERT INTO meeting_sessions (id, title, created_at, status, audio_source, template_id, stt_engine, funasr_base_url, funasr_model, transcription_language) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                session.id,
                session.title,
                session.created_at,
                self.status_to_string(&session.status),
                self.audio_source_to_string(&audio_source),
                session.template_id,
                session.stt_engine,
                session.funasr_base_url,
                session.funasr_model,
                session.transcription_language
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
        let query = format!(
            "SELECT {} FROM meeting_sessions WHERE id = ?1",
            MEETING_SESSION_SELECT
        );
        let session = conn
            .query_row(&query, params![session_id], |row| self.row_to_session(row))
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
        let query = format!(
            "SELECT {} FROM meeting_sessions ORDER BY created_at DESC",
            MEETING_SESSION_SELECT
        );
        let mut stmt = conn.prepare(&query)?;

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

    /// Deletes multiple meeting sessions by their IDs.
    pub fn delete_sessions(&self, session_ids: &[String]) -> Result<()> {
        info!("Deleting {} meeting sessions", session_ids.len());

        for session_id in session_ids {
            // Delete session folder if it exists
            let session_folder = self.meetings_dir.join(session_id);
            if session_folder.exists() {
                fs::remove_dir_all(&session_folder)?;
            }
        }

        // Delete from database in a single transaction
        let conn = self.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare("DELETE FROM meeting_sessions WHERE id = ?1")?;
            for session_id in session_ids {
                stmt.execute(params![session_id])?;
            }
        }
        tx.commit()?;

        info!("Deleted {} meeting sessions", session_ids.len());
        Ok(())
    }

    /// Deletes every meeting session and all managed meeting files.
    pub fn delete_all_sessions(&self) -> Result<()> {
        info!("Deleting all meeting sessions");

        {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state
                .current_session
                .as_ref()
                .map(|session| {
                    session.status == MeetingStatus::Recording
                        || session.status == MeetingStatus::Processing
                })
                .unwrap_or(false)
            {
                return Err(anyhow::anyhow!(
                    "Cannot clear history while a meeting is recording or processing"
                ));
            }
        }

        if self.meetings_dir.exists() {
            fs::remove_dir_all(&self.meetings_dir)?;
        }
        fs::create_dir_all(&self.meetings_dir)?;

        let conn = self.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        for table in [
            "transcript_segments",
            "meeting_tags",
            "meeting_participants",
            "meeting_key_points",
            "meeting_action_items",
            "meeting_notes",
            "meeting_sessions",
        ] {
            tx.execute(&format!("DELETE FROM {}", table), [])?;
        }
        tx.commit()?;

        self.live_transcripts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();

        {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state.current_session = None;
        }

        info!("Deleted all meeting sessions and files");
        Ok(())
    }

    // --- Notes -----------------------------------------------------------

    /// Adds a note to the given meeting session.
    ///
    /// Generates a new UUID for the note and records the current timestamp.
    /// The note is persisted via the meeting_notes table.
    pub fn add_note(
        &self,
        session_id: &str,
        timestamp_seconds: i64,
        content: String,
    ) -> Result<MeetingNote> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("Note content cannot be empty"));
        }

        // Ensure the parent session exists; surface a clean error otherwise so
        // the FK constraint failure doesn't bubble up as an opaque SQLite error.
        if self.get_session(session_id)?.is_none() {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }

        let note = MeetingNote {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            timestamp_seconds: timestamp_seconds.max(0),
            content: trimmed.to_string(),
            author: None,
            created_at: chrono::Utc::now().timestamp(),
        };

        super::db::insert_note(&self.db_path, &note)?;
        info!(
            "Added note {} to session {} at t={}s",
            note.id, session_id, note.timestamp_seconds
        );
        Ok(note)
    }

    /// Returns the notes attached to a meeting session, ordered chronologically.
    pub fn list_notes(&self, session_id: &str) -> Result<Vec<MeetingNote>> {
        super::db::list_notes_by_session(&self.db_path, session_id)
    }

    /// Deletes a note by id. Returns Ok(()) if the note existed, error otherwise.
    pub fn delete_note(&self, note_id: &str) -> Result<()> {
        let deleted = super::db::delete_note(&self.db_path, note_id)?;
        if !deleted {
            return Err(anyhow::anyhow!("Note not found: {}", note_id));
        }
        info!("Deleted note {}", note_id);
        Ok(())
    }

    // --- Action items ----------------------------------------------------

    pub fn add_action_item(
        &self,
        session_id: &str,
        task: String,
        assignee: Option<String>,
        due_date: Option<String>,
        status: ActionItemStatus,
    ) -> Result<ActionItem> {
        let task = task.trim().to_string();
        if task.is_empty() {
            return Err(anyhow::anyhow!("Task cannot be empty"));
        }
        if self.get_session(session_id)?.is_none() {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }
        let existing = super::db::list_action_items_by_session(&self.db_path, session_id)?;
        let next_order = existing.iter().map(|i| i.sort_order).max().unwrap_or(-1) + 1;
        let item = ActionItem {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            task,
            assignee: assignee.filter(|s| !s.trim().is_empty()),
            due_date: due_date.filter(|s| !s.trim().is_empty()),
            status,
            sort_order: next_order,
            created_at: chrono::Utc::now().timestamp(),
        };
        super::db::insert_action_item(&self.db_path, &item)?;
        Ok(item)
    }

    pub fn list_action_items(&self, session_id: &str) -> Result<Vec<ActionItem>> {
        super::db::list_action_items_by_session(&self.db_path, session_id)
    }

    pub fn update_action_item(&self, item: &ActionItem) -> Result<()> {
        super::db::update_action_item(&self.db_path, item)
    }

    pub fn delete_action_item(&self, id: &str) -> Result<()> {
        let deleted = super::db::delete_action_item(&self.db_path, id)?;
        if !deleted {
            return Err(anyhow::anyhow!("Action item not found: {}", id));
        }
        Ok(())
    }

    // --- Key points ------------------------------------------------------

    pub fn add_key_point(
        &self,
        session_id: &str,
        category: Option<String>,
        content: String,
    ) -> Result<KeyPoint> {
        let content = content.trim().to_string();
        if content.is_empty() {
            return Err(anyhow::anyhow!("Key point cannot be empty"));
        }
        if self.get_session(session_id)?.is_none() {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }
        let existing = super::db::list_key_points_by_session(&self.db_path, session_id)?;
        let next_order = existing.iter().map(|i| i.sort_order).max().unwrap_or(-1) + 1;
        let kp = KeyPoint {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            category: category.filter(|s| !s.trim().is_empty()),
            content,
            sort_order: next_order,
            created_at: chrono::Utc::now().timestamp(),
        };
        super::db::insert_key_point(&self.db_path, &kp)?;
        Ok(kp)
    }

    pub fn list_key_points(&self, session_id: &str) -> Result<Vec<KeyPoint>> {
        super::db::list_key_points_by_session(&self.db_path, session_id)
    }

    // --- Participants ----------------------------------------------------

    pub fn add_participant(
        &self,
        session_id: &str,
        name: String,
        role: Option<String>,
    ) -> Result<Participant> {
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(anyhow::anyhow!("Participant name cannot be empty"));
        }
        if self.get_session(session_id)?.is_none() {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }
        let existing = super::db::list_participants_by_session(&self.db_path, session_id)?;
        let next_order = existing.iter().map(|p| p.sort_order).max().unwrap_or(-1) + 1;
        let p = Participant {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            name,
            role: role.filter(|s| !s.trim().is_empty()),
            sort_order: next_order,
            created_at: chrono::Utc::now().timestamp(),
            color_index: -1,
        };
        super::db::insert_participant(&self.db_path, &p)?;
        Ok(p)
    }

    pub fn list_participants(&self, session_id: &str) -> Result<Vec<Participant>> {
        super::db::list_participants_by_session(&self.db_path, session_id)
    }

    pub fn update_participant(&self, p: &Participant) -> Result<()> {
        super::db::update_participant(&self.db_path, p)
    }

    pub fn delete_participant(&self, id: &str) -> Result<()> {
        let deleted = super::db::delete_participant(&self.db_path, id)?;
        if !deleted {
            return Err(anyhow::anyhow!("Participant not found: {}", id));
        }
        Ok(())
    }

    // --- Tags ------------------------------------------------------------

    pub fn add_tag(&self, session_id: &str, label: String, color: Option<String>) -> Result<Tag> {
        let label = label.trim().to_string();
        if label.is_empty() {
            return Err(anyhow::anyhow!("Tag label cannot be empty"));
        }
        if self.get_session(session_id)?.is_none() {
            return Err(anyhow::anyhow!("Session not found: {}", session_id));
        }
        let t = Tag {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.to_string(),
            label,
            color: color.filter(|s| !s.trim().is_empty()),
            created_at: chrono::Utc::now().timestamp(),
        };
        super::db::insert_tag(&self.db_path, &t)?;
        Ok(t)
    }

    pub fn list_tags(&self, session_id: &str) -> Result<Vec<Tag>> {
        super::db::list_tags_by_session(&self.db_path, session_id)
    }

    pub fn delete_tag(&self, id: &str) -> Result<()> {
        let deleted = super::db::delete_tag(&self.db_path, id)?;
        if !deleted {
            return Err(anyhow::anyhow!("Tag not found: {}", id));
        }
        Ok(())
    }

    pub fn list_all_tag_labels(&self) -> Result<Vec<String>> {
        super::db::list_all_tag_labels(&self.db_path)
    }

    /// Replaces all AI-extracted insights for a session.
    /// Wipes previous action items/key points/participants (but keeps
    /// manually-added tags) and inserts the new ones.
    pub fn replace_insights(
        &self,
        session_id: &str,
        action_items: Vec<ActionItem>,
        key_points: Vec<KeyPoint>,
        participants: Vec<Participant>,
    ) -> Result<()> {
        super::db::delete_action_items_by_session(&self.db_path, session_id)?;
        super::db::delete_key_points_by_session(&self.db_path, session_id)?;
        super::db::delete_participants_by_session(&self.db_path, session_id)?;
        for item in action_items {
            super::db::insert_action_item(&self.db_path, &item)?;
        }
        for kp in key_points {
            super::db::insert_key_point(&self.db_path, &kp)?;
        }
        for p in participants {
            super::db::insert_participant(&self.db_path, &p)?;
        }
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
            stt_engine: row
                .get("stt_engine")
                .unwrap_or_else(|_| "whisper".to_string()),
            funasr_base_url: row.get("funasr_base_url").unwrap_or(None),
            funasr_model: row.get("funasr_model").unwrap_or(None),
            transcription_language: row.get("transcription_language").unwrap_or(None),
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
    /// * `stt_engine` - STT engine to use: "whisper" (local), "soniox" (cloud), or "funasr" (local service)
    /// * `soniox_api_key` - Soniox API key, required when stt_engine = "soniox"
    ///
    /// # Returns
    /// * `Ok(MeetingSession)` - The newly created and active session
    /// * `Err` - If state guard fails, session creation, recorder initialization, or audio capture fails
    pub fn start_recording(
        &self,
        audio_source: AudioSourceType,
        stt_engine: String,
        soniox_api_key: Option<String>,
        source_language: Option<String>,
        target_language: Option<String>,
    ) -> Result<MeetingSession> {
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

        let settings = get_settings(&self.app_handle);
        let funasr_base_url = if stt_engine == "funasr" {
            Some(settings.funasr_base_url.clone())
        } else {
            None
        };
        let funasr_model = if stt_engine == "funasr" {
            Some(settings.funasr_model.clone())
        } else {
            None
        };

        // Create a new session with the specified audio and STT config.
        let session = self.create_session_with_stt_config(
            audio_source.clone(),
            stt_engine.clone(),
            funasr_base_url,
            funasr_model,
            source_language.clone(),
        )?;
        {
            let mut pause = self.pause_state.lock().unwrap_or_else(|p| p.into_inner());
            *pause = MeetingPauseState::default();
        }
        self.live_transcripts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(session.id.clone(), String::new());

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

        // Live transcript event payload. Emitted while recording as chunks are
        // transcribed; the final transcript is still produced after stop.
        #[derive(Clone, Serialize)]
        struct MeetingLiveTranscriptPayload {
            session_id: String,
            text: String,
            chunk_text: String,
            is_final: bool,
            // --- Speaker attribution (new) ---
            /// Participant ID of the active speaker. None = unassigned.
            speaker_id: Option<String>,
            /// Segment start time in milliseconds.
            start_ms: i64,
            /// Segment end time in milliseconds.
            end_ms: i64,
        }

        let (live_tx, live_rx) = mpsc::channel::<Vec<f32>>();

        // --- Soniox cloud STT path ---
        if stt_engine == "soniox" {
            let api_key = soniox_api_key.clone().unwrap_or_default();
            let soniox_source_lang = source_language.clone();
            let soniox_target_lang = target_language.clone();
            let app_handle_soniox = self.app_handle.clone();
            let session_id_soniox = session.id.clone();
            let active_speaker_soniox = self.active_speaker.clone();
            let manager_soniox = self.clone();
            let live_transcripts_soniox = self.live_transcripts.clone();
            thread::spawn(move || {
                // Build a single-threaded tokio runtime for this worker thread
                let rt = match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("Soniox: failed to build tokio runtime: {}", e);
                        return;
                    }
                };

                rt.block_on(async move {
                    use futures_util::{SinkExt, StreamExt};
                    use tokio_tungstenite::{connect_async, tungstenite::Message};

                    const SONIOX_ENDPOINT: &str =
                        "wss://stt-rt.soniox.com/transcribe-websocket";

                    #[derive(serde::Serialize, Clone)]
                    struct SttErrorPayload {
                        session_id: String,
                        message: String,
                    }

                    let emit_error = |app: &AppHandle, sid: &str, msg: String| {
                        log::error!("Soniox STT error: {}", msg);
                        let _ = app.emit(
                            "meeting_stt_error",
                            SttErrorPayload {
                                session_id: sid.to_string(),
                                message: msg,
                            },
                        );
                    };

                    let ws = match connect_async(SONIOX_ENDPOINT).await {
                        Ok((ws, _)) => ws,
                        Err(e) => {
                            emit_error(
                                &app_handle_soniox,
                                &session_id_soniox,
                                format!("Cannot connect to Soniox: {}", e),
                            );
                            return;
                        }
                    };
                    let (mut ws_sink, mut ws_stream) = ws.split();

                    let mut config_msg = serde_json::json!({
                        "api_key": api_key,
                        "model": "stt-rt-v4",
                        "audio_format": "pcm_s16le",
                        "sample_rate": 16000,
                        "num_channels": 1,
                        "enable_endpoint_detection": true,
                        "max_endpoint_delay_ms": 3000,
                        "enable_speaker_diarization": true,
                    });

                    // Language hints (skip "auto" — let Soniox detect)
                    if let Some(ref lang) = soniox_source_lang {
                        if !lang.is_empty() && lang != "auto" {
                            config_msg["language_hints"] = serde_json::json!([lang]);
                        }
                    }

                    // Realtime translation
                    if let Some(ref tgt) = soniox_target_lang {
                        if !tgt.is_empty() {
                            config_msg["translation"] = serde_json::json!({
                                "type": "one_way",
                                "target_language": tgt,
                            });
                        }
                    }

                    if let Err(e) = ws_sink
                        .send(Message::Text(config_msg.to_string().into()))
                        .await
                    {
                        emit_error(
                            &app_handle_soniox,
                            &session_id_soniox,
                            format!("Failed to send config to Soniox: {}", e),
                        );
                        return;
                    }

                    // Channel to bridge std mpsc → tokio
                    let (pcm_tx, mut pcm_rx) =
                        tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

                    // Spawn a blocking reader thread that converts f32 samples → i16 PCM
                    // and forwards them into the async channel.
                    {
                        let pcm_tx = pcm_tx.clone();
                        let should_continue = {
                            let manager = manager_soniox.clone();
                            let sid = session_id_soniox.clone();
                            move || {
                                let state =
                                    manager.state.lock().unwrap_or_else(|p| p.into_inner());
                                state
                                    .current_session
                                    .as_ref()
                                    .map(|s| {
                                        s.id == sid
                                            && s.status == MeetingStatus::Recording
                                    })
                                    .unwrap_or(false)
                            }
                        };
                        thread::spawn(move || loop {
                            match live_rx.recv_timeout(Duration::from_millis(500)) {
                                Ok(samples) => {
                                    // Convert f32 [-1,1] to i16 LE PCM
                                    let pcm: Vec<u8> = samples
                                        .iter()
                                        .flat_map(|&s| {
                                            let v = (s * 32767.0)
                                                .clamp(-32768.0, 32767.0)
                                                as i16;
                                            v.to_le_bytes()
                                        })
                                        .collect();
                                    if pcm_tx.send(pcm).is_err() {
                                        break;
                                    }
                                }
                                Err(mpsc::RecvTimeoutError::Timeout) => {
                                    if !should_continue() {
                                        break;
                                    }
                                }
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                        });
                    }

                    let mut full_text = String::new();
                    let mut transcript_parts: Vec<String> = Vec::new();
                    // Maps Soniox speaker label (e.g. "S1") → participant id in DB
                    let mut speaker_map: std::collections::HashMap<String, String> =
                        std::collections::HashMap::new();

                    loop {
                        tokio::select! {
                            biased;

                            pcm = pcm_rx.recv() => {
                                match pcm {
                                    Some(bytes) => {
                                        if let Err(e) = ws_sink
                                            .send(Message::Binary(bytes.into()))
                                            .await
                                        {
                                            log::warn!("Soniox: send audio error: {}", e);
                                            break;
                                        }
                                    }
                                    None => {
                                        // Converter thread finished — flush + close
                                        let _ = ws_sink
                                            .send(Message::Binary(vec![].into()))
                                            .await;
                                        let _ = ws_sink.send(Message::Close(None)).await;
                                        break;
                                    }
                                }
                            }

                            msg = ws_stream.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        // Parse Soniox token response
                                        if let Ok(value) =
                                            serde_json::from_str::<serde_json::Value>(&text)
                                        {
                                            // Soniox error response
                                            if let Some(code) = value.get("error_code") {
                                                let msg = value
                                                    .get("error_message")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("Unknown error");
                                                emit_error(
                                                    &app_handle_soniox,
                                                    &session_id_soniox,
                                                    format!("Soniox error {}: {}", code, msg),
                                                );
                                                break;
                                            }
                                            if let Some(tokens) =
                                                value.get("tokens").and_then(|v| v.as_array())
                                            {
                                                let mut chunk_text = String::new();
                                                let mut has_end = false;
                                                let mut soniox_speaker: Option<String> = None;
                                                for token in tokens {
                                                    let tok = token
                                                        .get("text")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("");
                                                    if tok == "<end>" {
                                                        has_end = true;
                                                        continue;
                                                    }
                                                    let is_final = token
                                                        .get("is_final")
                                                        .and_then(|v| v.as_bool())
                                                        .unwrap_or(false);
                                                    if is_final {
                                                        chunk_text.push_str(tok);
                                                        if soniox_speaker.is_none() {
                                                            soniox_speaker = token
                                                                .get("speaker")
                                                                .and_then(|v| v.as_str())
                                                                .map(|s| s.to_string());
                                                        }
                                                    }
                                                }

                                                // Resolve Soniox speaker label → participant id
                                                let resolved_speaker_id: Option<String> =
                                                    if let Some(ref label) = soniox_speaker {
                                                        if let Some(pid) = speaker_map.get(label) {
                                                            Some(pid.clone())
                                                        } else {
                                                            // Create a new participant for this speaker
                                                            let participant_name =
                                                                format!("Speaker {}", label);
                                                            match manager_soniox.add_participant(
                                                                &session_id_soniox,
                                                                participant_name,
                                                                None,
                                                            ) {
                                                                Ok(p) => {
                                                                    // Emit event so FE updates participant list
                                                                    let _ = app_handle_soniox.emit(
                                                                        "meeting_participant_added",
                                                                        p.clone(),
                                                                    );
                                                                    speaker_map.insert(
                                                                        label.clone(),
                                                                        p.id.clone(),
                                                                    );
                                                                    Some(p.id)
                                                                }
                                                                Err(e) => {
                                                                    log::warn!(
                                                                        "Soniox: failed to create participant for {}: {}",
                                                                        label, e
                                                                    );
                                                                    None
                                                                }
                                                            }
                                                        }
                                                    } else {
                                                        // Fall back to manually set active speaker
                                                        active_speaker_soniox
                                                            .lock()
                                                            .ok()
                                                            .and_then(|s| {
                                                                s.as_ref().map(|(id, _)| id.clone())
                                                            })
                                                    };

                                                if !chunk_text.is_empty() {
                                                    if !full_text.is_empty() {
                                                        full_text.push(' ');
                                                    }
                                                    full_text.push_str(&chunk_text);
                                                    transcript_parts.push(chunk_text.clone());
                                                    live_transcripts_soniox
                                                        .lock()
                                                        .unwrap_or_else(|p| p.into_inner())
                                                        .insert(
                                                            session_id_soniox.clone(),
                                                            full_text.clone(),
                                                        );
                                                    #[derive(Clone, serde::Serialize)]
                                                    struct LivePayload {
                                                        session_id: String,
                                                        text: String,
                                                        chunk_text: String,
                                                        is_final: bool,
                                                        speaker_id: Option<String>,
                                                        start_ms: i64,
                                                        end_ms: i64,
                                                    }
                                                    let payload = LivePayload {
                                                        session_id: session_id_soniox.clone(),
                                                        text: full_text.clone(),
                                                        chunk_text,
                                                        is_final: has_end,
                                                        speaker_id: resolved_speaker_id,
                                                        start_ms: 0,
                                                        end_ms: 0,
                                                    };
                                                     // Persist segment to DB (best-effort)
                                                    {
                                                        let db_path = manager_soniox.db_path.clone();
                                                        let seg_session = session_id_soniox.clone();
                                                        let seg_speaker = payload.speaker_id.clone();
                                                        let seg_text = payload.chunk_text.clone();
                                                        thread::spawn(move || {
                                                            let segment = TranscriptSegment {
                                                                id: uuid::Uuid::new_v4().to_string(),
                                                                meeting_id: seg_session,
                                                                start_ms: 0,
                                                                end_ms: 0,
                                                                text: seg_text,
                                                                speaker_id: seg_speaker,
                                                                sequence: 0,
                                                                created_at: chrono::Utc::now().timestamp_millis(),
                                                            };
                                                            match rusqlite::Connection::open(&db_path) {
                                                                Ok(conn) => {
                                                                    if let Err(e) = super::db::insert_transcript_segment(&conn, &segment) {
                                                                        log::warn!("Soniox: failed to save segment: {}", e);
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    log::warn!("Soniox: DB open failed: {}", e);
                                                                }
                                                            }
                                                        });
                                                    }
                                                    if let Err(e) = app_handle_soniox
                                                        .emit("meeting_live_transcript", payload)
                                                    {
                                                        log::warn!(
                                                            "Soniox: emit transcript failed: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) | None => break,
                                    Some(Err(e)) => {
                                        emit_error(
                                            &app_handle_soniox,
                                            &session_id_soniox,
                                            format!("Soniox connection lost: {}", e),
                                        );
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                });
            });
        } else if stt_engine == "funasr" {
            // FunASR's managed OpenAI-compatible server is batch-oriented. For
            // meeting UX we run near-realtime chunk transcription against the
            // same local server while still doing a final full-file pass after
            // stop_recording().
            let manager = self.clone();
            let app_handle = self.app_handle.clone();
            let session_id = session.id.clone();
            let active_speaker_clone = self.active_speaker.clone();
            let base_url = session
                .funasr_base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8000".to_string());
            let model = session
                .funasr_model
                .clone()
                .unwrap_or_else(|| "fun-asr-nano".to_string());
            let language = session.transcription_language.clone();

            thread::spawn(move || {
                const SAMPLE_RATE: usize = 16_000;
                const CHUNK_SAMPLES: usize = SAMPLE_RATE * 8;
                const MIN_CHUNK_SAMPLES: usize = SAMPLE_RATE * 2;
                const MIN_CHUNK_RMS: f32 = 0.003;

                let should_continue = || {
                    let state = manager.state.lock().unwrap_or_else(|p| p.into_inner());
                    state
                        .current_session
                        .as_ref()
                        .map(|s| s.id == session_id && s.status == MeetingStatus::Recording)
                        .unwrap_or(false)
                };

                let write_chunk_wav = |samples: &[f32]| -> Result<NamedTempFile> {
                    let temp_file = NamedTempFile::with_suffix(".wav")?;
                    let spec = WavSpec {
                        channels: 1,
                        sample_rate: 16000,
                        bits_per_sample: 16,
                        sample_format: hound::SampleFormat::Int,
                    };
                    {
                        let mut writer = WavWriter::new(temp_file.reopen()?, spec)?;
                        for sample in samples {
                            let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                            writer.write_sample(value)?;
                        }
                        writer.finalize()?;
                    }
                    Ok(temp_file)
                };

                let transcribe_chunk =
                    |samples: Vec<f32>,
                     start_ms: i64,
                     end_ms: i64,
                     transcript_parts: &mut Vec<String>| {
                        if samples.len() < MIN_CHUNK_SAMPLES {
                            return;
                        }

                        let rms = (samples.iter().map(|s| s * s).sum::<f32>()
                            / samples.len() as f32)
                            .sqrt();
                        if rms < MIN_CHUNK_RMS {
                            log::debug!(
                                "[FUNASR_STT] live chunk skipped low energy: session={} rms={:.4} samples={}",
                                session_id,
                                rms,
                                samples.len()
                            );
                            return;
                        }

                        let temp_wav = match write_chunk_wav(&samples) {
                            Ok(file) => file,
                            Err(e) => {
                                log::warn!("[FUNASR_STT] failed to write live chunk wav: {}", e);
                                return;
                            }
                        };

                        log::info!(
                            "[FUNASR_STT] session={} live chunk start: {:.1}s rms={:.4}",
                            session_id,
                            samples.len() as f32 / SAMPLE_RATE as f32,
                            rms
                        );

                        let text = match tauri::async_runtime::block_on(
                            crate::funasr_client::transcribe_file(
                                &app_handle,
                                &base_url,
                                &model,
                                language.as_deref(),
                                temp_wav.path(),
                            ),
                        ) {
                            Ok(text) => text.trim().to_string(),
                            Err(e) => {
                                log::warn!("[FUNASR_STT] live chunk failed: {}", e);
                                let _ = app_handle.emit(
                                    "meeting_stt_error",
                                    serde_json::json!({
                                        "session_id": session_id,
                                        "message": format!("FunASR transcription error: {}", e)
                                    }),
                                );
                                return;
                            }
                        };

                        if text.is_empty() {
                            return;
                        }

                        transcript_parts.push(text.clone());
                        let full_text = transcript_parts.join(" ");
                        manager
                            .live_transcripts
                            .lock()
                            .unwrap_or_else(|p| p.into_inner())
                            .insert(session_id.clone(), full_text.clone());

                        let payload = MeetingLiveTranscriptPayload {
                            session_id: session_id.clone(),
                            text: full_text,
                            chunk_text: text.clone(),
                            is_final: false,
                            speaker_id: active_speaker_clone
                                .lock()
                                .ok()
                                .and_then(|s| s.as_ref().map(|(id, _)| id.clone())),
                            start_ms,
                            end_ms,
                        };

                        {
                            let db_path = manager.db_path.clone();
                            let seg_session = session_id.clone();
                            let seg_speaker = payload.speaker_id.clone();
                            let seg_text = payload.chunk_text.clone();
                            thread::spawn(move || {
                                let segment = TranscriptSegment {
                                    id: uuid::Uuid::new_v4().to_string(),
                                    meeting_id: seg_session,
                                    start_ms,
                                    end_ms,
                                    text: seg_text,
                                    speaker_id: seg_speaker,
                                    sequence: 0,
                                    created_at: chrono::Utc::now().timestamp_millis(),
                                };
                                match rusqlite::Connection::open(&db_path) {
                                    Ok(conn) => {
                                        if let Err(e) =
                                            super::db::insert_transcript_segment(&conn, &segment)
                                        {
                                            log::warn!(
                                                "[FUNASR_STT] failed to save live segment: {}",
                                                e
                                            );
                                        }
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "[FUNASR_STT] failed to open DB for live segment: {}",
                                            e
                                        );
                                    }
                                }
                            });
                        }

                        if let Err(e) = app_handle.emit("meeting_live_transcript", payload) {
                            log::warn!("[FUNASR_STT] failed to emit live transcript: {}", e);
                        } else {
                            log::info!(
                                "[FUNASR_STT] session={} live chunk completed: chars={}",
                                session_id,
                                text.chars().count()
                            );
                        }
                    };

                let mut pending = Vec::<f32>::new();
                let mut transcript_parts = Vec::<String>::new();
                let mut timeline_samples = 0usize;

                loop {
                    match live_rx.recv_timeout(Duration::from_millis(500)) {
                        Ok(samples) => {
                            pending.extend(samples);
                            while pending.len() >= CHUNK_SAMPLES {
                                let chunk: Vec<f32> = pending.drain(..CHUNK_SAMPLES).collect();
                                let start_ms = (timeline_samples as f64 / SAMPLE_RATE as f64
                                    * 1000.0)
                                    .round() as i64;
                                timeline_samples += chunk.len();
                                let end_ms = (timeline_samples as f64 / SAMPLE_RATE as f64 * 1000.0)
                                    .round() as i64;
                                transcribe_chunk(chunk, start_ms, end_ms, &mut transcript_parts);
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if !should_continue() {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }

                if pending.len() >= MIN_CHUNK_SAMPLES {
                    let start_ms =
                        (timeline_samples as f64 / SAMPLE_RATE as f64 * 1000.0).round() as i64;
                    timeline_samples += pending.len();
                    let end_ms =
                        (timeline_samples as f64 / SAMPLE_RATE as f64 * 1000.0).round() as i64;
                    transcribe_chunk(pending, start_ms, end_ms, &mut transcript_parts);
                }

                if !transcript_parts.is_empty() {
                    let payload = MeetingLiveTranscriptPayload {
                        session_id,
                        text: transcript_parts.join(" "),
                        chunk_text: String::new(),
                        is_final: true,
                        speaker_id: active_speaker_clone
                            .lock()
                            .ok()
                            .and_then(|s| s.as_ref().map(|(id, _)| id.clone())),
                        start_ms: 0,
                        end_ms: 0,
                    };
                    if let Err(e) = app_handle.emit("meeting_live_transcript", payload) {
                        log::warn!("[FUNASR_STT] failed to emit final live transcript: {}", e);
                    }
                }
            });
        } else {
            // --- Whisper (local) STT path ---
            let manager = self.clone();
            let app_handle = self.app_handle.clone();
            let session_id = session.id.clone();
            let active_speaker_clone = self.active_speaker.clone();
            let vad_path = self
                .app_handle
                .path()
                .resolve(
                    "resources/models/silero_vad_v4.onnx",
                    BaseDirectory::Resource,
                )
                .map_err(|e| anyhow::anyhow!("Failed to resolve VAD path: {}", e))?;
            thread::spawn(move || {
                const SAMPLE_RATE: usize = 16_000;
                const VAD_FRAME_SAMPLES: usize = SAMPLE_RATE * 30 / 1000;
                // Minimum utterance length to send to Whisper. Too short and
                // we waste cycles on noise; too long and live feel suffers.
                const MIN_FINAL_SAMPLES: usize = SAMPLE_RATE * 8 / 10;
                const MAX_UTTERANCE_SAMPLES: usize = SAMPLE_RATE * 10;
                // RMS gate applied per VAD frame BEFORE Silero. If a 30ms
                // frame is quieter than this, force it to Noise. Prevents
                // Silero false-positives on background hiss.
                // Lowered from 0.01 to accommodate built-in mic without headphones
                // (voice captured further from mic, lower signal level).
                const FRAME_RMS_GATE: f32 = 0.004;
                // Minimum mean RMS over an entire utterance to actually
                // send it to Whisper. Lowered from 0.012 for same reason.
                const UTTERANCE_RMS_MIN: f32 = 0.005;

                let mut input_pending = Vec::<f32>::new();
                let mut utterance = Vec::<f32>::new();
                let mut transcript_parts = Vec::<String>::new();

                // SmoothedVad params tuned for live UX:
                //  - prefill 8 frames (~240ms) so utterance start isn't clipped
                //  - hangover 15 frames (~450ms) so brief pauses don't split words
                //  - onset 3 frames (~90ms) to start quickly but avoid 1-frame
                //    glitches firing utterances on noise
                // Silero threshold lowered from 0.55 to 0.35 so speech is detected
                // even when signal is weaker (no headphones, built-in mic).
                let mut live_vad = match SileroVad::new(&vad_path, 0.35)
                    .map(|silero| SmoothedVad::new(Box::new(silero), 8, 15, 3))
                {
                    Ok(vad) => Some(vad),
                    Err(e) => {
                        log::warn!(
                            "Meeting live VAD unavailable, falling back to fixed chunks: {}",
                            e
                        );
                        None
                    }
                };

                let should_continue = || {
                    let state = manager.state.lock().unwrap_or_else(|p| p.into_inner());
                    state
                        .current_session
                        .as_ref()
                        .map(|s| s.id == session_id && s.status == MeetingStatus::Recording)
                        .unwrap_or(false)
                };

                let transcribe_chunk = |audio: Vec<f32>, transcript_parts: &mut Vec<String>| {
                    if audio.is_empty() {
                        return;
                    }

                    let rms =
                        (audio.iter().map(|s| s * s).sum::<f32>() / audio.len() as f32).sqrt();
                    if rms < UTTERANCE_RMS_MIN {
                        log::debug!(
                            "Live: dropping low-energy utterance (rms={:.4}, samples={})",
                            rms,
                            audio.len()
                        );
                        return;
                    }

                    log::info!(
                        "Live: flushing utterance ({} samples, {:.1}s, rms={:.4})",
                        audio.len(),
                        audio.len() as f32 / 16_000.0,
                        rms
                    );

                    match manager.transcription_manager.transcribe_live(audio) {
                        Ok(text) => {
                            let chunk_text = text.trim().to_string();
                            if chunk_text.is_empty() {
                                return;
                            }
                            if chunk_text
                                .chars()
                                .all(|c| c.is_ascii_punctuation() || c.is_whitespace())
                            {
                                return;
                            }
                            let lower = chunk_text.to_lowercase();
                            if matches!(lower.as_str(), "music" | "thank you" | "you") {
                                return;
                            }
                            transcript_parts.push(chunk_text.clone());
                            let full_text = transcript_parts.join(" ");
                            manager
                                .live_transcripts
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .insert(session_id.clone(), full_text.clone());
                            let payload = MeetingLiveTranscriptPayload {
                                session_id: session_id.clone(),
                                text: full_text,
                                chunk_text,
                                is_final: false,
                                speaker_id: active_speaker_clone
                                    .lock()
                                    .ok()
                                    .and_then(|s| s.as_ref().map(|(id, _)| id.clone())),
                                start_ms: 0,
                                end_ms: 0,
                            };
                            // Save segment to DB asynchronously (best-effort, don't block audio thread)
                            {
                                let db_path = manager.db_path.clone();
                                let seg_session = session_id.clone();
                                let seg_speaker = payload.speaker_id.clone();
                                let seg_text = payload.chunk_text.clone();
                                let seg_start = payload.start_ms;
                                let seg_end = payload.end_ms;
                                std::thread::spawn(move || {
                                    let segment = TranscriptSegment {
                                        id: uuid::Uuid::new_v4().to_string(),
                                        meeting_id: seg_session,
                                        start_ms: seg_start,
                                        end_ms: seg_end,
                                        text: seg_text,
                                        speaker_id: seg_speaker,
                                        sequence: 0,
                                        created_at: chrono::Utc::now().timestamp_millis(),
                                    };
                                    match rusqlite::Connection::open(&db_path) {
                                        Ok(conn) => {
                                            if let Err(e) = super::db::insert_transcript_segment(
                                                &conn, &segment,
                                            ) {
                                                log::warn!(
                                                    "Failed to save transcript segment: {}",
                                                    e
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "Failed to open DB for transcript segment: {}",
                                                e
                                            );
                                        }
                                    }
                                });
                            }
                            if let Err(e) = app_handle.emit("meeting_live_transcript", payload) {
                                log::warn!("Failed to emit meeting_live_transcript: {}", e);
                            }
                        }
                        Err(e) => {
                            log::warn!("Live transcript chunk failed: {}", e);
                        }
                    }
                };

                let flush_utterance =
                    |utterance: &mut Vec<f32>, transcript_parts: &mut Vec<String>| {
                        if utterance.len() >= MIN_FINAL_SAMPLES {
                            let audio = std::mem::take(utterance);
                            transcribe_chunk(audio, transcript_parts);
                        } else {
                            utterance.clear();
                        }
                    };

                loop {
                    match live_rx.recv_timeout(Duration::from_millis(500)) {
                        Ok(samples) => {
                            input_pending.extend(samples);
                            if let Some(vad) = live_vad.as_mut() {
                                while input_pending.len() >= VAD_FRAME_SAMPLES {
                                    let frame: Vec<f32> =
                                        input_pending.drain(..VAD_FRAME_SAMPLES).collect();
                                    // Cheap RMS gate before invoking Silero:
                                    // skip truly quiet frames entirely so
                                    // Silero can't false-positive on hiss.
                                    // While in an ongoing utterance we still
                                    // feed the frame so SmoothedVad's
                                    // hangover ends naturally.
                                    let frame_rms = (frame.iter().map(|s| s * s).sum::<f32>()
                                        / frame.len() as f32)
                                        .sqrt();
                                    if frame_rms < FRAME_RMS_GATE && utterance.is_empty() {
                                        continue;
                                    }
                                    match vad.push_frame(&frame) {
                                        Ok(VadFrame::Speech(speech)) => {
                                            utterance.extend_from_slice(speech);
                                            // Hard-cap very long utterances so
                                            // we don't make the user wait
                                            // forever for a flush.
                                            if utterance.len() >= MAX_UTTERANCE_SAMPLES {
                                                flush_utterance(
                                                    &mut utterance,
                                                    &mut transcript_parts,
                                                );
                                            }
                                        }
                                        Ok(VadFrame::Noise) => {
                                            // SmoothedVad already applied
                                            // its hangover; flush the
                                            // utterance now (end-of-speech).
                                            if !utterance.is_empty() {
                                                flush_utterance(
                                                    &mut utterance,
                                                    &mut transcript_parts,
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "Meeting live VAD failed, using fixed chunks: {}",
                                                e
                                            );
                                            live_vad = None;
                                            break;
                                        }
                                    }
                                }
                            }

                            if live_vad.is_none() {
                                while input_pending.len() >= SAMPLE_RATE * 5 {
                                    let chunk: Vec<f32> =
                                        input_pending.drain(..SAMPLE_RATE * 5).collect();
                                    transcribe_chunk(chunk, &mut transcript_parts);
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            if !should_continue() {
                                break;
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }

                if !input_pending.is_empty() {
                    utterance.extend(input_pending);
                }
                if !utterance.is_empty() {
                    flush_utterance(&mut utterance, &mut transcript_parts);
                }

                if !transcript_parts.is_empty() {
                    let payload = MeetingLiveTranscriptPayload {
                        session_id,
                        text: transcript_parts.join(" "),
                        chunk_text: String::new(),
                        is_final: true,
                        speaker_id: active_speaker_clone
                            .lock()
                            .ok()
                            .and_then(|s| s.as_ref().map(|(id, _)| id.clone())),
                        start_ms: 0,
                        end_ms: 0,
                    };
                    if let Err(e) = app_handle.emit("meeting_live_transcript", payload) {
                        log::warn!("Failed to emit final meeting_live_transcript: {}", e);
                    }
                }
            });
        }

        // Add sample callback for incremental WAV writing and live transcript
        // chunking. Sending to the live worker is non-blocking enough for the
        // unbounded std channel and avoids doing Whisper work on the audio
        // callback path.
        let wav_handle_clone = wav_handle.clone();
        let pause_state_for_samples = self.pause_state.clone();
        let sample_callback = move |samples: Vec<f32>| {
            if pause_state_for_samples
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_paused
            {
                return;
            }
            if let Err(e) = wav_handle_clone.write_samples(&samples) {
                error!("Failed to write audio samples: {}", e);
            }
            let _ = live_tx.send(samples);
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

        // Forward live audio statistics to the frontend. The callback is
        // invoked on the audio worker thread at ~10Hz; we emit a Tauri event
        // so the RecordingView can render real-time RMS / peak / SNR.
        #[derive(Clone, Serialize)]
        struct MeetingAudioStatsPayload {
            session_id: String,
            rms: f32,
            peak: f32,
            snr_db: f32,
            noise_floor_db: f32,
            audio_source: AudioSourceType,
        }

        let stats_app_handle = self.app_handle.clone();
        let stats_session_id = session.id.clone();
        let stats_audio_source = session.audio_source.clone();
        mixed_recorder = mixed_recorder.with_audio_stats_callback(move |stats| {
            let payload = MeetingAudioStatsPayload {
                session_id: stats_session_id.clone(),
                rms: stats.rms,
                peak: stats.peak,
                snr_db: stats.snr_db,
                noise_floor_db: stats.noise_floor_db,
                audio_source: stats_audio_source.clone(),
            };
            if let Err(e) = stats_app_handle.emit("meeting_audio_stats", payload) {
                log::warn!("Failed to emit meeting_audio_stats: {}", e);
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

        let now_ts = chrono::Utc::now().timestamp();
        let paused_secs = {
            let mut pause = self.pause_state.lock().unwrap_or_else(|p| p.into_inner());
            if pause.is_paused {
                if let Some(started_at) = pause.paused_started_at.take() {
                    pause.total_paused_secs += now_ts - started_at;
                }
                pause.is_paused = false;
            }
            pause.total_paused_secs
        };
        let duration = now_ts - current_session.created_at - paused_secs;
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

        let live_transcript = self
            .live_transcripts
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&session_id)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        if let Some(live_transcript) = live_transcript {
            let should_complete_from_live = current_session.stt_engine != "funasr";
            if !should_complete_from_live {
                log_ctx.log_debug(
                    "Live FunASR transcript exists; keeping session in processing so the final batch pass can refine transcript.txt",
                );
            } else {
                log_ctx.log_state_transition("Recording", "Completed");

                // Persist duration first; save_transcript_and_update_status will
                // persist transcript_path + Completed status. This skips the old
                // Processing state because live transcript already produced the
                // transcript while recording.
                let conn = self.get_connection()?;
                conn.execute(
                    "UPDATE meeting_sessions SET duration = ?1 WHERE id = ?2",
                    params![duration, session_id],
                )?;

                {
                    let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
                    if let Some(session) = state.current_session.as_mut() {
                        session.duration = Some(duration);
                    }
                }

                let session_for_stopped = self.get_session(&session_id)?.ok_or_else(|| {
                    anyhow::anyhow!(
                        "Session {} not found when emitting meeting_stopped",
                        session_id
                    )
                })?;

                if let Err(e) = self
                    .app_handle
                    .emit("meeting_stopped", session_for_stopped.clone())
                {
                    log_ctx.log_error(&format!("Failed to emit meeting_stopped event: {}", e));
                } else {
                    log_ctx.log_debug("Emitted meeting_stopped event");
                }

                self.save_transcript_and_update_status(&session_id, &live_transcript)?;

                if let Ok(Some(session_data)) = self.get_session(&session_id) {
                    if let Err(e) = self
                        .app_handle
                        .emit("meeting_completed", session_data.clone())
                    {
                        log_ctx
                            .log_error(&format!("Failed to emit meeting_completed event: {}", e));
                    } else {
                        log_ctx.log_debug("Emitted meeting_completed event from live transcript");
                    }
                }

                self.live_transcripts
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&session_id);

                let total_time = timer.elapsed_ms();
                log_ctx.log_success_with_duration(
                    total_time,
                    &format!(
                        "Recording stopped using live transcript - duration={}s, audio={}",
                        duration, audio_path_opt
                    ),
                );

                log_meeting_event(
                    &session_id,
                    "recording_stopped_live_completed",
                    &format!("duration={}s path={}", duration, audio_path_opt),
                );

                return Ok(audio_path_opt);
            }
        }

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

        if session_for_event.stt_engine == "funasr" {
            info!(
                "[FUNASR_STT] session={} queued for batch transcription after stop: model={} base_url={}",
                session_id,
                session_for_event
                    .funasr_model
                    .as_deref()
                    .unwrap_or("fun-asr-nano"),
                session_for_event
                    .funasr_base_url
                    .as_deref()
                    .unwrap_or("http://localhost:8000")
            );
        }

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
            match manager_clone.process_transcription(&session_id_clone, &audio_path_clone) {
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
                        if let Ok(Some(session_data)) = manager_clone.get_session(&session_id_clone)
                        {
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

            manager_clone
                .live_transcripts
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&session_id_clone);
        });

        Ok(audio_path_opt)
    }

    pub fn pause_recording(&self) -> Result<MeetingSession> {
        let session = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .current_session
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Cannot pause recording: no active session"))?
        };

        if session.status != MeetingStatus::Recording {
            return Err(anyhow::anyhow!(
                "Cannot pause recording: session is not recording"
            ));
        }

        {
            let mut pause = self.pause_state.lock().unwrap_or_else(|p| p.into_inner());
            if !pause.is_paused {
                pause.is_paused = true;
                pause.paused_started_at = Some(chrono::Utc::now().timestamp());
            }
        }

        if let Err(e) = self.app_handle.emit("meeting_paused", session.clone()) {
            log::warn!("Failed to emit meeting_paused: {}", e);
        }

        Ok(session)
    }

    pub fn resume_recording(&self) -> Result<MeetingSession> {
        let session = {
            let state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .current_session
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Cannot resume recording: no active session"))?
        };

        if session.status != MeetingStatus::Recording {
            return Err(anyhow::anyhow!(
                "Cannot resume recording: session is not recording"
            ));
        }

        {
            let mut pause = self.pause_state.lock().unwrap_or_else(|p| p.into_inner());
            if pause.is_paused {
                if let Some(started_at) = pause.paused_started_at.take() {
                    pause.total_paused_secs += chrono::Utc::now().timestamp() - started_at;
                }
                pause.is_paused = false;
            }
        }

        if let Err(e) = self.app_handle.emit("meeting_resumed", session.clone()) {
            log::warn!("Failed to emit meeting_resumed: {}", e);
        }

        Ok(session)
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
    /// Like `process_transcription` but allows overriding the model and language for one run.
    pub fn process_transcription_with_override(
        &self,
        session_id: &str,
        audio_path: &str,
        model_id: Option<&str>,
        language: Option<&str>,
    ) -> Result<String> {
        debug!(
            "process_transcription_with_override: session={} model={:?} language={:?}",
            session_id, model_id, language
        );
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

        let full_audio_path = self.meetings_dir.join(audio_path);
        if !full_audio_path.exists() {
            return Err(anyhow::anyhow!(
                "Audio file not found: {:?}",
                full_audio_path
            ));
        }

        let reader = WavReader::open(&full_audio_path).map_err(|e| {
            anyhow::anyhow!("Failed to open audio file {:?}: {}", full_audio_path, e)
        })?;
        let spec = reader.spec();
        if spec.bits_per_sample != 16 || spec.sample_rate != 16000 {
            return Err(anyhow::anyhow!(
                "Audio format mismatch: expected 16-bit/16000Hz, got {}/{}Hz",
                spec.bits_per_sample,
                spec.sample_rate
            ));
        }
        let samples: Vec<f32> = reader
            .into_samples::<i16>()
            .filter_map(Result::ok)
            .map(|s| s as f32 / i16::MAX as f32)
            .collect();
        if samples.is_empty() {
            return Err(anyhow::anyhow!("Audio file contains no samples"));
        }

        // If this is a funasr session, ignore overrides and fall back to standard path.
        if session.stt_engine == "funasr" {
            return self.process_transcription(session_id, audio_path);
        }

        let transcription_text = self
            .transcription_manager
            .transcribe_with_override(samples, model_id, language)
            .map_err(|e| {
                anyhow::anyhow!("Transcription failed for {:?}: {}", full_audio_path, e)
            })?;

        // Run diarization if enabled
        let settings = get_settings(&self.app_handle);
        if settings.diarization_enabled && session.stt_engine != "soniox" {
            if let Some(dm) = self
                .app_handle
                .try_state::<std::sync::Arc<SpeakerDiarizationManager>>()
            {
                if dm.is_available() {
                    match dm.process(&full_audio_path) {
                        Ok(segments) => {
                            let _ = self.save_diarization_segments(session_id, &segments);
                            let formatted = self
                                .format_transcript_with_speakers(&transcription_text, &segments);
                            let _ = self.app_handle.emit(
                                "diarization_completed",
                                serde_json::json!({ "session_id": session_id }),
                            );
                            return Ok(formatted);
                        }
                        Err(e) => {
                            log::warn!(
                                "[diarization] override run failed for {}: {}",
                                session_id,
                                e
                            );
                        }
                    }
                }
            }
        }

        Ok(transcription_text)
    }

    pub fn process_transcription(&self, session_id: &str, audio_path: &str) -> Result<String> {
        debug!("Processing transcription for audio: {}", audio_path);
        let session = self
            .get_session(session_id)?
            .ok_or_else(|| anyhow::anyhow!("Session not found: {}", session_id))?;

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

        // Diagnostic: audio level summary to identify silent-input regressions
        // ("Music"/"Thank you" hallucinations typically mean Whisper saw silence).
        if !samples.is_empty() {
            let mut sum_sq = 0.0f64;
            let mut peak = 0.0f32;
            let mut nonzero = 0usize;
            for &s in &samples {
                sum_sq += (s as f64) * (s as f64);
                let a = s.abs();
                if a > peak {
                    peak = a;
                }
                if s != 0.0 {
                    nonzero += 1;
                }
            }
            let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
            let duration_s = samples.len() as f32 / 16_000.0;
            log::info!(
                "[transcribe-audio] {:?}: samples={} duration={:.2}s peak={:.4} rms={:.4} nonzero_pct={:.1}%",
                full_audio_path.file_name().unwrap_or_default(),
                samples.len(),
                duration_s,
                peak,
                rms,
                100.0 * nonzero as f32 / samples.len() as f32,
            );
        }

        if samples.is_empty() {
            return Err(anyhow::anyhow!(
                "Audio file contains no samples: {:?}",
                full_audio_path
            ));
        }

        if session.stt_engine == "funasr" {
            let full_audio_path_clone = full_audio_path.clone();
            let base_url = session
                .funasr_base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8000".to_string());
            let model = session
                .funasr_model
                .clone()
                .unwrap_or_else(|| "fun-asr-nano".to_string());
            let language = session.transcription_language.clone();

            log::info!(
                "[FUNASR_STT] session={} starting batch transcription: model={} base_url={} audio={}",
                session.id,
                model,
                base_url,
                full_audio_path_clone.display()
            );

            let transcript = tauri::async_runtime::block_on(crate::funasr_client::transcribe_file(
                &self.app_handle,
                &base_url,
                &model,
                language.as_deref(),
                &full_audio_path_clone,
            ))
            .map_err(|e| {
                anyhow::anyhow!(
                    "FunASR transcription failed for {:?}: {}. Start FunASR with: funasr-server --model {} --device cpu --port 8000",
                    full_audio_path,
                    e,
                    model
                )
            })?;

            log::info!(
                "[FUNASR_STT] session={} completed batch transcription: chars={}",
                session.id,
                transcript.chars().count()
            );

            return Ok(transcript);
        }

        // Ensure the transcription model is loaded before processing.
        // The model may have been unloaded by the idle-timeout since recording stopped.
        if !self.transcription_manager.is_model_loaded() {
            info!(
                "[process_transcription] model not loaded, initiating load for session {}",
                session_id
            );
            self.transcription_manager.initiate_model_load();
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

        // Run speaker diarization if enabled and models are available.
        // Skip for engines that provide their own diarization (soniox, funasr).
        let settings = get_settings(&self.app_handle);
        if settings.diarization_enabled
            && session.stt_engine != "soniox"
            && session.stt_engine != "funasr"
        {
            if let Some(dm) = self
                .app_handle
                .try_state::<std::sync::Arc<SpeakerDiarizationManager>>()
            {
                if dm.is_available() {
                    match dm.process(&full_audio_path) {
                        Ok(segments) => {
                            if let Err(e) = self.save_diarization_segments(session_id, &segments) {
                                log::warn!(
                                    "[diarization] failed to save segments for {}: {}",
                                    session_id,
                                    e
                                );
                            }
                            let formatted = self
                                .format_transcript_with_speakers(&transcription_text, &segments);
                            if let Err(e) = self.app_handle.emit(
                                "diarization_completed",
                                serde_json::json!({ "session_id": session_id }),
                            ) {
                                log::warn!(
                                    "[diarization] failed to emit event for {}: {}",
                                    session_id,
                                    e
                                );
                            }
                            return Ok(formatted);
                        }
                        Err(e) => {
                            log::warn!(
                                "[diarization] processing failed for {}, using raw transcript: {}",
                                session_id,
                                e
                            );
                        }
                    }
                }
            }
        }

        Ok(transcription_text)
    }

    /// Assigns speaker IDs to existing transcript segments by overlap mapping,
    /// and creates participant entries for each detected speaker.
    ///
    /// For each transcript segment already in the DB, finds the diarization
    /// segment with the maximum time overlap and assigns its speaker.
    /// Falls back to inserting bare time-range segments when no transcript
    /// segments exist yet (e.g. batch audio with no prior segments).
    fn save_diarization_segments(
        &self,
        session_id: &str,
        diar_segments: &[crate::managers::diarization::DiarizationSegment],
    ) -> Result<()> {
        use super::db::{
            insert_participant, insert_transcript_segment, list_transcript_segments,
            update_participant_color_index, update_segment_speaker,
        };

        if diar_segments.is_empty() {
            return Ok(());
        }

        let conn = self.get_connection()?;
        let now_ms = chrono::Utc::now().timestamp_millis();

        // Collect unique speaker IDs and create participant entries.
        let mut speaker_ids: Vec<usize> = diar_segments.iter().map(|s| s.speaker_id).collect();
        speaker_ids.sort_unstable();
        speaker_ids.dedup();

        // Color palette indices cycle through 0-7.
        for (color_idx, &spk) in speaker_ids.iter().enumerate() {
            let participant_id = format!("diar_{}_{}", session_id, spk);
            let p = Participant {
                id: participant_id.clone(),
                session_id: session_id.to_string(),
                name: format!("Speaker {}", spk + 1),
                role: None,
                sort_order: spk as i64,
                created_at: now_ms,
                color_index: (color_idx % 8) as i64,
            };
            // insert_participant uses db_path; ignore duplicate errors gracefully.
            let _ = insert_participant(&self.db_path, &p);
            let _ = update_participant_color_index(&conn, &participant_id, (color_idx % 8) as i64);
        }

        // Try to map onto existing transcript segments first.
        let transcript_segments = list_transcript_segments(&conn, session_id)?;

        if transcript_segments.is_empty() {
            // No transcript segments yet — insert bare time-range segments.
            for (i, seg) in diar_segments.iter().enumerate() {
                let participant_id = format!("diar_{}_{}", session_id, seg.speaker_id);
                let ts = TranscriptSegment {
                    id: uuid::Uuid::new_v4().to_string(),
                    meeting_id: session_id.to_string(),
                    start_ms: (seg.start_sec * 1000.0) as i64,
                    end_ms: (seg.end_sec * 1000.0) as i64,
                    text: String::new(),
                    speaker_id: Some(participant_id),
                    sequence: i as i64,
                    created_at: now_ms,
                };
                insert_transcript_segment(&conn, &ts)?;
            }
            return Ok(());
        }

        // Map each transcript segment to the diarization segment with the
        // greatest millisecond overlap.
        for ts in &transcript_segments {
            if let Some(idx) = crate::managers::diarization::best_speaker_for_segment(
                ts.start_ms,
                ts.end_ms,
                diar_segments,
            ) {
                let participant_id =
                    format!("diar_{}_{}", session_id, diar_segments[idx].speaker_id);
                update_segment_speaker(&conn, &ts.id, Some(&participant_id))?;
            }
        }

        Ok(())
    }

    /// Formats transcript text with speaker labels derived from diarization segments.
    ///
    /// Walks the transcript lines and prepends a "[Speaker N]" label whenever
    /// the speaker changes, using the same overlap heuristic as `save_diarization_segments`.
    fn format_transcript_with_speakers(
        &self,
        transcript: &str,
        diar_segments: &[crate::managers::diarization::DiarizationSegment],
    ) -> String {
        if diar_segments.is_empty() || transcript.is_empty() {
            return transcript.to_string();
        }

        // Split transcript into non-empty lines and assign a speaker to each.
        let lines: Vec<&str> = transcript.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.is_empty() {
            return transcript.to_string();
        }

        // Estimate each line's time position by distributing evenly over the
        // full audio duration reported by the last diarization segment.
        let total_sec = diar_segments.iter().map(|s| s.end_sec).fold(0.0_f32, f32::max);
        let line_duration = if total_sec > 0.0 { total_sec / lines.len() as f32 } else { 1.0 };

        let mut output = String::with_capacity(transcript.len() + lines.len() * 16);
        let mut last_speaker = usize::MAX;

        for (i, line) in lines.iter().enumerate() {
            let line_mid = (i as f32 + 0.5) * line_duration;
            // Find diarization segment whose interval contains line_mid.
            let speaker = diar_segments
                .iter()
                .find(|d| d.start_sec <= line_mid && line_mid < d.end_sec)
                .or_else(|| {
                    // Fallback: closest segment by midpoint distance.
                    diar_segments.iter().min_by(|a, b| {
                        let mid_a = (a.start_sec + a.end_sec) / 2.0;
                        let mid_b = (b.start_sec + b.end_sec) / 2.0;
                        (mid_a - line_mid)
                            .abs()
                            .partial_cmp(&(mid_b - line_mid).abs())
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                })
                .map(|d| d.speaker_id)
                .unwrap_or(0);

            if speaker != last_speaker {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&format!("[Speaker {}]\n", speaker + 1));
                last_speaker = speaker;
            }
            output.push_str(line);
            output.push('\n');
        }

        output
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
        let query = format!(
            "SELECT {} FROM meeting_sessions WHERE status = ?1 ORDER BY created_at DESC",
            MEETING_SESSION_SELECT
        );
        let mut stmt = conn.prepare(&query)?;

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
