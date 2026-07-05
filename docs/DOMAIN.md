# Domain Model — Meetdy

Cross-platform desktop speech-to-text application (Tauri + React/TypeScript).

## Bounded Contexts

- **Audio** — device enumeration, recording, resampling, audio I/O
- **Transcription** — VAD + Whisper/Parakeet inference pipeline
- **Model Management** — download, cache, select Whisper/Parakeet models
- **Settings** — user preferences, shortcuts, device selection, i18n
- **History** — transcription history storage and retrieval
- **Overlay** — recording UI (progress, cancel, status)

## Core Entities

| Entity | Context | Identity | Lifecycle |
|--------|---------|----------|-----------|
| Recording | Audio | Device + timestamp | Start → Capturing → VAD segments → Stop |
| Transcription | Transcription | Recording + model | Queued → Processing → Completed → Pasted |
| Model | Model Management | Model variant + size | Downloading → Downloaded → Selected → Removed |
| Settings | Settings | User profile | Default → Customized → Persisted |

## Integration Patterns

- **Audio → Transcription**: Audio sends PCM data → Transcription runs VAD + Whisper
- **Transcription → Clipboard**: Completed text → system clipboard paste
- **Model Management → Transcription**: Selected model → loaded into inference engine
- **Settings → All**: Tauri store plugin → Rust state → all managers

## State Machines

### Recording
```
Idle → [Push-to-Talk key down] → Capturing → [VAD detects speech] → Active
Active → [Push-to-Talk key up or silence] → Processing → Idle
```

### Model Lifecycle
```
Available → [Download] → Downloading → Downloaded → [Select] → Active
Active → [Delete] → Removed
```

## Technology Stack

- **Backend**: Rust (Tauri 2.x) — `src-tauri/src/`
- **Frontend**: React + TypeScript + Vite — `src/`
- **Inference**: whisper-rs (Whisper), Parakeet
- **Audio**: cpal, rubato, rodio
- **VAD**: Silero VAD via vad-rs
- **State**: Zustand (frontend), Tauri store plugin (persistence)
- **i18n**: i18next with 4 locales (en, es, fr, vi)
