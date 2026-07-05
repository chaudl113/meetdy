# Shared Types

TypeScript bindings for these types are auto-generated in `src/bindings.ts` by `tauri-specta`.
The schemas below reflect the Rust-side definitions.

---

## `AppSettings`

Full application settings persisted via Tauri store plugin.

```typescript
interface AppSettings {
  bindings: Record<string, ShortcutBinding>;
  push_to_talk: boolean;
  audio_feedback: boolean;
  audio_feedback_volume: number;              // f32, 0.0–1.0
  sound_theme: SoundTheme;
  start_hidden: boolean;
  autostart_enabled: boolean;
  update_checks_enabled: boolean;
  selected_model: string;
  always_on_microphone: boolean;
  selected_microphone: string | null;
  clamshell_microphone: string | null;
  selected_output_device: string | null;
  translate_to_english: boolean;
  selected_language: string;
  overlay_position: OverlayPosition;
  debug_mode: boolean;
  log_level: LogLevel;
  custom_words: string[];
  model_unload_timeout: ModelUnloadTimeout;
  word_correction_threshold: number;          // f64
  history_limit: number;                      // usize
  recording_retention_period: RecordingRetentionPeriod;
  paste_method: PasteMethod;
  clipboard_handling: ClipboardHandling;
  post_process_enabled: boolean;
  post_process_provider_id: string;
  post_process_providers: PostProcessProvider[];
  post_process_api_keys: Record<string, string>;
  post_process_models: Record<string, string>;
  post_process_prompts: LLMPrompt[];
  post_process_selected_prompt_id: string | null;
  mute_while_recording: boolean;
  append_trailing_space: boolean;
  app_language: string;
  meeting_templates: MeetingTemplate[];
}
```

### `ShortcutBinding`

```typescript
interface ShortcutBinding {
  id: string;
  name: string;
  description: string;
  default_binding: string;
  current_binding: string;
}
```

### `PostProcessProvider`

```typescript
interface PostProcessProvider {
  id: string;
  label: string;
  base_url: string;
  requires_api_key: boolean;
  default_model: string | null;
}
```

### `LLMPrompt`

```typescript
interface LLMPrompt {
  id: string;
  name: string;
  prompt: string;
}
```

---

## Enums

### `SoundTheme`

```typescript
type SoundTheme = "marimba" | "pop" | "custom";
```

### `OverlayPosition`

```typescript
type OverlayPosition = "none" | "top" | "bottom";
```

### `LogLevel`

```typescript
type LogLevel = "trace" | "debug" | "info" | "warn" | "error";
```

### `ModelUnloadTimeout`

Controls idle timeout before the transcription model is unloaded from memory.

```typescript
type ModelUnloadTimeout =
  | "never"         // Model stays loaded indefinitely
  | "immediately"   // Unload right after transcription
  | "min_2"
  | "min_5"
  | "min_10"
  | "min_15"
  | "hour_1"
  | "sec_5";        // Debug mode only — 5 seconds
```

### `PasteMethod`

```typescript
type PasteMethod = "ctrl_v" | "direct" | "none" | "shift_insert" | "ctrl_shift_v";
```

### `ClipboardHandling`

```typescript
type ClipboardHandling = "dont_modify" | "copy_to_clipboard";
```

### `RecordingRetentionPeriod`

```typescript
type RecordingRetentionPeriod =
  | "never"           // Keep all recordings
  | "preserve_limit"  // Respect history_limit
  | "days_3"
  | "weeks_2"
  | "months_3";
```

---

## Model Types

### `ModelInfo`

```typescript
interface ModelInfo {
  id: string;
  name: string;
  description: string;
  filename: string;
  url: string | null;
  size_mb: number;            // u64
  is_downloaded: boolean;
  is_downloading: boolean;
  partial_size: number;       // u64 — bytes downloaded so far
  is_directory: boolean;      // true if model is stored as a directory
  engine_type: EngineType;
  accuracy_score: number;     // f32, 0.0–1.0
  speed_score: number;        // f32, 0.0–1.0
}
```

### `EngineType`

```typescript
type EngineType = "whisper" | "parakeet";
```

### `DownloadProgress`

```typescript
interface DownloadProgress {
  model_id: string;
  downloaded: number;  // u64 — bytes
  total: number;       // u64 — bytes
  percentage: number;  // f64 — 0.0–100.0
}
```

### `ModelLoadStatus`

```typescript
interface ModelLoadStatus {
  is_loaded: boolean;
  current_model: string | null;
}
```

---

## Meeting Types

### `MeetingSession`

```typescript
interface MeetingSession {
  id: string;                          // UUID
  title: string;
  created_at: number;                  // i64 — unix timestamp (seconds)
  duration: number | null;             // i64 — recording duration in seconds
  status: MeetingStatus;
  audio_path: string | null;           // relative path, e.g. "{id}/audio.wav"
  transcript_path: string | null;      // relative path, e.g. "{id}/transcript.txt"
  error_message: string | null;
  audio_source: AudioSourceType;
  summary_path: string | null;         // relative path, e.g. "{id}/summary.md"
  template_id: string | null;
}
```

### `MeetingStatus`

```typescript
type MeetingStatus =
  | "idle"        // No active session
  | "recording"   // Audio capture in progress
  | "processing"  // Transcription running
  | "completed"   // Transcription succeeded
  | "failed"      // Transcription or recording error
  | "interrupted"; // App closed during recording
```

**State machine:**

```
Idle → Recording → Processing → Completed
                    ↘ Failed →   Processing (retry)
Recording → Interrupted → Processing (resume on next launch)
```

### `AudioSourceType`

```typescript
type AudioSourceType = "microphone_only" | "system_only" | "mixed";
```

- `microphone_only` — Capture only microphone input (default).
- `system_only` — Capture only system audio (macOS 13.0+).
- `mixed` — Capture both microphone and system audio mixed (macOS 13.0+).

### `MeetingNote`

```typescript
interface MeetingNote {
  id: string;                 // UUID
  session_id: string;         // Parent session ID
  timestamp_seconds: number;  // i64 — offset from recording start
  content: string;
  author: string | null;      // Reserved for multi-user support
  created_at: number;         // i64 — unix timestamp (seconds)
}
```

### `MeetingTemplate`

```typescript
interface MeetingTemplate {
  id: string;
  name: string;
  icon: string;
  title_template: string;                // May contain {date}, {time}
  audio_source: string;                  // "microphone_only" | "system_only" | "mixed"
  prompt_id: string | null;
  summary_prompt_template: string | null; // Must contain {} placeholder
  created_at: number;                    // i64 — unix timestamp (seconds)
  updated_at: number;                    // i64 — unix timestamp (seconds)
}
```

---

## Audio Types

### `AudioDevice`

```typescript
interface AudioDevice {
  index: string;
  name: string;
  is_default: boolean;
}
```

### `CustomSounds`

```typescript
interface CustomSounds {
  start: boolean;  // true if custom_start.wav exists
  stop: boolean;   // true if custom_stop.wav exists
}
```

---

## History Types

### `HistoryEntry`

```typescript
interface HistoryEntry {
  id: number;                     // i64 — auto-increment
  file_name: string;
  timestamp: number;              // i64 — unix timestamp (seconds)
  saved: boolean;
  title: string;
  transcription_text: string;
  post_processed_text: string | null;
  post_process_prompt: string | null;
}
```

---

## Ollama Types

### `OllamaPullProgress`

Emitted during `ollama_pull_progress` events.

```typescript
interface OllamaPullProgress {
  model: string;
  status: string;
  total: number | null;      // u64 — total bytes
  completed: number | null;  // u64 — completed bytes
  percent: number | null;    // f64 — 0.0–100.0
}
```

---

## Auth Types

### `ChatgptLoginEvent`

Emitted with the `chatgpt-login-success` event.

```typescript
interface ChatgptLoginEvent {
  access_token: string;
}
```
