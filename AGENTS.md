# AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

**Prerequisites:**

- [Rust](https://rustup.rs/) (latest stable)
- [Bun](https://bun.sh/) package manager

**Core Development:**

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev
# If cmake error on macOS:
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev

# Build for production
bun run tauri build

# Frontend only development
bun run dev        # Start Vite dev server
bun run build      # Build frontend (TypeScript + Vite)
bun run preview    # Preview built frontend
```

**Model Setup (Required for Development):**

```bash
# Create models directory
mkdir -p src-tauri/resources/models

# Download required VAD model
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
```

## Architecture Overview

Handy is a cross-platform desktop speech-to-text application built with Tauri (Rust backend + React/TypeScript frontend).

### Core Components

**Backend (Rust - src-tauri/src/):**

- `lib.rs` - Main application entry point with Tauri setup, tray menu, and managers
- `managers/` - Core business logic managers:
  - `audio.rs` - Audio recording and device management
  - `model.rs` - Whisper model downloading and management
  - `transcription.rs` - Speech-to-text processing pipeline
- `audio_toolkit/` - Low-level audio processing:
  - `audio/` - Device enumeration, recording, resampling
  - `vad/` - Voice Activity Detection using Silero VAD
- `commands/` - Tauri command handlers for frontend communication
- `shortcut.rs` - Global keyboard shortcut handling
- `settings.rs` - Application settings management

**Frontend (React/TypeScript - src/):**

- `App.tsx` - Main application component with onboarding flow
- `components/settings/` - Settings UI components
- `components/model-selector/` - Model management interface
- `hooks/` - React hooks for settings and model management
- `lib/types.ts` - Shared TypeScript type definitions

### Key Architecture Patterns

**Manager Pattern:** Core functionality is organized into managers (Audio, Model, Transcription) that are initialized at startup and managed by Tauri's state system.

**Command-Event Architecture:** Frontend communicates with backend via Tauri commands, backend sends updates via events.

**Pipeline Processing:** Audio → VAD → Whisper → Text output with configurable components at each stage.

### Technology Stack

**Core Libraries:**

- `whisper-rs` - Local Whisper inference with GPU acceleration
- `cpal` - Cross-platform audio I/O
- `vad-rs` - Voice Activity Detection
- `rdev` - Global keyboard shortcuts
- `rubato` - Audio resampling
- `rodio` - Audio playback for feedback sounds

**Platform-Specific Features:**

- macOS: Metal acceleration for Whisper, accessibility permissions
- Windows: Vulkan acceleration, code signing
- Linux: OpenBLAS + Vulkan acceleration

### Application Flow

1. **Initialization:** App starts minimized to tray, loads settings, initializes managers
2. **Model Setup:** First-run downloads preferred Whisper model (Small/Medium/Turbo/Large)
3. **Recording:** Global shortcut triggers audio recording with VAD filtering
4. **Processing:** Audio sent to Whisper model for transcription
5. **Output:** Text pasted to active application via system clipboard

### Settings System

Settings are stored using Tauri's store plugin with reactive updates:

- Keyboard shortcuts (configurable, supports push-to-talk)
- Audio devices (microphone/output selection)
- Model preferences (Small/Medium/Turbo/Large Whisper variants)
- Audio feedback and translation options

### Single Instance Architecture

The app enforces single instance behavior - launching when already running brings the settings window to front rather than creating a new process.

## Multi-Agent Swarm Protocol

This project uses a swarm orchestration model. The default agent is `orchestrator` (see `.opencode/agent/orchestrator.md`). Non-trivial work MUST be delegated to specialized subagents — never implemented directly by the orchestrator.

**Available subagents:** `architect`, `backend`, `frontend`, `database`, `security`, `devops`, `qa`, `docs`, `reviewer`, `git`.

**Mandatory workflow for any multi-step task:**

1. **Understand** — extract goals, requirements, constraints, dependencies, risks.
2. **Plan** — build a task graph with explicit dependencies and owner agents.
3. **Delegate in waves** — run independent tasks in parallel; never start a blocked task.
4. **Minimize context** — each subagent receives only its assigned files and required context.
5. **Verify** — confirm files changed, tests run, builds pass. Never trust claims.
6. **Review** — `reviewer` agent must be different from the author. Reviewer cannot approve own work.
7. **Recover** — on failure: retry once, then spawn a debug/root-cause agent before continuing.
8. **Report** — produce a final report (Goal / Task Graph / Agents Used / Waves / Files Changed / Tests / Risks / Next Steps).

**Agent boundaries:**

- `architect`, `security`, `reviewer`, `git` are read-only for source code.
- `backend` edits only `src-tauri/` and server-side TS.
- `frontend` edits only `src/` UI files.
- `database` owns migrations and schema.
- `devops` owns `.github/`, build scripts, `tauri.conf.json`, infra.
- `qa` edits only test files.
- `docs` edits only `*.md` and docstrings.

Cross-boundary edits require explicit orchestrator approval.
Cross-boundary edits require explicit orchestrator approval.

<!-- HARNESS:BEGIN -->
## Harness

This repo uses Harness. Before work, read:

- `README.md`
- `docs/HARNESS.md`
- `docs/FEATURE_INTAKE.md`
- `docs/ARCHITECTURE.md`
- `docs/CONTEXT_RULES.md`
- `docs/CONTEXT.md` (domain glossary)
- `docs/DOMAIN.md` (domain model)
- `docs/TOOL_REGISTRY.md`
- `docs/TEST_MATRIX.md`
- `scripts/bin/harness-cli query matrix` on macOS/Linux, or `.\scripts\bin\harness-cli.exe query matrix` on Windows

Use the Rust Harness CLI at `scripts/bin/harness-cli` on macOS/Linux or
`scripts/bin/harness-cli.exe` on Windows as the main operational tool.

## Project Docs

- `docs/ADR/` — architecture decision records (application-level)
- `docs/decisions/` — Harness infrastructure decisions
- `docs/API/` — Tauri command contracts + event schemas
- `docs/RUNBOOK/` — development, build, model setup procedures

## Skills

Skills live in `.claude/skills/`:

| Category | User-invoked (type `/name`) | Model-invoked (auto) |
|---|---|---|
| Engineering | `grill-with-docs`, `to-prd`, `to-issues`, `improve-codebase-architecture`, `setup-matt-pocock-skills`, `triage` | `tdd`, `diagnosing-bugs`, `codebase-design`, `domain-modeling`, `prototype`, `code-review` |
| Productivity | `handoff` | `grilling` |
| Project | (add project-specific skills here) | — |
<!-- HARNESS:END -->

<!-- maestro:start -->
# Maestro Harness Protocol
Read .maestro/harness/HARNESS.md first before working in this repo.
<!-- maestro:end -->
