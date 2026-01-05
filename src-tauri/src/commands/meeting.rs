use crate::managers::meeting::{AudioSourceType, MeetingSession, MeetingSessionManager, MeetingStatus};
use log::info;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Starts a new meeting session recording.
///
/// This command:
/// 1. Validates no active recording is in progress
/// 2. Creates a new meeting session with UUID and folder
/// 3. Starts audio capture with the specified source
/// 4. Updates session status to Recording
///
/// # Arguments
/// * `audio_source` - The audio source configuration (microphone_only, system_only, or mixed)
///
/// # Returns
/// * `Ok(MeetingSession)` - The newly created and active session
/// * `Err(String)` - If state guard fails or recording initialization fails
#[tauri::command]
#[specta::specta]
pub fn start_meeting_session(
    app: AppHandle,
    audio_source: Option<AudioSourceType>,
) -> Result<MeetingSession, String> {
    let source = audio_source.unwrap_or_default();
    info!("start_meeting_session command called with audio_source: {:?}", source);

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .start_recording(source)
        .map_err(|e| format!("Failed to start meeting session: {}", e))
}

/// Stops the current meeting session recording.
///
/// This command:
/// 1. Validates current session is in Recording state
/// 2. Stops audio capture
/// 3. Finalizes WAV file
/// 4. Updates session status to Processing
/// 5. Spawns background transcription task
///
/// # Returns
/// * `Ok(String)` - The relative path to the audio file (e.g., "{session-id}/audio.wav")
/// * `Err(String)` - If no recording is active or stopping fails
#[tauri::command]
#[specta::specta]
pub fn stop_meeting_session(app: AppHandle) -> Result<String, String> {
    info!("stop_meeting_session command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .stop_recording()
        .map_err(|e| format!("Failed to stop meeting session: {}", e))
}

/// Gets the current meeting status.
///
/// Returns the status of the currently active session, if any.
///
/// # Returns
/// * `Some(MeetingStatus)` - The current session status if a session exists
/// * `None` - If no active session
#[tauri::command]
#[specta::specta]
pub fn get_meeting_status(app: AppHandle) -> Option<MeetingStatus> {
    info!("get_meeting_status command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager.get_current_status()
}

/// Gets the current active meeting session.
///
/// Returns full details of the currently active session, if any.
///
/// # Returns
/// * `Ok(Some(MeetingSession))` - The current session if active
/// * `Ok(None)` - If no active session
/// * `Err(String)` - If database query fails
#[tauri::command]
#[specta::specta]
pub fn get_current_meeting(app: AppHandle) -> Result<Option<MeetingSession>, String> {
    info!("get_current_meeting command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();

    // Get current session from in-memory state
    let current_session = manager.get_current_session();

    // If no current session, return None
    let session_id = match current_session {
        Some(session) => session.id,
        None => return Ok(None),
    };

    // Retrieve full session details from database
    manager
        .get_session(&session_id)
        .map_err(|e| format!("Failed to get current meeting: {}", e))
}

/// Updates the title of a meeting session.
///
/// Updates the title in the database. The title can be edited at any time
/// after the session is created.
///
/// # Arguments
/// * `session_id` - The unique ID of the session to update
/// * `title` - The new title for the session
///
/// # Returns
/// * `Ok(())` - If the title was updated successfully
/// * `Err(String)` - If session not found or database update fails
#[tauri::command]
#[specta::specta]
pub fn update_meeting_title(
    app: AppHandle,
    session_id: String,
    title: String,
) -> Result<(), String> {
    info!(
        "update_meeting_title command called: session_id={}, title={}",
        session_id, title
    );

    let manager = app.state::<Arc<MeetingSessionManager>>();

    // Validate title is not empty
    if title.trim().is_empty() {
        return Err("Title cannot be empty".to_string());
    }

    // Update title using the manager's public method
    manager
        .update_session_title(&session_id, &title)
        .map_err(|e| format!("Failed to update meeting title: {}", e))
}

/// Retries transcription for a failed meeting session.
///
/// This command:
/// 1. Validates the session exists and is in Failed status
/// 2. Updates status to Processing
/// 3. Spawns background transcription task
///
/// # Arguments
/// * `session_id` - The unique ID of the session to retry
///
/// # Returns
/// * `Ok(())` - If retry was initiated successfully
/// * `Err(String)` - If session not found, not in Failed status, or retry fails
#[tauri::command]
#[specta::specta]
pub fn retry_transcription(app: AppHandle, session_id: String) -> Result<(), String> {
    info!(
        "retry_transcription command called for session: {}",
        session_id
    );

    let manager = app.state::<Arc<MeetingSessionManager>>();

    // Get session from database
    let session = manager
        .get_session(&session_id)
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Validate session is in a retryable status (Failed, Interrupted, or Completed)
    match session.status {
        MeetingStatus::Failed | MeetingStatus::Interrupted | MeetingStatus::Completed => {
            // OK to retry
        }
        _ => {
            return Err(format!(
                "Cannot retry transcription: session is in {:?} status, expected Failed, Interrupted, or Completed",
                session.status
            ));
        }
    }

    // Use the manager's retry method to prepare for transcription
    let audio_path = manager
        .retry_transcription_for_session(&session_id)
        .map_err(|e| format!("Failed to prepare retry: {}", e))?;

    // Emit processing event
    let _ = app.emit("meeting_processing", &session);

    // Spawn background transcription task
    let manager_clone = Arc::clone(&manager);
    let session_id_clone = session_id.clone();
    let audio_path_clone = audio_path.clone();
    let app_clone = app.clone();

    std::thread::spawn(move || {
        match manager_clone.process_transcription(&audio_path_clone) {
            Ok(transcript) => {
                // Save transcript and update status to Completed
                if let Err(e) = manager_clone.save_transcript(&session_id_clone, &transcript) {
                    // Failed to save transcript
                    let error_msg = format!("Failed to save transcript: {}", e);
                    let _ = manager_clone.update_session_status_with_error(
                        &session_id_clone,
                        MeetingStatus::Failed,
                        &error_msg,
                    );

                    // Update in-memory state
                    manager_clone.set_session_error(&session_id_clone, &error_msg);

                    // Emit failed event
                    if let Some(updated_session) =
                        manager_clone.get_session(&session_id_clone).ok().flatten()
                    {
                        let _ = app_clone.emit("meeting_failed", &updated_session);
                    }
                } else {
                    // Success - emit completed event
                    if let Some(updated_session) =
                        manager_clone.get_session(&session_id_clone).ok().flatten()
                    {
                        let _ = app_clone.emit("meeting_completed", &updated_session);
                    }
                }
            }
            Err(e) => {
                // Transcription failed
                let error_msg = format!("Transcription failed: {}", e);
                let _ = manager_clone.update_session_status_with_error(
                    &session_id_clone,
                    MeetingStatus::Failed,
                    &error_msg,
                );

                // Update in-memory state
                manager_clone.set_session_error(&session_id_clone, &error_msg);

                // Emit failed event
                if let Some(updated_session) =
                    manager_clone.get_session(&session_id_clone).ok().flatten()
                {
                    let _ = app_clone.emit("meeting_failed", &updated_session);
                }
            }
        }
    });

    info!("Retry transcription initiated for session: {}", session_id);

    Ok(())
}

/// Gets the transcript text content for a completed meeting session.
///
/// Reads the transcript file from disk and returns its content.
///
/// # Arguments
/// * `session_id` - The unique ID of the session to get transcript for
///
/// # Returns
/// * `Ok(Some(String))` - The transcript text if available
/// * `Ok(None)` - If no transcript exists for this session
/// * `Err(String)` - If session not found or file read fails
#[tauri::command]
#[specta::specta]
pub fn get_meeting_transcript(app: AppHandle, session_id: String) -> Result<Option<String>, String> {
    info!(
        "get_meeting_transcript command called for session: {}",
        session_id
    );

    let manager = app.state::<Arc<MeetingSessionManager>>();

    // Get session from database
    let session = manager
        .get_session(&session_id)
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Check if transcript path exists
    let transcript_path = match session.transcript_path {
        Some(path) => path,
        None => return Ok(None),
    };

    // Read transcript file
    let meetings_dir = manager.get_meetings_dir();
    let full_path = meetings_dir.join(&transcript_path);

    if !full_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read transcript file: {}", e))?;

    Ok(Some(content))
}

/// Lists all meeting sessions.
///
/// Returns all meeting sessions from the database, ordered by creation time
/// (newest first).
///
/// # Returns
/// * `Ok(Vec<MeetingSession>)` - All meeting sessions
/// * `Err(String)` - If database query fails
#[tauri::command]
#[specta::specta]
pub fn list_meeting_sessions(app: AppHandle) -> Result<Vec<MeetingSession>, String> {
    info!("list_meeting_sessions command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .list_sessions()
        .map_err(|e| format!("Failed to list meeting sessions: {}", e))
}

/// Gets the path to the meetings directory.
///
/// # Returns
/// * `Ok(String)` - The absolute path to the meetings directory
/// * `Err(String)` - If getting the path fails
#[tauri::command]
#[specta::specta]
pub fn get_meetings_directory(app: AppHandle) -> Result<String, String> {
    info!("get_meetings_directory command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    Ok(manager.get_meetings_dir().to_string_lossy().to_string())
}

/// Deletes a meeting session and its associated files.
///
/// This command:
/// 1. Validates the session exists
/// 2. Deletes the session folder (audio, transcript files)
/// 3. Removes the session from the database
///
/// # Arguments
/// * `session_id` - The unique ID of the session to delete
///
/// # Returns
/// * `Ok(())` - If the session was deleted successfully
/// * `Err(String)` - If session not found or deletion fails
#[tauri::command]
#[specta::specta]
pub fn delete_meeting_session(app: AppHandle, session_id: String) -> Result<(), String> {
    info!("delete_meeting_session command called for session: {}", session_id);

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .delete_session(&session_id)
        .map_err(|e| format!("Failed to delete meeting session: {}", e))
}
