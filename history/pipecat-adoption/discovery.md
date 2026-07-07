# Discovery Report: Adopt Pipecat Patterns in meetdy

Date: 2026
Scope: Apply useful patterns from `pipecat-ai/pipecat` (Python real-time voice agent framework) to meetdy (Tauri + Rust meeting recorder/transcriber).

## Architecture Snapshot

### Top-level layout

- `src-tauri/` — Rust backend (Tauri host).
- `src/` — Frontend (Vite + TS + React).
- Plan target: `src-tauri/src/` only (frontend untouched unless new events added).

### Key Rust modules

| Module | Lines | Role |
|---|---|---|
| `audio_toolkit/audio/recorder.rs` | — | Mic capture via `cpal`. |
| `audio_toolkit/system_audio.rs` | — | System audio (ScreenCaptureKit on macOS). |
| `audio_toolkit/mixed_recorder.rs` | 504 | Merges mic + system audio; computes live `AudioStats`. |
| `audio_toolkit/vad/silero.rs` | 52 | Silero VAD wrapper (`vad-rs`). Frame = 30ms @ 16kHz. |
| `audio_toolkit/vad/smoothed.rs` | 105 | Onset/hangover smoothing around Silero. |
| `managers/audio.rs` | 444 | OS-level mute, device listing, clamshell handling. |
| `managers/model.rs` | 784 | Model registry (Whisper variants + Parakeet). Download/extract. |
| `managers/transcription.rs` | 544 | Loads `transcribe-rs` engine (Whisper/Parakeet), idle unload, calls `transcribe()`. |
| `managers/meeting/manager.rs` | **2235** | Meeting lifecycle, recording loop, VAD, STT call, DB writes, emits events. |
| `managers/meeting/db.rs` | — | SQLite (rusqlite) for sessions/notes. |
| `managers/meeting/wav_writer.rs` | — | Streaming WAV writer. |
| `managers/history.rs` | — | Quick-record (non-meeting) history. |
| `managers/meeting_logger.rs` | — | Ad-hoc structured log/perf metric helpers. |
| `llm_client.rs` | 358 | Cloud LLM client (used for summary). |
| `ollama.rs` | 422 | Local Ollama runner for summary. |
| `commands/meeting.rs` | — | Tauri commands; orchestrates manager + LLM summary. |

### Pipeline today (de-facto)

```
cpal mic ─┐
          ├─> MixedAudioRecorder ─> resample 16kHz f32
sckit sys ┘                          │
                                     ▼
                          SmoothedVad(SileroVad)
                                     │ utterance buffer
                                     ▼
                    TranscriptionManager.transcribe()
                       (transcribe-rs: Whisper | Parakeet)
                                     │ text
                                     ▼
                    apply_custom_words + strip_noise
                                     │
                                     ▼
                  emit("meeting_live_transcript")
                  append to live_transcripts[session]
                                     │
                                     ▼
                          SQLite + WAV on stop
```

Live partial transcript event already exists (`meeting_live_transcript`), but only emitted at utterance boundaries — not while user is speaking.

## Existing Patterns

- VAD: `SileroVad` (boolean) wrapped by `SmoothedVad` (onset/hangover/prefill). Pipecat-equivalent: `SileroVADAnalyzer`. We already have this.
- Engine abstraction: `LoadedEngine { Whisper(WhisperEngine), Parakeet(ParakeetEngine) }` enum. Not a trait — adding new providers means editing the enum.
- Events: Tauri `emit("meeting_*", payload)`. Pipecat-equivalent: `Frame` over queues. meetdy uses ad-hoc event names per concern.
- LLM: Two paths (`llm_client.rs` cloud, `ollama.rs` local), selected in `commands/meeting.rs` summary flow. Provider switching is conditional, not trait-based.
- Audio stats throttled 100ms → `emit("meeting_audio_stats")`. Pipecat-equivalent: observability frames.

## Technical Constraints

- Rust edition: stable; Tauri v2 (per `src-tauri/Cargo.toml` style).
- Crates already present: `cpal`, `vad-rs`, `hound`, `rusqlite`, `tauri`, `log`, `transcribe-rs`.
- Realtime budget: utterance latency ≤ 1.5s perceived. Mac M-series target.
- Privacy: app sells on local-first. Any cloud STT MUST be opt-in and key in OS keychain.
- macOS-first; Windows/Linux supported best-effort (mute logic already gates by OS).
- Frontend already consumes `meeting_live_transcript` — backward-compat needed.

## External References (Pipecat patterns)

| Pattern | Pipecat module | meetdy mapping |
|---|---|---|
| Frame-based pipeline | `pipecat.pipeline.pipeline.Pipeline`, `FrameProcessor` | New `audio_toolkit/pipeline/` |
| VAD analyzer | `SileroVADAnalyzer` | Already in `audio_toolkit/vad/` (tune + expose) |
| Audio denoise | Krisp / RNNoise / Koala filters | Add `audio_toolkit/audio/denoise.rs` using `nnnoiseless` |
| STT service trait | `STTService` base class | New `managers/stt/` trait + adapters |
| Cloud STT providers | Groq, Deepgram, OpenAI, etc. | New `managers/stt/groq.rs`, `deepgram.rs` (opt-in) |
| LLM service trait | `LLMService` base | Refactor `llm_client.rs` + `ollama.rs` into trait |
| Streaming partial transcripts | `InterimTranscriptionFrame` | New event `meeting_partial_transcript` |
| Observability | OpenTelemetry processor | `tracing` spans + structured perf logs |
| Structured conversation | Pipecat Flows | Summarizer state machine (section → final) |
| Realtime S2S | OpenAI Realtime / Gemini Live | Phase 4 only, optional |

## Known Risks

| Component | Risk | Note |
|---|---|---|
| `manager.rs` 2235-line refactor | HIGH | Touches recording lifecycle, easy to regress. Need golden tests. |
| RNNoise denoise quality | MEDIUM | Could clip soft speech. Must be toggleable + tested A/B. |
| Cloud STT keychain wiring | MEDIUM | Tauri secure storage cross-platform varies. |
| Streaming STT via WebSocket | MEDIUM | Provider quirks (Deepgram interim_results). |
| Frame/Processor abstraction | MEDIUM | Over-engineering if only 2 providers. Defer until phase 3. |
| Tracing overhead | LOW | `tracing` is mature, low cost. |
| OpenTelemetry exporter in shipped app | LOW | Only enable in dev/debug builds. |

