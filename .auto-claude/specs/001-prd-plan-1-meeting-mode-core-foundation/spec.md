# Specification: Meeting Mode Core Foundation (PLAN 1)

## Overview

This specification defines the implementation of **Meeting Mode** for the Meetdy application - a session-based meeting recorder that operates completely separate from the existing Quick Dictation functionality. Meeting Mode allows users to record entire meeting sessions, automatically generating transcripts after recording stops. Unlike Quick Dictation (which is designed for short voice snippets), Meeting Mode handles continuous long-form recordings with session-based persistence. After PLAN 1 completion, Meetdy will function as a valid meeting recorder even without AI summary capabilities.

## Workflow Type

**Type**: feature

**Rationale**: This is a major new feature that requires:
- New Rust backend managers for meeting session handling
- New React components for Meeting Mode UI
- New state management with Zustand
- New database tables for meeting sessions
- Integration with existing STT pipeline

The feature is additive and must not modify existing dictation functionality.

## Task Scope

### Services Involved
- **main** (primary) - Full-stack Tauri app with React frontend and Rust backend

### This Task Will:
- [ ] Create Meeting Mode toggle to switch between Quick Dictation and Meeting Mode
- [ ] Implement session-based audio recording with file persistence
- [ ] Build meeting session lifecycle state machine (Idle → Recording → Processing → Completed/Failed)
- [ ] Create MeetingSessionManager in Rust backend
- [ ] Add meeting session data model and database table
- [ ] Build Meeting Mode UI with Start/Stop controls and state indicators
- [ ] Integrate with existing STT pipeline for post-recording transcription
- [ ] Implement crash-resilient data persistence
- [ ] Add proper error handling for microphone/transcription failures

### Out of Scope:
- AI summary generation
- Action items extraction
- Search/history UI for meetings
- Cloud sync
- Multi-speaker detection/diarization
- Transcript formatting/beautification
- Real-time transcription during recording

## Service Context

### Main Service (Tauri App)

**Tech Stack:**
- Language: TypeScript (frontend) + Rust (backend)
- Framework: React + Tauri v2
- Build Tool: Vite
- Styling: Tailwind CSS
- State Management: Zustand
- Database: SQLite (via rusqlite)
- Key directories:
  - `src/` - React frontend source
  - `src/components/` - React components
  - `src/stores/` - Zustand stores
  - `src-tauri/src/` - Rust backend source
  - `src-tauri/src/managers/` - Backend managers (audio, transcription, history, model)
  - `src-tauri/src/commands/` - Tauri commands exposed to frontend

**Entry Point:** `src/App.tsx` (frontend), `src-tauri/src/main.rs` (backend)

**How to Run:**
```bash
npm run dev  # Starts Vite dev server + Tauri
# Or: cargo tauri dev
```

**Port:** 3000 (Vite dev server)

## Files to Modify

| File | Service | What to Change |
|------|---------|---------------|
| `src/App.tsx` | main | Add MeetingMode view routing and mode toggle logic |
| `src/components/Sidebar.tsx` | main | Add Meeting Mode section to sidebar navigation |
| `src/stores/settingsStore.ts` | main | Add `current_mode` setting (dictation/meeting) |
| `src-tauri/src/lib.rs` | main | Register new meeting commands |
| `src-tauri/src/managers/mod.rs` | main | Export new MeetingSessionManager |
| `src-tauri/src/settings.rs` | main | Add meeting mode settings |

## Files to Create

| File | Service | Purpose |
|------|---------|---------|
| `src/stores/meetingStore.ts` | main | Zustand store for meeting session state |
| `src/components/meeting/MeetingMode.tsx` | main | Main Meeting Mode container component |
| `src/components/meeting/MeetingControls.tsx` | main | Start/Stop/Timer controls |
| `src/components/meeting/MeetingStatusIndicator.tsx` | main | Recording/Processing state indicator |
| `src/components/meeting/index.ts` | main | Export barrel file |
| `src-tauri/src/managers/meeting.rs` | main | MeetingSessionManager - core session handling |
| `src-tauri/src/commands/meeting.rs` | main | Tauri commands for meeting operations |

## Files to Reference

These files show patterns to follow:

| File | Pattern to Copy |
|------|----------------|
| `src/stores/settingsStore.ts` | Zustand store structure with async actions |
| `src-tauri/src/managers/audio.rs` | Audio recording manager pattern with state machine |
| `src-tauri/src/managers/transcription.rs` | Transcription pipeline integration |
| `src-tauri/src/managers/history.rs` | SQLite database operations and file storage |
| `src/components/Sidebar.tsx` | Sidebar section configuration pattern |
| `src/components/settings/general/GeneralSettings.tsx` | Settings component pattern |

## Patterns to Follow

### Rust Manager Pattern

From `src-tauri/src/managers/audio.rs`:

```rust
#[derive(Clone, Debug)]
pub enum RecordingState {
    Idle,
    Recording { binding_id: String },
}

#[derive(Clone)]
pub struct AudioRecordingManager {
    state: Arc<Mutex<RecordingState>>,
    app_handle: tauri::AppHandle,
    recorder: Arc<Mutex<Option<AudioRecorder>>>,
    is_recording: Arc<Mutex<bool>>,
}

impl AudioRecordingManager {
    pub fn new(app: &tauri::AppHandle) -> Result<Self, anyhow::Error> {
        // Initialize with app handle
    }

    pub fn try_start_recording(&self, binding_id: &str) -> bool {
        // State transition logic
    }

    pub fn stop_recording(&self, binding_id: &str) -> Option<Vec<f32>> {
        // Return audio samples
    }
}
```

**Key Points:**
- Use Arc<Mutex<>> for thread-safe state
- Clone trait for sharing across threads
- State machine enum for lifecycle
- AppHandle for accessing app resources

### Zustand Store Pattern

From `src/stores/settingsStore.ts`:

```typescript
import { create } from "zustand";
import { subscribeWithSelector } from "zustand/middleware";
import { commands } from "@/bindings";

interface MeetingStore {
  // State
  sessionStatus: MeetingStatus;
  currentSession: MeetingSession | null;
  isLoading: boolean;

  // Actions
  startMeeting: () => Promise<void>;
  stopMeeting: () => Promise<void>;

  // Internal setters
  setSessionStatus: (status: MeetingStatus) => void;
}

export const useMeetingStore = create<MeetingStore>()(
  subscribeWithSelector((set, get) => ({
    // State initialization
    sessionStatus: 'idle',
    currentSession: null,
    isLoading: false,

    // Actions that call Rust backend
    startMeeting: async () => {
      const result = await commands.startMeetingSession();
      if (result.status === "ok") {
        set({ sessionStatus: 'recording', currentSession: result.data });
      }
    },
  }))
);
```

**Key Points:**
- Use `subscribeWithSelector` middleware for selective subscriptions
- Separate actions from internal setters
- Use `commands` from bindings for Tauri calls
- Handle result.status === "ok" pattern

### Database Manager Pattern

From `src-tauri/src/managers/history.rs`:

```rust
static MIGRATIONS: &[M] = &[
    M::up(
        "CREATE TABLE IF NOT EXISTS meeting_sessions (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            duration INTEGER,
            status TEXT NOT NULL,
            audio_path TEXT,
            transcript_path TEXT
        );",
    ),
];

pub struct MeetingSessionManager {
    app_handle: AppHandle,
    meetings_dir: PathBuf,
    db_path: PathBuf,
}

impl MeetingSessionManager {
    pub fn new(app_handle: &AppHandle) -> Result<Self> {
        let app_data_dir = app_handle.path().app_data_dir()?;
        let meetings_dir = app_data_dir.join("meetings");
        // Initialize database, ensure directories exist
    }
}
```

**Key Points:**
- Use `rusqlite_migration` for schema migrations
- Store files in `app_data_dir`
- Separate directory for meeting data (not mixing with dictation)

## Requirements

### Functional Requirements

1. **Mode Toggle (FR-01)**
   - Description: Toggle between Quick Dictation (default) and Meeting Mode
   - Acceptance: Only one mode can be active at a time; switching modes stops any ongoing activity

2. **Start Meeting Recording (FR-02)**
   - Description: Start continuous audio recording for a meeting session
   - Acceptance: Recording indicator visible, timer displayed, audio saved to file incrementally

3. **Stop Meeting Recording (FR-03)**
   - Description: Stop recording and trigger transcription processing
   - Acceptance: Audio file saved, processing indicator shown, UI not blocked during transcription

4. **Meeting Session Persistence (FR-04)**
   - Description: Each meeting session saved with metadata, audio, and transcript
   - Acceptance: Data survives app restart; each meeting has unique ID and folder

5. **Post-Recording Transcription (FR-05)**
   - Description: Transcription runs after Stop using existing STT pipeline
   - Acceptance: Raw transcript generated; transcription failure doesn't lose audio

6. **Session State Lifecycle (FR-06)**
   - Description: Implement 5 states: Idle, Recording, Processing, Completed, Failed
   - Acceptance: Cannot start new meeting while Recording; Processing runs in background

7. **Default Meeting Title (FR-07)**
   - Description: Auto-generate title from timestamp, editable after recording
   - Acceptance: Title format like "Meeting - January 15, 2025 3:30 PM"

### Edge Cases

1. **App crash during recording** - Audio must be recoverable from incremental file writes
2. **App crash during processing** - Meeting should be in "Processing" state on restart, retry-able
3. **Disk full** - Graceful error message, no data corruption
4. **Microphone disconnect** - Detect and show error, save whatever audio was captured
5. **Model not loaded** - Show error, keep audio saved for later transcription retry
6. **User cancels during recording** - Option to discard or save partial recording

## Implementation Notes

### DO
- Follow the manager pattern in `src-tauri/src/managers/audio.rs` for MeetingSessionManager
- Reuse `AudioRecorder` from `audio_toolkit` for recording
- Reuse `TranscriptionManager` for STT processing
- Use `rusqlite` for database (same as HistoryManager)
- Save audio incrementally (not in RAM) for crash resilience
- Emit events via `app_handle.emit()` for state changes
- Use WAV format for audio files
- Store meetings in separate `meetings/` directory under app_data_dir

### DON'T
- Don't modify existing dictation recording flow in `AudioRecordingManager`
- Don't add real-time transcription during recording
- Don't buffer entire audio in memory
- Don't share audio files between dictation and meeting modes
- Don't add AI/summary features (out of scope for PLAN 1)

## Data Model

### MeetingSession (Rust)

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct MeetingSession {
    pub id: String,           // UUID
    pub title: String,        // User-editable, default = timestamp
    pub created_at: i64,      // Unix timestamp
    pub duration: Option<i64>, // Duration in seconds (set after stop)
    pub status: MeetingStatus,
    pub audio_path: Option<String>,     // Relative path within meetings dir
    pub transcript_path: Option<String>, // Relative path for transcript
}

#[derive(Clone, Debug, Serialize, Deserialize, Type, PartialEq)]
pub enum MeetingStatus {
    Idle,
    Recording,
    Processing,
    Completed,
    Failed,
}
```

### Database Schema

```sql
CREATE TABLE IF NOT EXISTS meeting_sessions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    duration INTEGER,
    status TEXT NOT NULL DEFAULT 'idle',
    audio_path TEXT,
    transcript_path TEXT,
    error_message TEXT
);
```

### File Storage Structure

```
{app_data_dir}/
├── meetings/
│   ├── {session-id}/
│   │   ├── audio.wav
│   │   └── transcript.txt
│   └── {session-id}/
│       ├── audio.wav
│       └── transcript.txt
└── recordings/          # Existing dictation data (unchanged)
    └── handy-*.wav
```

## Development Environment

### Start Services

```bash
# Install dependencies
npm install

# Start development (Vite + Tauri)
npm run dev

# Or use Tauri CLI directly
cargo tauri dev
```

### Service URLs
- Frontend Dev Server: http://localhost:3000

### Required Environment Variables
- None required for local development
- Rust toolchain must be installed
- Xcode Command Line Tools (macOS) for native builds

## Success Criteria

The task is complete when:

1. [ ] Mode toggle switches between Dictation and Meeting Mode cleanly
2. [ ] Start/Stop meeting recording works reliably
3. [ ] Audio is saved incrementally (crash-resilient)
4. [ ] Transcript is generated after stopping
5. [ ] Meeting sessions persist in SQLite database
6. [ ] Each meeting has its own folder with audio + transcript files
7. [ ] UI shows correct state indicators (Recording/Processing/Completed/Failed)
8. [ ] Existing dictation mode is completely unaffected
9. [ ] Error states handled gracefully with user-friendly messages
10. [ ] No console errors in normal operation
11. [ ] App can be used to record a real meeting (functional validation)

## QA Acceptance Criteria

**CRITICAL**: These criteria must be verified by the QA Agent before sign-off.

### Unit Tests
| Test | File | What to Verify |
|------|------|----------------|
| MeetingSession state transitions | `src-tauri/src/managers/meeting.rs` | State machine transitions are valid (Idle→Recording→Processing→Completed/Failed) |
| Session ID uniqueness | `src-tauri/src/managers/meeting.rs` | Each session gets unique UUID |
| Database CRUD operations | `src-tauri/src/managers/meeting.rs` | Create, read, update meeting sessions in SQLite |

### Integration Tests
| Test | Services | What to Verify |
|------|----------|----------------|
| Start → Stop → Transcribe flow | MeetingSessionManager ↔ TranscriptionManager | Audio recorded, saved, transcribed successfully |
| Frontend ↔ Backend sync | React Store ↔ Tauri Commands | State changes emit events, frontend receives them |
| File persistence | MeetingSessionManager ↔ FileSystem | Audio and transcript files created in correct directories |

### End-to-End Tests
| Flow | Steps | Expected Outcome |
|------|-------|------------------|
| Complete meeting recording | 1. Switch to Meeting Mode 2. Click Start 3. Speak for 30s 4. Click Stop 5. Wait for processing | Meeting saved with audio + transcript |
| Dictation unaffected | 1. Use dictation mode 2. Switch to Meeting Mode 3. Switch back to dictation 4. Use dictation | Dictation works exactly as before |
| Error recovery | 1. Start meeting 2. Remove microphone 3. Check error handling | Error shown, partial audio saved |

### Browser Verification (Frontend)
| Page/Component | URL | Checks |
|----------------|-----|--------|
| Meeting Mode UI | `http://localhost:3000` (Meeting tab) | Start/Stop buttons render, timer displays correctly |
| Mode Toggle | `http://localhost:3000` | Toggle switches modes, UI updates accordingly |
| State Indicators | `http://localhost:3000` | Recording indicator (red dot), Processing spinner visible |
| Error States | `http://localhost:3000` | Error messages display for microphone/transcription failures |

### Database Verification
| Check | Query/Command | Expected |
|-------|---------------|----------|
| Table created | `sqlite3 history.db ".schema meeting_sessions"` | Schema matches spec |
| Session saved | `SELECT * FROM meeting_sessions` | Session record with correct fields |
| Status updates | `SELECT status FROM meeting_sessions WHERE id = ?` | Status progresses through lifecycle |

### File System Verification
| Check | Path | Expected |
|-------|------|----------|
| Meetings directory | `{app_data}/meetings/` | Directory exists |
| Session folder | `{app_data}/meetings/{session-id}/` | Folder created per session |
| Audio file | `{session-folder}/audio.wav` | Valid WAV file, playable |
| Transcript file | `{session-folder}/transcript.txt` | Text file with transcription content |

### QA Sign-off Requirements
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] All E2E tests pass
- [ ] Browser verification complete
- [ ] Database state verified
- [ ] File storage verified
- [ ] Mode toggle works correctly
- [ ] Existing dictation functionality unaffected
- [ ] No regressions in existing functionality
- [ ] Code follows established patterns (managers, stores, components)
- [ ] No security vulnerabilities introduced
- [ ] Error handling covers all edge cases
- [ ] Performance acceptable (no recording lag)

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        FRONTEND (React)                         │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────────┐  ┌────────────────────┐  │
│  │ App.tsx     │  │ MeetingMode.tsx  │  │ MeetingControls    │  │
│  │ (routing)   │  │ (container)      │  │ (Start/Stop/Timer) │  │
│  └──────┬──────┘  └────────┬─────────┘  └─────────┬──────────┘  │
│         │                  │                      │              │
│  ┌──────┴──────────────────┴──────────────────────┴───────────┐  │
│  │                    meetingStore.ts (Zustand)               │  │
│  │     - sessionStatus, currentSession, actions               │  │
│  └────────────────────────────┬───────────────────────────────┘  │
└───────────────────────────────┼─────────────────────────────────┘
                                │ Tauri Commands
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        BACKEND (Rust)                           │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                 commands/meeting.rs                         │ │
│  │   start_meeting(), stop_meeting(), get_meeting_status()     │ │
│  └─────────────────────────────┬───────────────────────────────┘ │
│                                │                                 │
│  ┌─────────────────────────────┴───────────────────────────────┐ │
│  │              managers/meeting.rs                            │ │
│  │         MeetingSessionManager                               │ │
│  │   - State machine (Idle/Recording/Processing/Complete)      │ │
│  │   - Audio file writing                                      │ │
│  │   - Session persistence                                     │ │
│  └───────────┬─────────────────────────────────┬───────────────┘ │
│              │                                 │                 │
│  ┌───────────▼───────────────┐   ┌─────────────▼─────────────┐  │
│  │  AudioRecorder            │   │  TranscriptionManager     │  │
│  │  (existing, reused)       │   │  (existing, reused)       │  │
│  └───────────────────────────┘   └───────────────────────────┘  │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                      SQLite Database                        │ │
│  │              meeting_sessions table                         │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │                     File System                             │ │
│  │   {app_data}/meetings/{session-id}/audio.wav                │ │
│  │   {app_data}/meetings/{session-id}/transcript.txt           │ │
│  └─────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Order

1. **Backend Foundation** (Rust)
   - Create `MeetingSessionManager` struct and state enum
   - Implement database table migration
   - Create session folder/file management

2. **Recording Pipeline** (Rust)
   - Integrate with existing `AudioRecorder`
   - Implement incremental WAV file writing
   - Add start/stop recording methods

3. **Transcription Integration** (Rust)
   - Add post-recording transcription trigger
   - Implement background processing
   - Handle transcription failures gracefully

4. **Tauri Commands** (Rust)
   - Expose commands: start_meeting, stop_meeting, get_meeting_status
   - Generate TypeScript bindings

5. **Frontend Store** (TypeScript)
   - Create `useMeetingStore` with Zustand
   - Implement state sync with backend events

6. **UI Components** (React)
   - Build MeetingMode container
   - Create MeetingControls with timer
   - Add StatusIndicator component

7. **Integration & Polish**
   - Add mode toggle to sidebar
   - Wire up routing in App.tsx
   - Error handling and edge cases

---

*End of Specification*
