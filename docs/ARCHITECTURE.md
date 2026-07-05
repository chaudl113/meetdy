# Architecture

Meetdy is a cross-platform desktop speech-to-text application built with **Tauri** (Rust backend + React/TypeScript frontend).

## High-Level Stack

| Layer | Technology |
|-------|-----------|
| Desktop Shell | Tauri v2 (Rust) |
| Frontend UI | React + TypeScript + Vite |
| Audio Capture | cpal + rubato (resampling) |
| Voice Detection | Silero VAD (ONNX via vad-rs) |
| Speech Recognition | Whisper (whisper-rs) / Parakeet |
| IPC | Tauri Commands + Events |
| Settings | Tauri Store plugin |
| i18n | i18next |

## Core Pipeline

```
Audio Input → VAD (Silero) → Whisper/Parakeet Inference → Text → Clipboard
```

1. **Audio Capture**: Records from selected microphone via cpal, resampled to 16kHz mono with rubato.
2. **Voice Activity Detection**: Silero VAD ONNX model filters silence, segments speech chunks.
3. **Inference**: Whisper or Parakeet model transcribes speech to text locally (Metal on macOS, Vulkan on Windows/Linux).
4. **Output**: Transcribed text is pasted into the active application via system clipboard.

## Manager Pattern

Backend functionality is organized into **Managers** — Rust modules that own a specific subsystem:

| Manager | Responsibility |
|---------|---------------|
| `AudioManager` | Audio device enumeration, recording, playback |
| `ModelManager` | Whisper/Parakeet model downloading and lifecycle |
| `TranscriptionManager` | Pipeline orchestration, VAD → model inference |

Managers are initialized at startup and held in Tauri's managed state. Each wraps platform-specific code behind a common trait interface.

## Command-Event Architecture

The frontend and backend communicate via two Tauri IPC mechanisms:

- **Commands** (`src-tauri/src/commands/`): Frontend-initiated calls to the Rust backend. Examples: `start_recording`, `get_models`, `update_settings`.
- **Events**: Backend-pushed updates to the frontend. Examples: `transcription-progress`, `model-downloaded`, `recording-started`.

This decouples the UI from backend state changes — the frontend subscribes to events it cares about without polling.

## Directory Structure

```
src/                      # React/TypeScript frontend
  components/             # UI components (settings, model-selector, overlay)
  hooks/                  # React hooks for settings and models
  i18n/locales/           # Translation files
  lib/                    # Shared types and utilities

src-tauri/                # Rust backend
  src/
    lib.rs                # App entry point, tray menu, manager init
    commands/             # Tauri command handlers
    managers/             # Core business logic managers
    audio_toolkit/        # Low-level audio processing
      audio/              # Device enumeration, recording, resampling
      vad/                # Silero VAD integration
    shortcut.rs           # Global keyboard shortcut handling
    settings.rs           # Settings persistence
```

## Platform-Specific Acceleration

| Platform | Inference Backend |
|----------|------------------|
| macOS | Metal (via whisper-rs) |
| Windows | Vulkan (via whisper-rs) |
| Linux | OpenBLAS + Vulkan |

## Application Lifecycle

1. App starts minimized to system tray.
2. Loads settings, initializes managers, downloads model if not present.
3. User presses global shortcut to start/stop recording (push-to-talk mode).
4. Audio is streamed through VAD → Whisper → text output.
5. Result is pasted to the active application.

## Key Design Decisions

- **Single Instance**: Enforced via Tauri — re-launching brings the settings window to front.
- **Local Inference**: All processing runs on-device; no cloud dependency.
- **Push-to-Talk**: Recording triggered by global keyboard shortcut, not continuous.
- **Settings as Store**: Tauri store plugin provides reactive settings persistence with JSON backend.
