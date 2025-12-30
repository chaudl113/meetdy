use crate::managers::meeting::{MeetingSession, MeetingSessionManager, MeetingStatus};
use log::info;
use rusqlite::params;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

/// Starts a new meeting session recording.
///
/// This command:
/// 1. Validates no active recording is in progress
/// 2. Creates a new meeting session with UUID and folder
/// 3. Starts audio capture and incremental WAV writing
/// 4. Updates session status to Recording
///
/// # Returns
/// * `Ok(MeetingSession)` - The newly created and active session
/// * `Err(String)` - If state guard fails or recording initialization fails
#[tauri::command]
#[specta::specta]
pub fn start_meeting_session(
    app: AppHandle,
) -> Result<MeetingSession, String> {
    info!("start_meeting_session command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .start_recording()
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

    // Get current session ID from in-memory state
    let current_session = {
        let state = manager.state.lock().unwrap();
        state.current_session.clone()
    };

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

    // Update title in database
    let conn = manager
        .get_connection()
        .map_err(|e| format!("Failed to connect to database: {}", e))?;

    let rows_affected = conn
        .execute(
            "UPDATE meeting_sessions SET title = ?1 WHERE id = ?2",
            params![title, session_id],
        )
        .map_err(|e| format!("Failed to update meeting title: {}", e))?;

    if rows_affected == 0 {
        return Err(format!("Session not found: {}", session_id));
    }

    // Update in-memory state if this is the current session
    {
        let mut state = manager.state.lock().unwrap();
        if let Some(mut session) = state.current_session.as_ref() {
            if session.id == session_id {
                let mut updated_session = session.clone();
                updated_session.title = title.clone();
                state.current_session = Some(updated_session);
            }
        }
    }

    info!(
        "Updated meeting title for session {}: {}",
        session_id, title
    );

    Ok(())
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

    // Validate session is in Failed status
    if session.status != MeetingStatus::Failed {
        return Err(format!(
            "Cannot retry transcription: session is in {:?} status, expected Failed",
            session.status
        ));
    }

    // Get audio path
    let audio_path = session
        .audio_path
        .ok_or("Session has no audio file to transcribe")?;

    // Update status to Processing
    manager
        .update_session_status(&session_id, MeetingStatus::Processing)
        .map_err(|e| format!("Failed to update session status: {}", e))?;

    // Update in-memory state
    {
        let mut state = manager.state.lock().unwrap();
        if let Some(ref mut current_session) = state.current_session {
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
                if let Err(e) =
                    manager_clone.save_transcript_and_update_status(&session_id_clone, &transcript)
                {
                    // Failed to save transcript
                    let error_msg = format!("Failed to save transcript: {}", e);
                    let _ = manager_clone
                        .update_session_status_with_error(&session_id_clone, MeetingStatus::Failed, &error_msg);

                    // Update in-memory state
                    {
                        let mut state = manager_clone.state.lock().unwrap();
                        if let Some(ref mut session) = state.current_session {
                            if session.id == session_id_clone {
                                session.status = MeetingStatus::Failed;
                                session.error_message = Some(error_msg.clone());
                            }
                        }
                    }

                    // Emit failed event
                    if let Some(updated_session) = manager_clone.get_session(&session_id_clone).ok().flatten() {
                        let _ = app_clone.emit("meeting_failed", &updated_session);
                    }
                } else {
                    // Success - emit completed event
                    if let Some(updated_session) = manager_clone.get_session(&session_id_clone).ok().flatten() {
                        let _ = app_clone.emit("meeting_completed", &updated_session);
                    }
                }
            }
            Err(e) => {
                // Transcription failed
                let error_msg = format!("Transcription failed: {}", e);
                let _ = manager_clone
                    .update_session_status_with_error(&session_id_clone, MeetingStatus::Failed, &error_msg);

                // Update in-memory state
                {
                    let mut state = manager_clone.state.lock().unwrap();
                    if let Some(ref mut session) = state.current_session {
                        if session.id == session_id_clone {
                            session.status = MeetingStatus::Failed;
                            session.error_message = Some(error_msg.clone());
                        }
                    }
                }

                // Emit failed event
                if let Some(updated_session) = manager_clone.get_session(&session_id_clone).ok().flatten() {
                    let _ = app_clone.emit("meeting_failed", &updated_session);
                }
            }
        }
    });

    info!("Retry transcription initiated for session: {}", session_id);

    Ok(())
}
