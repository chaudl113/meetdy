use crate::managers::meeting::{AudioSourceType, MeetingSession, MeetingSessionManager, MeetingStatus};
use crate::settings::get_settings;
use log::{debug, info};
use std::path::{Component, Path};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Maximum transcript size in bytes (1MB) to prevent OOM and LLM context overflow
const MAX_TRANSCRIPT_SIZE: u64 = 1024 * 1024;

/// Validates that a relative path is safe and doesn't escape the base directory.
/// Prevents path traversal attacks (e.g., "../../../etc/passwd").
///
/// This function validates both existing and non-existing paths by checking
/// the parent directory for non-existing files.
fn validate_safe_path(base_dir: &Path, relative_path: &str) -> Result<std::path::PathBuf, String> {
    let path = Path::new(relative_path);

    // Reject absolute paths
    if path.is_absolute() {
        return Err("Absolute paths are not allowed".to_string());
    }

    // Check path components for dangerous elements
    for component in path.components() {
        match component {
            Component::ParentDir => {
                return Err("Path traversal (parent directory) is not allowed".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("Absolute path components are not allowed".to_string());
            }
            _ => {}
        }
    }

    // Build the full path
    let full_path = base_dir.join(relative_path);

    // Canonicalize base directory
    let canonical_base = base_dir.canonicalize()
        .map_err(|e| format!("Failed to canonicalize base directory: {}", e))?;

    // For existing paths, verify the canonical path
    if full_path.exists() {
        let canonical_full = full_path.canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

        if !canonical_full.starts_with(&canonical_base) {
            return Err("Path escapes the allowed directory".to_string());
        }
    } else {
        // For non-existing paths, validate the parent directory
        // This prevents symlink attacks where parent exists but points outside
        if let Some(parent) = full_path.parent() {
            if parent.exists() {
                let canonical_parent = parent.canonicalize()
                    .map_err(|e| format!("Failed to canonicalize parent directory: {}", e))?;

                if !canonical_parent.starts_with(&canonical_base) {
                    return Err("Parent directory escapes the allowed directory".to_string());
                }
            }
            // If parent doesn't exist, we'll fail later when trying to write
        }
    }

    Ok(full_path)
}

/// Validates a path for writing. Same as validate_safe_path but with additional
/// checks to ensure the target directory exists and is writable.
fn validate_safe_write_path(base_dir: &Path, relative_path: &str) -> Result<std::path::PathBuf, String> {
    let full_path = validate_safe_path(base_dir, relative_path)?;

    // Ensure parent directory exists for write operations
    if let Some(parent) = full_path.parent() {
        if !parent.exists() {
            return Err(format!("Parent directory does not exist: {:?}", parent));
        }
    }

    Ok(full_path)
}

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

    // Read transcript file with path validation
    let meetings_dir = manager.get_meetings_dir();
    let full_path = validate_safe_path(&meetings_dir, &transcript_path)?;

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

/// Generates an AI summary for a meeting session.
///
/// This command:
/// 1. Validates the session exists and has a transcript
/// 2. Reads the transcript content
/// 3. Sends it to the configured LLM provider for summarization
/// 4. Saves the summary to a markdown file
/// 5. Updates the session with the summary path
///
/// # Arguments
/// * `session_id` - The unique ID of the session to summarize
///
/// # Returns
/// * `Ok(String)` - The generated summary text
/// * `Err(String)` - If session not found, no transcript, or LLM call fails
#[tauri::command]
#[specta::specta]
pub async fn generate_meeting_summary(app: AppHandle, session_id: String) -> Result<String, String> {
    info!(
        "generate_meeting_summary command called for session: {}",
        session_id
    );

    let manager = app.state::<Arc<MeetingSessionManager>>();

    // Get session from database
    let session = manager
        .get_session(&session_id)
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Check if transcript exists
    let transcript_path = session
        .transcript_path
        .ok_or_else(|| "No transcript available for this session".to_string())?;

    // Read transcript content with path validation
    let meetings_dir = manager.get_meetings_dir();
    let full_transcript_path = validate_safe_path(&meetings_dir, &transcript_path)?;

    if !full_transcript_path.exists() {
        return Err("Transcript file not found".to_string());
    }

    // Check file size before reading to prevent OOM
    let metadata = std::fs::metadata(&full_transcript_path)
        .map_err(|e| format!("Failed to get transcript metadata: {}", e))?;

    if metadata.len() > MAX_TRANSCRIPT_SIZE {
        return Err(format!(
            "Transcript too large ({} bytes). Maximum allowed: {} bytes",
            metadata.len(),
            MAX_TRANSCRIPT_SIZE
        ));
    }

    // Read transcript using blocking task to avoid blocking async runtime
    let transcript_path_clone = full_transcript_path.clone();
    let transcript = tokio::task::spawn_blocking(move || {
        std::fs::read_to_string(&transcript_path_clone)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| format!("Failed to read transcript: {}", e))?;

    if transcript.trim().is_empty() {
        return Err("Transcript is empty".to_string());
    }

    // Get settings for LLM configuration
    let settings = get_settings(&app);

    // Get active provider
    let provider = settings
        .active_post_process_provider()
        .cloned()
        .ok_or_else(|| "No LLM provider configured. Please set up a provider in Settings.".to_string())?;

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    if model.trim().is_empty() {
        return Err(format!(
            "No model configured for provider '{}'. Please configure in Settings.",
            provider.label
        ));
    }

    let api_key = settings
        .post_process_api_keys
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Validate API key is set
    if api_key.trim().is_empty() {
        return Err(format!(
            "No API key configured for provider '{}'. Please set your API key in Settings.",
            provider.label
        ));
    }

    // Build summary prompt
    let summary_prompt = format!(
        r#"Please summarize this meeting transcript concisely. Structure your response with:

## Key Points
- Main topics and discussions

## Action Items
- Tasks assigned with owners (if mentioned)

## Decisions Made
- Important decisions reached

## Next Steps
- Follow-up actions needed

Transcript:
{}

Provide a clear, professional summary in markdown format."#,
        transcript
    );

    debug!(
        "Generating summary with provider '{}' (model: {})",
        provider.id, model
    );

    // Call LLM API
    let summary = crate::llm_client::send_chat_completion(&provider, api_key, &model, summary_prompt)
        .await
        .map_err(|e| format!("LLM API call failed: {}", e))?
        .ok_or_else(|| "LLM returned empty response".to_string())?;

    // Save summary to file with path validation
    let summary_filename = format!("{}/summary.md", session_id);
    let summary_path = validate_safe_write_path(&meetings_dir, &summary_filename)?;

    // Write using blocking task to avoid blocking async runtime
    let summary_clone = summary.clone();
    tokio::task::spawn_blocking(move || {
        std::fs::write(&summary_path, &summary_clone)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
    .map_err(|e| format!("Failed to save summary: {}", e))?;

    // Update database with summary path
    manager
        .update_session_summary_path(&session_id, &summary_filename)
        .map_err(|e| format!("Failed to update session: {}", e))?;

    info!(
        "Summary generated and saved for session {}: {} bytes",
        session_id,
        summary.len()
    );

    // Emit event for frontend
    if let Some(updated_session) = manager.get_session(&session_id).ok().flatten() {
        let _ = app.emit("meeting_summary_generated", &updated_session);
    }

    Ok(summary)
}

/// Gets the summary text content for a meeting session.
///
/// Reads the summary file from disk and returns its content.
///
/// # Arguments
/// * `session_id` - The unique ID of the session to get summary for
///
/// # Returns
/// * `Ok(Some(String))` - The summary text if available
/// * `Ok(None)` - If no summary exists for this session
/// * `Err(String)` - If session not found or file read fails
#[tauri::command]
#[specta::specta]
pub fn get_meeting_summary(app: AppHandle, session_id: String) -> Result<Option<String>, String> {
    info!(
        "get_meeting_summary command called for session: {}",
        session_id
    );

    let manager = app.state::<Arc<MeetingSessionManager>>();

    // Get session from database
    let session = manager
        .get_session(&session_id)
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Check if summary path exists
    let summary_path = match session.summary_path {
        Some(path) => path,
        None => return Ok(None),
    };

    // Read summary file with path validation
    let meetings_dir = manager.get_meetings_dir();
    let full_path = validate_safe_path(&meetings_dir, &summary_path)?;

    if !full_path.exists() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&full_path)
        .map_err(|e| format!("Failed to read summary file: {}", e))?;

    Ok(Some(content))
}
