# Meetdy Context

Shared domain language for the Meetdy project — a cross-platform desktop speech-to-text application.

## Language

**Transcription**:
The process of converting spoken audio into written text via Whisper/Parakeet models.
_Avoid_: speech-to-text result, STT output

**Pipeline**:
The audio processing chain: Audio Capture → VAD → Model Inference → Text Output.
_Avoid_: flow, processing chain

**VAD** (Voice Activity Detection):
Silero VAD model that detects speech segments in audio, filtering silence.
_Avoid_: silence detection, speech detection

**Push-to-Talk**:
Recording mode where user holds a keyboard shortcut to record, releasing to stop.
_Avoid_: PTT, manual recording

**Model**:
A Whisper or Parakeet model variant (Small, Medium, Turbo, Large) used for inference.
_Avoid_: engine, AI model

**Overlay**:
The floating UI window shown during recording (progress bar, cancel button, status).
_Avoid_: recording popup, HUD

**Tauri**:
The Rust framework bridging the frontend (React/TypeScript) and backend (Rust) via IPC commands.
_Avoid_: framework, shell

**Command**:
A Tauri IPC function call from frontend to backend (Rust), e.g. `start_recording`, `get_models`.
_Avoid_: API call, RPC

**Event**:
A Tauri event pushed from backend to frontend, e.g. `transcription-progress`, `model-downloaded`.
_Avoid_: notification, message

**Manager**:
A Rust module owning a specific subsystem (Audio, Model, Transcription, History).
_Avoid_: service, handler, controller

**Settings**:
User preferences persisted via Tauri store plugin — shortcuts, devices, models, audio options.
_Avoid_: config, preferences

**Clipboard**:
System clipboard where transcribed text is pasted after processing.
_Avoid_: paste buffer

**i18n**:
Internationalization via i18next — all user-facing strings in `src/i18n/locales/`.
_Avoid_: l10n, translation system

**Whisper**:
OpenAI's speech recognition model, run locally via whisper-rs with Metal/Vulkan GPU acceleration.
_Avoid_: STT engine, speech engine

**Parakeet**:
Alternative speech recognition engine supporting multiple model formats.
_Avoid_: model engine
