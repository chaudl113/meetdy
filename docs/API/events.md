# Events

Events are pushed from backend to frontend via `app.emit()` or `window.emit()`.
The frontend subscribes using `listen("event-name", callback)`.

## Legend

| Icon | Meaning |
|---|---|
| 🌐 | Emitted to all windows (`app.emit`) |
| 🪟 | Emitted to a specific window (`window.emit`) |
| 🧵 | Emitted from a background thread |

---

## Model Download Events

### `model-download-progress` 🌐

Emitted periodically during model download.

| When | During `download_model` |
|---|---|
| **Payload** | `DownloadProgress { model_id: String, downloaded: u64, total: u64, percentage: f64 }` |

### `model-download-complete` 🌐

Emitted when a model download finishes successfully.

| When | After successful download, before extraction |
|---|---|
| **Payload** | `String` — the `model_id` |

### `model-extraction-started` 🌐

Emitted when archive extraction begins.

| When | After download completes, before extraction |
|---|---|
| **Payload** | `String` — the `model_id` |

### `model-extraction-completed` 🌐

Emitted when archive extraction finishes successfully.

| When | After extraction completes |
|---|---|
| **Payload** | `String` — the `model_id` |

### `model-extraction-failed` 🌐

Emitted when archive extraction fails.

| When | On extraction error |
|---|---|
| **Payload** | `{ "model_id": String, "error": String }` |

---

## Meeting Events

### `meeting_started` 🌐

Emitted when a new meeting recording begins.

| When | Inside `start_meeting_session`, after audio capture starts |
|---|---|
| **Payload** | `MeetingSession` |

### `meeting_stopped` 🌐

Emitted when recording stops and the WAV file is finalized.

| When | Inside `stop_meeting_session` |
|---|---|
| **Payload** | `MeetingSession` |

### `meeting_processing` 🌐

Emitted when transcription begins for a session.

| When | After recording stops, and on `retry_transcription` |
|---|---|
| **Payload** | `MeetingSession` |

### `meeting_completed` 🌐 🧵

Emitted when transcription finishes successfully.

| When | From background transcription thread, after saving transcript |
|---|---|
| **Payload** | `MeetingSession` — includes `transcript_path` |

### `meeting_failed` 🌐 🧵

Emitted when transcription or recording fails.

| When | From background thread on transcription error, or from recording thread on mic disconnect |
|---|---|
| **Payload** | `MeetingSession` — includes `error_message` |

### `meeting_summary_status` 🌐

Emitted during summary generation to report progress.

| When | Inside `generate_meeting_summary`, e.g. "Starting Ollama server...", "Downloading model ..." |
|---|---|
| **Payload** | `String` — human-readable status message |

### `meeting_summary_generated` 🌐

Emitted when an AI summary has been generated and saved.

| When | After `generate_meeting_summary` completes |
|---|---|
| **Payload** | `MeetingSession` — includes `summary_path` |

### `mic_disconnected` 🌐 🧵

Emitted when the microphone device is lost or disconnected during recording.

| When | From recording thread on audio input error |
|---|---|
| **Payload** | `{ "session_id": String, "error_message": String, "partial_audio_saved": bool }` |

---

## Overlay Window Events

### `show-overlay` 🪟

Emitted to the overlay window to show a floating status indicator.

| When | Recording starts, transcription starts |
|---|---|
| **Payload** | `String` — `"recording"` or `"transcribing"` |

### `hide-overlay` 🪟

Emitted to the overlay window to dismiss the floating indicator.

| When | Transcription completes or is cancelled |
|---|---|
| **Payload** | `()` — empty/null |

### `mic-level` 🌐 🪟

Emitted to both the main and overlay windows with current microphone signal levels.

| When | Periodically during recording |
|---|---|
| **Payload** | `Vec<f32>` — array of per-channel RMS levels |

---

## History Events

### `history-updated` 🌐

Emitted when the history database changes.

| When | After inserting, updating, or deleting a history entry (legacy push-to-talk path) |
|---|---|
| **Payload** | `()` — empty/null |

---

## Authentication Events

### `chatgpt-login-success` 🌐

Emitted when the user completes ChatGPT Plus sign-in in the login webview.

| When | Called from `complete_chatgpt_login` |
|---|---|
| **Payload** | `{ "access_token": String }` |

---

## Update Events

### `check-for-updates` 🌐

Emitted to trigger an update check notification.

| When | On app startup (periodic), and when manually triggered |
|---|---|
| **Payload** | `()` — empty/null |

---

## Ollama Events

### `ollama_pull_progress` 🌐

Emitted while downloading/streaming an Ollama model during auto-pull.

| When | Inside `generate_meeting_summary` when auto-pulling a missing Ollama model |
|---|---|
| **Payload** | `OllamaPullProgress { model: String, status: String, total: Option<u64>, completed: Option<u64>, percent: Option<f64> }` |
