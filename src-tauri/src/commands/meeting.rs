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
