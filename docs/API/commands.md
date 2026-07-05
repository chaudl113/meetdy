# Tauri Commands

All commands are registered in `src-tauri/src/lib.rs` via Specta's `ts::export` builder.
Each entry lists the Rust signature, parameter types, return type, and associated events.

## Settings

### `get_app_settings`

Returns the full application settings object.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<AppSettings, String>` |
| **Events** | _(none)_ |

### `get_default_settings`

Returns the factory-default application settings.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<AppSettings, String>` |
| **Events** | _(none)_ |

### `set_log_level`

Updates the file log level at runtime.

| | |
|---|---|
| **Parameters** | `level: LogLevel` — one of `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `get_app_dir_path`

Returns the platform-specific app data directory path.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `get_log_dir_path`

Returns the platform-specific app log directory path.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `open_recordings_folder`

Opens the recordings directory in the system file manager.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `open_log_dir`

Opens the log directory in the system file manager.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `open_app_data_dir`

Opens the app data directory in the system file manager.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

---

## Audio

### `get_available_microphones`

Lists all available input audio devices.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Vec<AudioDevice>, String>` |
| **Events** | _(none)_ |

### `get_selected_microphone`

Returns the currently selected microphone device name.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `set_selected_microphone`

Sets the active microphone device and updates the audio manager.

| | |
|---|---|
| **Parameters** | `device_name: String` — device name, or `"default"` for system default |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `get_available_output_devices`

Lists all available output audio devices.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Vec<AudioDevice>, String>` |
| **Events** | _(none)_ |

### `get_selected_output_device`

Returns the currently selected output device name.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `set_selected_output_device`

Sets the output device for audio feedback playback.

| | |
|---|---|
| **Parameters** | `device_name: String` — device name, or `"default"` for system default |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `get_microphone_mode`

Returns whether the microphone is in always-on mode.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<bool, String>` |
| **Events** | _(none)_ |

### `update_microphone_mode`

Switches between on-demand and always-on microphone modes.

| | |
|---|---|
| **Parameters** | `always_on: bool` — `true` for always-on, `false` for on-demand |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `is_recording`

Checks if audio recording is currently active (legacy push-to-talk path).

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `bool` |
| **Events** | _(none)_ |

### `set_clamshell_microphone`

Sets the microphone to use when the laptop lid is closed.

| | |
|---|---|
| **Parameters** | `device_name: String` — device name, or `"default"` for system default |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `get_clamshell_microphone`

Returns the clamshell-mode microphone setting.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `check_custom_sounds`

Checks whether custom start/stop sound files exist in app data.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `CustomSounds { start: bool, stop: bool }` |
| **Events** | _(none)_ |

### `play_test_sound`

Plays a preview of the configured start or stop sound.

| | |
|---|---|
| **Parameters** | `sound_type: String` — `"start"` or `"stop"` |
| **Returns** | _(none — fire-and-forget)_ |
| **Events** | _(none)_ |

---

## Models

### `get_available_models`

Returns all model entries known to the model manager.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Vec<ModelInfo>, String>` |
| **Events** | _(none)_ |

### `get_model_info`

Returns metadata for a single model by ID.

| | |
|---|---|
| **Parameters** | `model_id: String` |
| **Returns** | `Result<Option<ModelInfo>, String>` |
| **Events** | _(none)_ |

### `download_model`

Starts downloading a model. Emits `model-download-progress` periodically and
`model-download-complete` on success.

| | |
|---|---|
| **Parameters** | `model_id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | `model-download-progress`, `model-extraction-started`, `model-extraction-completed`, `model-extraction-failed`, `model-download-complete` |

### `delete_model`

Deletes a downloaded model from disk.

| | |
|---|---|
| **Parameters** | `model_id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `set_active_model`

Loads a downloaded model into the transcription engine and persists the selection.

| | |
|---|---|
| **Parameters** | `model_id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `get_current_model`

Returns the model ID selected in settings (may not be loaded).

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `get_transcription_model_status`

Returns the ID of the currently loaded transcription model, or `None`.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Option<String>, String>` |
| **Events** | _(none)_ |

### `is_model_loading`

Returns `true` while no model is loaded (i.e. loading state).

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<bool, String>` |
| **Events** | _(none)_ |

### `has_any_models_available`

Returns `true` if at least one model is downloaded and ready.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<bool, String>` |
| **Events** | _(none)_ |

### `has_any_models_or_downloads`

Returns `true` if any model is downloaded.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<bool, String>` |
| **Events** | _(none)_ |

### `cancel_download`

Cancels an in-progress model download.

| | |
|---|---|
| **Parameters** | `model_id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none — progress events from the cancelled download will stop)_ |

### `get_recommended_first_model`

Returns the recommended default model ID for first-time users.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

---

## Transcription

### `get_model_load_status`

Reports whether a model is loaded and which one.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<ModelLoadStatus, String>` |
| **Events** | _(none)_ |

### `set_model_unload_timeout`

Configures the idle timeout before unloading the model.

| | |
|---|---|
| **Parameters** | `timeout: ModelUnloadTimeout` — `"never"`, `"immediately"`, `"min_2"`, `"min_5"`, `"min_10"`, `"min_15"`, `"hour_1"`, `"sec_5"` |
| **Returns** | _(none)_ |
| **Events** | _(none)_ |

### `unload_model_manually`

Force-unloads the current transcription model from memory.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

---

## Meeting

### `start_meeting_session`

Creates and starts a new meeting recording session.

| | |
|---|---|
| **Parameters** | `audio_source: Option<AudioSourceType>` — `"microphone_only"`, `"system_only"`, or `"mixed"`<br>`template_id: Option<String>` — optional meeting template to apply |
| **Returns** | `Result<MeetingSession, String>` |
| **Events** | `meeting_started` |

### `stop_meeting_session`

Stops the current meeting recording and triggers transcription.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` — relative path to the audio file |
| **Events** | `meeting_stopped`, `meeting_processing`, then eventually `meeting_completed` or `meeting_failed` |

### `get_meeting_status`

Returns the current meeting session status (if any).

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Option<MeetingStatus>` |
| **Events** | _(none)_ |

### `get_current_meeting`

Returns full details of the currently active meeting session, if any.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Option<MeetingSession>, String>` |
| **Events** | _(none)_ |

### `update_meeting_title`

Renames a meeting session.

| | |
|---|---|
| **Parameters** | `session_id: String`<br>`title: String` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `list_meeting_sessions`

Lists all meeting sessions ordered newest-first.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Vec<MeetingSession>, String>` |
| **Events** | _(none)_ |

### `get_meetings_directory`

Returns the platform-specific absolute path to the meetings directory.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `delete_meeting_session`

Deletes a meeting session and its associated files (audio, transcript, summary).

| | |
|---|---|
| **Parameters** | `session_id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `get_meeting_transcript`

Reads the transcript file content for a completed meeting.

| | |
|---|---|
| **Parameters** | `session_id: String` |
| **Returns** | `Result<Option<String>, String>` |
| **Events** | _(none)_ |

### `retry_transcription`

Re-processes transcription for a failed, interrupted, or completed session.

| | |
|---|---|
| **Parameters** | `session_id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | `meeting_processing`, then eventually `meeting_completed` or `meeting_failed` |

### `generate_meeting_summary`

Sends the meeting transcript to the configured LLM provider for summarization.

| | |
|---|---|
| **Parameters** | `session_id: String` |
| **Returns** | `Result<String, String>` — the generated summary markdown |
| **Events** | `meeting_summary_status` (progress), `meeting_summary_generated` (completion), `ollama_pull_progress` (if auto-pulling Ollama model) |

### `get_meeting_summary`

Reads the summary markdown file for a meeting.

| | |
|---|---|
| **Parameters** | `session_id: String` |
| **Returns** | `Result<Option<String>, String>` |
| **Events** | _(none)_ |

### `add_meeting_note`

Adds a timestamped note to a meeting session.

| | |
|---|---|
| **Parameters** | `session_id: String`<br>`timestamp_seconds: i64` — seconds offset from recording start<br>`content: String` |
| **Returns** | `Result<MeetingNote, String>` |
| **Events** | _(none)_ |

### `list_meeting_notes`

Lists all notes for a meeting session, oldest first.

| | |
|---|---|
| **Parameters** | `session_id: String` |
| **Returns** | `Result<Vec<MeetingNote>, String>` |
| **Events** | _(none)_ |

### `delete_meeting_note`

Deletes a single meeting note by ID.

| | |
|---|---|
| **Parameters** | `note_id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

---

## History

### `get_history_entries`

Returns all history entries (legacy push-to-talk transcriptions).

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Vec<HistoryEntry>, String>` |
| **Events** | _(none)_ |

### `toggle_history_entry_saved`

Toggles the saved/unsaved flag on a history entry.

| | |
|---|---|
| **Parameters** | `id: i64` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `get_audio_file_path`

Resolves the full filesystem path to a recording file.

| | |
|---|---|
| **Parameters** | `file_name: String` |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `delete_history_entry`

Deletes a history entry and its associated audio file.

| | |
|---|---|
| **Parameters** | `id: i64` |
| **Returns** | `Result<(), String>` |
| **Events** | `history-updated` |

### `update_history_limit`

Sets the maximum number of history entries and prunes old ones.

| | |
|---|---|
| **Parameters** | `limit: usize` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

### `update_recording_retention_period`

Sets the recording retention policy and prunes old recordings.

| | |
|---|---|
| **Parameters** | `period: String` — `"never"`, `"preserve_limit"`, `"days_3"`, `"weeks_2"`, `"months_3"` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

---

## Templates

### `list_meeting_templates`

Returns all meeting templates, including user-created and built-in defaults.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<Vec<MeetingTemplate>, String>` |
| **Events** | _(none)_ |

### `create_meeting_template`

Creates a new meeting template.

| | |
|---|---|
| **Parameters** | `name: String` — 1–50 chars, unique<br>`icon: String` — emoji or icon key<br>`title_template: String` — may contain `{date}` and `{time}` placeholders<br>`audio_source: String` — `"microphone_only"`, `"system_only"`, `"mixed"`<br>`prompt_id: Option<String>` — optional reference to a post-processing prompt<br>`summary_prompt_template: Option<String>` — optional custom LLM prompt; must contain `{}` for transcript insertion |
| **Returns** | `Result<MeetingTemplate, String>` |
| **Events** | _(none)_ |

### `update_meeting_template`

Updates an existing meeting template. All fields except `id` are optional.

| | |
|---|---|
| **Parameters** | `id: String`<br>`name: Option<String>`<br>`icon: Option<String>`<br>`title_template: Option<String>`<br>`audio_source: Option<String>`<br>`prompt_id: Option<String>`<br>`summary_prompt_template: Option<String>` |
| **Returns** | `Result<MeetingTemplate, String>` |
| **Events** | _(none)_ |

### `delete_meeting_template`

Deletes a user-created template. Built-in templates (IDs starting with `"template_"`) cannot be deleted.

| | |
|---|---|
| **Parameters** | `id: String` |
| **Returns** | `Result<(), String>` |
| **Events** | _(none)_ |

---

## Utilities

### `cancel_operation`

Signals the backend to cancel the currently running operation (e.g., transcription in progress).

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | _(none)_ |
| **Events** | _(none)_ |

### `translate_text`

Translates text using Google Translate's public web endpoint (no API key required). Best-effort; long texts are split at paragraph/sentence boundaries.

| | |
|---|---|
| **Parameters** | `text: String`<br>`source: String` — ISO 639-1 language code, or `"auto"`<br>`target: String` — ISO 639-1 language code |
| **Returns** | `Result<String, String>` |
| **Events** | _(none)_ |

### `open_chatgpt_login`

Opens a Tauri webview window for ChatGPT Plus sign-in. After the user logs in, an injected script captures the session token and calls `complete_chatgpt_login`.

| | |
|---|---|
| **Parameters** | _(none)_ |
| **Returns** | `Result<(), String>` |
| **Events** | `chatgpt-login-success` |

### `complete_chatgpt_login`

**Internal command** — called by the injected script in the login webview. Emits the captured access token as an event and closes the login window.

| | |
|---|---|
| **Parameters** | `access_token: String` |
| **Returns** | `Result<(), String>` |
| **Events** | `chatgpt-login-success` |
