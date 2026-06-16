use crate::managers::meeting::{
    ActionItem, ActionItemStatus, AudioSourceType, KeyPoint, MeetingInsights, MeetingNote,
    MeetingSession, MeetingSessionManager, MeetingStatus, Participant, Tag, TranscriptSegment,
};
use crate::settings::{get_settings, write_settings};
use log::{debug, info, warn};
use serde::Deserialize;
use std::path::{Component, Path};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Maximum transcript size in bytes (1MB) to prevent OOM and LLM context overflow
const MAX_TRANSCRIPT_SIZE: u64 = 1024 * 1024;

/// Interpolates a title template with current date/time placeholders.
///
/// Supported placeholders:
/// - `{date}` - Replaced with current date in YYYY-MM-DD format
/// - `{time}` - Replaced with current time in HH:MM format
///
/// # Arguments
/// * `template` - The title template string
///
/// # Returns
/// The interpolated title string
fn interpolate_title_template(template: &str) -> String {
    let now = chrono::Local::now();
    template
        .replace("{date}", &now.format("%Y-%m-%d").to_string())
        .replace("{time}", &now.format("%H:%M").to_string())
}

/// Builds the default summary prompt for meetings without a custom template.
///
/// This is the standard prompt used when no template-specific prompt is configured.
///
/// # Arguments
/// * `transcript` - The meeting transcript to summarize
///
/// # Returns
/// The formatted prompt string ready for LLM consumption
fn build_default_summary_prompt(transcript: &str) -> String {
    format!(
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
    )
}

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
    let canonical_base = base_dir
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize base directory: {}", e))?;

    // For existing paths, verify the canonical path
    if full_path.exists() {
        let canonical_full = full_path
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize path: {}", e))?;

        if !canonical_full.starts_with(&canonical_base) {
            return Err("Path escapes the allowed directory".to_string());
        }
    } else {
        // For non-existing paths, validate the parent directory
        // This prevents symlink attacks where parent exists but points outside
        if let Some(parent) = full_path.parent() {
            if parent.exists() {
                let canonical_parent = parent
                    .canonicalize()
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
fn validate_safe_write_path(
    base_dir: &Path,
    relative_path: &str,
) -> Result<std::path::PathBuf, String> {
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
/// 2. Optionally loads a template with pre-configured settings
/// 3. Creates a new meeting session with UUID and folder
/// 4. Starts audio capture with the specified (or template-based) source
/// 5. Updates session status to Recording
///
/// # Arguments
/// * `audio_source` - The audio source configuration (microphone_only, system_only, or mixed)
///                    If None and template_id is provided, uses template's audio_source
/// * `template_id` - Optional ID of a meeting template to use for this session
///
/// # Returns
/// * `Ok(MeetingSession)` - The newly created and active session
/// * `Err(String)` - If state guard fails, template not found, or recording initialization fails
#[tauri::command]
#[specta::specta]
pub async fn start_meeting_session(
    app: AppHandle,
    audio_source: Option<AudioSourceType>,
    template_id: Option<String>,
    stt_engine: Option<String>,
    soniox_api_key: Option<String>,
    funasr_base_url: Option<String>,
    funasr_model: Option<String>,
) -> Result<MeetingSession, String> {
    info!(
        "start_meeting_session command called with template_id: {:?}, audio_source: {:?}, stt_engine: {:?}",
        template_id, audio_source, stt_engine
    );

    // Load template if template_id is provided
    let template = if let Some(tid) = template_id.as_ref() {
        let settings = get_settings(&app);
        settings
            .meeting_templates
            .iter()
            .find(|t| &t.id == tid)
            .cloned()
    } else {
        None
    };

    // Determine audio source: use explicit parameter, then template, then default
    let source = audio_source
        .or_else(|| {
            template
                .as_ref()
                .and_then(|t| match t.audio_source.as_str() {
                    "microphone_only" => Some(AudioSourceType::MicrophoneOnly),
                    "system_only" => Some(AudioSourceType::SystemOnly),
                    "mixed" => Some(AudioSourceType::Mixed),
                    _ => None,
                })
        })
        .unwrap_or_default();

    debug!("Using audio source: {:?}", source);

    // Determine STT engine: explicit param > template > global setting > "whisper"
    let engine = stt_engine
        .or_else(|| template.as_ref().and_then(|t| t.stt_engine.clone()))
        .or_else(|| {
            let s = get_settings(&app);
            if s.meeting_stt_engine != "whisper" {
                Some(s.meeting_stt_engine)
            } else {
                None
            }
        })
        .unwrap_or_else(|| "whisper".to_string());

    if !["whisper", "soniox", "funasr"].contains(&engine.as_str()) {
        return Err(format!("Invalid stt_engine: {}", engine));
    }

    // Resolve Soniox API key: explicit param > global setting
    let resolved_api_key = soniox_api_key.or_else(|| {
        if engine == "soniox" {
            get_settings(&app).soniox_api_key
        } else {
            None
        }
    });

    // Resolve language and local FunASR settings.
    let mut settings = get_settings(&app);
    let mut source_language = {
        let lang = &settings.selected_language;
        if lang.is_empty() || lang == "auto" {
            None
        } else {
            Some(lang.clone())
        }
    };
    // Translation: only used when engine is soniox and translate_to_english is on
    let target_language = if engine == "soniox" && settings.translate_to_english {
        Some("en".to_string())
    } else {
        None
    };

    if let Some(base_url) = funasr_base_url {
        settings.funasr_base_url = base_url;
    }
    if let Some(model) = funasr_model {
        settings.funasr_model = model;
    }

    if engine == "funasr" {
        if source_language.is_none() {
            source_language = Some("vi".to_string());
            info!(
                "FunASR selected with auto language; defaulting transcription language to vi to avoid Chinese auto-detect drift"
            );
        }

        if settings.funasr_model == "sensevoice" {
            settings.funasr_model = "fun-asr-nano".to_string();
            info!("FunASR selected with sensevoice; switching to fun-asr-nano for Vietnamese");
        }
    }

    settings.meeting_stt_engine = engine.clone();
    write_settings(&app, settings.clone());

    if engine == "funasr" && !crate::funasr_client::is_runtime_installed(&app) {
        return Err(
            "FunASR is not set up. Open Models and download/setup FunASR before starting a FunASR meeting."
                .to_string(),
        );
    }

    if engine == "funasr" {
        crate::funasr_client::ensure_local_server_running(
            &app,
            &settings.funasr_base_url,
            &settings.funasr_model,
        )
        .await
        .map_err(|e| format!("Failed to start FunASR server: {}", e))?;
    }

    let manager = app.state::<Arc<MeetingSessionManager>>();
    let mut session = manager
        .start_recording(
            source,
            engine,
            resolved_api_key,
            source_language,
            target_language,
        )
        .map_err(|e| format!("Failed to start meeting session: {}", e))?;

    // Apply template settings if available
    if let Some(template) = template {
        debug!(
            "Applying template '{}' to session {}",
            template.name, session.id
        );

        // Generate title from template
        let generated_title = interpolate_title_template(&template.title_template);

        // Update session title (this will update in database)
        manager
            .update_session_title(&session.id, &generated_title)
            .map_err(|e| format!("Failed to update session title: {}", e))?;

        // Update the returned session object with the new title and template_id
        session.title = generated_title;
        session.template_id = Some(template.id.clone());

        // Store template_id in database for summary generation later
        manager
            .update_session_template_id(&session.id, &template.id)
            .map_err(|e| format!("Failed to update session template_id: {}", e))?;

        // Store template metadata for future reference
        // Note: prompt_id can be used for post-processing later
        debug!(
            "Session {} configured with template '{}' (prompt_id: {:?})",
            session.id, template.id, template.prompt_id
        );
    }

    Ok(session)
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

#[tauri::command]
#[specta::specta]
pub fn pause_meeting_session(app: AppHandle) -> Result<MeetingSession, String> {
    info!("pause_meeting_session command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .pause_recording()
        .map_err(|e| format!("Failed to pause meeting session: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn resume_meeting_session(app: AppHandle) -> Result<MeetingSession, String> {
    info!("resume_meeting_session command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .resume_recording()
        .map_err(|e| format!("Failed to resume meeting session: {}", e))
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
        match manager_clone.process_transcription(&session_id_clone, &audio_path_clone) {
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
pub fn get_meeting_transcript(
    app: AppHandle,
    session_id: String,
) -> Result<Option<String>, String> {
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
    info!(
        "delete_meeting_session command called for session: {}",
        session_id
    );

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .delete_session(&session_id)
        .map_err(|e| format!("Failed to delete meeting session: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn clear_all_meeting_sessions(app: AppHandle) -> Result<(), String> {
    info!("clear_all_meeting_sessions command called");

    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .delete_all_sessions()
        .map_err(|e| format!("Failed to clear meeting history: {}", e))
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
pub async fn generate_meeting_summary(
    app: AppHandle,
    session_id: String,
    output_language: Option<String>,
) -> Result<String, String> {
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
    let transcript =
        tokio::task::spawn_blocking(move || std::fs::read_to_string(&transcript_path_clone))
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
        .ok_or_else(|| {
            "No LLM provider configured. Please set up a provider in Settings.".to_string()
        })?;

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();

    // Fall back to provider's default model if none configured
    let model = if model.trim().is_empty() {
        provider.default_model.clone().unwrap_or_default()
    } else {
        model
    };

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

    // Validate API key is set — but only if the provider requires one
    if provider.requires_api_key && api_key.trim().is_empty() {
        return Err(format!(
            "No API key configured for provider '{}'. Please set your API key in Settings.",
            provider.label
        ));
    }

    // Build summary prompt - use template-specific prompt if available
    let mut summary_prompt = if let Some(template_id) = &session.template_id {
        // Find the template to get its custom summary prompt
        let template = settings
            .meeting_templates
            .iter()
            .find(|t| &t.id == template_id);

        if let Some(template) = template {
            if let Some(ref custom_prompt) = template.summary_prompt_template {
                debug!(
                    "Using template-specific summary prompt for template '{}'",
                    template.name
                );
                // Replace {} placeholder with transcript
                custom_prompt.replace("{}", &transcript)
            } else {
                // Template exists but has no custom prompt, use default
                build_default_summary_prompt(&transcript)
            }
        } else {
            // Template ID exists but template not found (may have been deleted)
            warn!(
                "Template '{}' not found, using default summary prompt",
                template_id
            );
            build_default_summary_prompt(&transcript)
        }
    } else {
        // No template associated with this session, use default
        build_default_summary_prompt(&transcript)
    };

    if let Some(language) = output_language.as_ref().filter(|v| !v.trim().is_empty()) {
        summary_prompt.push_str(&format!(
            "\n\nImportant: Write the summary in this language/locale: {}.",
            language
        ));
    }

    debug!(
        "Generating summary with provider '{}' (model: {})",
        provider.id, model
    );

    // Auto-setup for Ollama: start server + pull model if needed
    if provider.id == "ollama" || provider.id == "lmstudio" {
        let status = crate::ollama::check_ollama_status().await;
        match status.status {
            crate::ollama::OllamaStatus::NotInstalled => {
                return Err(format!(
                    "Ollama is not installed. Please download from: {}",
                    crate::ollama::get_ollama_install_url()
                ));
            }
            crate::ollama::OllamaStatus::Installed => {
                // Auto-start
                info!("Ollama not running, starting automatically...");
                let _ = app.emit("meeting_summary_status", "Starting Ollama server...");
                crate::ollama::start_ollama().await.map_err(|e| {
                    format!(
                        "Failed to auto-start Ollama: {}. Please start it manually.",
                        e
                    )
                })?;
            }
            crate::ollama::OllamaStatus::Running => {
                debug!("Ollama is already running");
            }
        }

        // Check if the model is available, if not — auto-pull
        if provider.id == "ollama" {
            let models = crate::ollama::check_ollama_status().await;
            let model_available = models
                .models
                .iter()
                .any(|m| m.name == model || m.name.starts_with(&format!("{}:", model)));

            if !model_available {
                info!("Model '{}' not found locally, pulling...", model);
                let _ = app.emit(
                    "meeting_summary_status",
                    &format!("Downloading model {}...", model),
                );
                crate::ollama::pull_ollama_model(app.clone(), model.clone())
                    .await
                    .map_err(|e| format!("Failed to download model '{}': {}", model, e))?;
            }
        }
    }

    // Call LLM API
    let summary =
        crate::llm_client::send_chat_completion(&provider, api_key, &model, summary_prompt)
            .await
            .map_err(|e| format!("LLM API call failed: {}", e))?
            .ok_or_else(|| "LLM returned empty response".to_string())?;

    // Save summary to file with path validation
    let summary_filename = format!("{}/summary.md", session_id);
    let summary_path = validate_safe_write_path(&meetings_dir, &summary_filename)?;

    // Write using blocking task to avoid blocking async runtime
    let summary_clone = summary.clone();
    tokio::task::spawn_blocking(move || std::fs::write(&summary_path, &summary_clone))
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

// --- Insight extraction ----------------------------------------------------

/// Raw JSON shape the LLM is asked to return.
#[derive(Debug, Deserialize, Default)]
struct ExtractedInsightsJson {
    #[serde(default)]
    key_points: Vec<ExtractedKeyPointJson>,
    #[serde(default)]
    action_items: Vec<ExtractedActionItemJson>,
    #[serde(default)]
    participants: Vec<ExtractedParticipantJson>,
}

#[derive(Debug, Deserialize)]
struct ExtractedKeyPointJson {
    #[serde(default)]
    category: Option<String>,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ExtractedActionItemJson {
    task: String,
    #[serde(default)]
    assignee: Option<String>,
    #[serde(default)]
    due_date: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExtractedParticipantJson {
    name: String,
    #[serde(default)]
    role: Option<String>,
}

fn build_extract_insights_prompt(transcript: &str, output_language: Option<&str>) -> String {
    let language_hint = output_language
        .filter(|s| !s.trim().is_empty())
        .map(|l| {
            format!(
                "\nWrite all extracted text in this language/locale: {}.\n",
                l
            )
        })
        .unwrap_or_default();

    format!(
        r#"You extract structured data from a meeting transcript. Return ONLY a single JSON object (no markdown fences, no commentary) matching exactly this schema:

{{
  "key_points": [{{ "category": "string|null", "content": "string" }}],
  "action_items": [{{ "task": "string", "assignee": "string|null", "due_date": "string|null", "status": "todo|in_progress|done|blocked" }}],
  "participants": [{{ "name": "string", "role": "string|null" }}]
}}

Rules:
- key_points: concise discussion bullets. Use "category" to group related bullets under a heading (e.g. "Regulatory", "Marketing"); use null if ungrouped.
- action_items: concrete tasks. Preserve relative dates verbatim (e.g. "end of week", "next Monday"). Default status is "todo".
- participants: people who spoke or were referenced as attendees. Role/team is free-form text or null.
- Omit entries you are not confident about. Return empty arrays rather than fabricating.
- Output must be valid JSON. Do not wrap it in ```json fences.{language_hint}

Transcript:
{transcript}"#,
        language_hint = language_hint,
        transcript = transcript,
    )
}

/// Strips ```json ... ``` fences if the model wrapped the response.
fn strip_json_fences(s: &str) -> &str {
    let trimmed = s.trim();
    let without_prefix = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    without_prefix
        .strip_suffix("```")
        .unwrap_or(without_prefix)
        .trim()
}

fn parse_action_status(s: Option<&str>) -> ActionItemStatus {
    match s.map(|v| v.trim().to_lowercase()).as_deref() {
        Some("in_progress") | Some("in-progress") | Some("inprogress") => {
            ActionItemStatus::InProgress
        }
        Some("done") | Some("completed") => ActionItemStatus::Done,
        Some("blocked") => ActionItemStatus::Blocked,
        _ => ActionItemStatus::Todo,
    }
}

/// Extracts structured insights (key points, action items, participants) from
/// a meeting transcript using the configured LLM, persists them (replacing any
/// previous AI-extracted data for the session), and returns the stored result.
#[tauri::command]
#[specta::specta]
pub async fn extract_meeting_insights(
    app: AppHandle,
    session_id: String,
    output_language: Option<String>,
) -> Result<MeetingInsights, String> {
    info!(
        "extract_meeting_insights command called for session: {}",
        session_id
    );

    let manager = app.state::<Arc<MeetingSessionManager>>();

    let session = manager
        .get_session(&session_id)
        .map_err(|e| format!("Failed to get session: {}", e))?
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    let transcript_path = session
        .transcript_path
        .ok_or_else(|| "No transcript available for this session".to_string())?;

    let meetings_dir = manager.get_meetings_dir();
    let full_transcript_path = validate_safe_path(&meetings_dir, &transcript_path)?;

    if !full_transcript_path.exists() {
        return Err("Transcript file not found".to_string());
    }

    let metadata = std::fs::metadata(&full_transcript_path)
        .map_err(|e| format!("Failed to get transcript metadata: {}", e))?;
    if metadata.len() > MAX_TRANSCRIPT_SIZE {
        return Err(format!(
            "Transcript too large ({} bytes). Maximum allowed: {} bytes",
            metadata.len(),
            MAX_TRANSCRIPT_SIZE
        ));
    }

    let transcript_path_clone = full_transcript_path.clone();
    let transcript =
        tokio::task::spawn_blocking(move || std::fs::read_to_string(&transcript_path_clone))
            .await
            .map_err(|e| format!("Task join error: {}", e))?
            .map_err(|e| format!("Failed to read transcript: {}", e))?;

    if transcript.trim().is_empty() {
        return Err("Transcript is empty".to_string());
    }

    let settings = get_settings(&app);
    let provider = settings
        .active_post_process_provider()
        .cloned()
        .ok_or_else(|| {
            "No LLM provider configured. Please set up a provider in Settings.".to_string()
        })?;

    let model = settings
        .post_process_models
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();
    let model = if model.trim().is_empty() {
        provider.default_model.clone().unwrap_or_default()
    } else {
        model
    };
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
    if provider.requires_api_key && api_key.trim().is_empty() {
        return Err(format!(
            "No API key configured for provider '{}'. Please set your API key in Settings.",
            provider.label
        ));
    }

    let prompt = build_extract_insights_prompt(&transcript, output_language.as_deref());

    debug!(
        "Extracting insights with provider '{}' (model: {})",
        provider.id, model
    );

    // Auto-setup for Ollama: mirrors generate_meeting_summary behavior.
    if provider.id == "ollama" || provider.id == "lmstudio" {
        let status = crate::ollama::check_ollama_status().await;
        match status.status {
            crate::ollama::OllamaStatus::NotInstalled => {
                return Err(format!(
                    "Ollama is not installed. Please download from: {}",
                    crate::ollama::get_ollama_install_url()
                ));
            }
            crate::ollama::OllamaStatus::Installed => {
                info!("Ollama not running, starting automatically...");
                let _ = app.emit("meeting_insights_status", "Starting Ollama server...");
                crate::ollama::start_ollama().await.map_err(|e| {
                    format!(
                        "Failed to auto-start Ollama: {}. Please start it manually.",
                        e
                    )
                })?;
            }
            crate::ollama::OllamaStatus::Running => {
                debug!("Ollama is already running");
            }
        }

        if provider.id == "ollama" {
            let models = crate::ollama::check_ollama_status().await;
            let model_available = models
                .models
                .iter()
                .any(|m| m.name == model || m.name.starts_with(&format!("{}:", model)));
            if !model_available {
                info!("Model '{}' not found locally, pulling...", model);
                let _ = app.emit(
                    "meeting_insights_status",
                    &format!("Downloading model {}...", model),
                );
                crate::ollama::pull_ollama_model(app.clone(), model.clone())
                    .await
                    .map_err(|e| format!("Failed to download model '{}': {}", model, e))?;
            }
        }
    }

    let raw = crate::llm_client::send_chat_completion(&provider, api_key, &model, prompt)
        .await
        .map_err(|e| format!("LLM API call failed: {}", e))?
        .ok_or_else(|| "LLM returned empty response".to_string())?;

    let json_str = strip_json_fences(&raw);
    let extracted: ExtractedInsightsJson = serde_json::from_str(json_str).map_err(|e| {
        format!(
            "Failed to parse LLM JSON response: {}. Raw response: {}",
            e,
            raw.chars().take(500).collect::<String>()
        )
    })?;

    let now = chrono::Utc::now().timestamp();

    let mut key_points = Vec::with_capacity(extracted.key_points.len());
    for (idx, kp) in extracted.key_points.into_iter().enumerate() {
        let content = kp.content.trim().to_string();
        if content.is_empty() {
            continue;
        }
        key_points.push(KeyPoint {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            category: kp
                .category
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            content,
            sort_order: idx as i64,
            created_at: now,
        });
    }

    let mut action_items = Vec::with_capacity(extracted.action_items.len());
    for (idx, ai) in extracted.action_items.into_iter().enumerate() {
        let task = ai.task.trim().to_string();
        if task.is_empty() {
            continue;
        }
        action_items.push(ActionItem {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            task,
            assignee: ai
                .assignee
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            due_date: ai
                .due_date
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            status: parse_action_status(ai.status.as_deref()),
            sort_order: idx as i64,
            created_at: now,
        });
    }

    let mut participants = Vec::with_capacity(extracted.participants.len());
    for (idx, p) in extracted.participants.into_iter().enumerate() {
        let name = p.name.trim().to_string();
        if name.is_empty() {
            continue;
        }
        participants.push(Participant {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            name,
            role: p
                .role
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            sort_order: idx as i64,
            created_at: now,
            color_index: -1,
        });
    }

    manager
        .replace_insights(
            &session_id,
            action_items.clone(),
            key_points.clone(),
            participants.clone(),
        )
        .map_err(|e| format!("Failed to persist insights: {}", e))?;

    // Tags are preserved as-is (manually managed); include current set in the response.
    let tags = manager
        .list_tags(&session_id)
        .map_err(|e| format!("Failed to list tags: {}", e))?;

    let insights = MeetingInsights {
        key_points,
        action_items,
        participants,
        tags,
    };

    let _ = app.emit("meeting_insights_extracted", &insights);

    info!(
        "Extracted insights for session {}: {} key points, {} action items, {} participants",
        session_id,
        insights.key_points.len(),
        insights.action_items.len(),
        insights.participants.len(),
    );

    Ok(insights)
}

// --- Meeting notes commands ------------------------------------------------

/// Adds a timestamped note to an existing meeting session.
///
/// The timestamp is given in seconds from the start of the recording and is
/// stored as-is. Pass `0` for notes captured outside a recording.
#[tauri::command]
#[specta::specta]
pub fn add_meeting_note(
    app: AppHandle,
    session_id: String,
    timestamp_seconds: i64,
    content: String,
) -> Result<MeetingNote, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .add_note(&session_id, timestamp_seconds, content)
        .map_err(|e| format!("Failed to add meeting note: {}", e))
}

/// Lists every note attached to a meeting session, oldest first.
#[tauri::command]
#[specta::specta]
pub fn list_meeting_notes(app: AppHandle, session_id: String) -> Result<Vec<MeetingNote>, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .list_notes(&session_id)
        .map_err(|e| format!("Failed to list meeting notes: {}", e))
}

/// Deletes a single meeting note by id.
#[tauri::command]
#[specta::specta]
pub fn delete_meeting_note(app: AppHandle, note_id: String) -> Result<(), String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .delete_note(&note_id)
        .map_err(|e| format!("Failed to delete meeting note: {}", e))
}

// --- Action items ----------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn add_meeting_action_item(
    app: AppHandle,
    session_id: String,
    task: String,
    assignee: Option<String>,
    due_date: Option<String>,
    status: Option<ActionItemStatus>,
) -> Result<ActionItem, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .add_action_item(
            &session_id,
            task,
            assignee,
            due_date,
            status.unwrap_or_default(),
        )
        .map_err(|e| format!("Failed to add action item: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn list_meeting_action_items(
    app: AppHandle,
    session_id: String,
) -> Result<Vec<ActionItem>, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .list_action_items(&session_id)
        .map_err(|e| format!("Failed to list action items: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn update_meeting_action_item(app: AppHandle, item: ActionItem) -> Result<(), String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .update_action_item(&item)
        .map_err(|e| format!("Failed to update action item: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn delete_meeting_action_item(app: AppHandle, item_id: String) -> Result<(), String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .delete_action_item(&item_id)
        .map_err(|e| format!("Failed to delete action item: {}", e))
}

// --- Key points ------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn add_meeting_key_point(
    app: AppHandle,
    session_id: String,
    category: Option<String>,
    content: String,
) -> Result<KeyPoint, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .add_key_point(&session_id, category, content)
        .map_err(|e| format!("Failed to add key point: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn list_meeting_key_points(
    app: AppHandle,
    session_id: String,
) -> Result<Vec<KeyPoint>, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .list_key_points(&session_id)
        .map_err(|e| format!("Failed to list key points: {}", e))
}

// --- Participants ----------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn add_meeting_participant(
    app: AppHandle,
    session_id: String,
    name: String,
    role: Option<String>,
) -> Result<Participant, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .add_participant(&session_id, name, role)
        .map_err(|e| format!("Failed to add participant: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn list_meeting_participants(
    app: AppHandle,
    session_id: String,
) -> Result<Vec<Participant>, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .list_participants(&session_id)
        .map_err(|e| format!("Failed to list participants: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn update_meeting_participant(app: AppHandle, participant: Participant) -> Result<(), String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .update_participant(&participant)
        .map_err(|e| format!("Failed to update participant: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn delete_meeting_participant(app: AppHandle, participant_id: String) -> Result<(), String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .delete_participant(&participant_id)
        .map_err(|e| format!("Failed to delete participant: {}", e))
}

// --- Tags ------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn add_meeting_tag(
    app: AppHandle,
    session_id: String,
    label: String,
    color: Option<String>,
) -> Result<Tag, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .add_tag(&session_id, label, color)
        .map_err(|e| format!("Failed to add tag: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn list_meeting_tags(app: AppHandle, session_id: String) -> Result<Vec<Tag>, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .list_tags(&session_id)
        .map_err(|e| format!("Failed to list tags: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn delete_meeting_tag(app: AppHandle, tag_id: String) -> Result<(), String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .delete_tag(&tag_id)
        .map_err(|e| format!("Failed to delete tag: {}", e))
}

#[tauri::command]
#[specta::specta]
pub fn list_all_meeting_tag_labels(app: AppHandle) -> Result<Vec<String>, String> {
    let manager = app.state::<Arc<MeetingSessionManager>>();
    manager
        .list_all_tag_labels()
        .map_err(|e| format!("Failed to list tag labels: {}", e))
}

/// Sets the active speaker for the current meeting session.
/// All transcript segments emitted after this call will be attributed to
/// `participant_id` until the active speaker is changed again.
#[tauri::command]
#[specta::specta]
pub fn set_active_speaker(
    meeting_manager: tauri::State<'_, Arc<MeetingSessionManager>>,
    participant_id: Option<String>,
) -> Result<(), String> {
    match participant_id {
        Some(id) => meeting_manager.set_active_speaker(id, 0.0),
        None => meeting_manager.clear_active_speaker(),
    }
    Ok(())
}

/// Returns all transcript segments for a meeting, ordered by sequence.
/// Used by MeetingDetailView and MeetingTranscriptDisplay to render
/// speaker-attributed transcript after recording is complete.
#[tauri::command]
#[specta::specta]
pub fn get_meeting_transcript_segments(
    meeting_manager: tauri::State<'_, Arc<MeetingSessionManager>>,
    session_id: String,
) -> Result<Vec<TranscriptSegment>, String> {
    let conn = rusqlite::Connection::open(meeting_manager.get_db_path())
        .map_err(|e| format!("DB open failed: {}", e))?;
    crate::managers::meeting::db::list_transcript_segments(&conn, &session_id)
        .map_err(|e| format!("Failed to list transcript segments: {}", e))
}
