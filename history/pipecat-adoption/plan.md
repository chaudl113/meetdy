# Plan: Adopt Pipecat Patterns in meetdy

Source ideas: <https://github.com/pipecat-ai/pipecat>
Goal: bring Pipecat's proven real-time voice patterns into meetdy without sacrificing local-first or stability.

## Phase 1 — Quick Wins (low risk, high impact)

### T1.1 — Adopt `tracing` per-stage in recording path
- **What**: Replace `log_performance_metric` ad-hoc calls in `managers/meeting/manager.rs` recording loop with `tracing::info_span!` for stages: `capture`, `vad`, `stt`, `store`. Add a JSON file subscriber (dev builds) writing to `{app_data}/traces/meetdy.jsonl`.
- **Files**: `Cargo.toml` (add `tracing`, `tracing-subscriber`), `lib.rs` (init), `managers/meeting/manager.rs`, `managers/transcription.rs`, `managers/meeting_logger.rs` (deprecate but keep shim).
- **Acceptance**:
  - [ ] One span per utterance covering full pipeline; child spans for each stage.
  - [ ] Dev build writes JSONL traces; release build is no-op (env filter).
  - [ ] Existing `log::*` calls untouched outside the recording path.
- **Risk**: LOW.

### T1.2 — VAD tuning + settings expose
- **What**: Surface `SmoothedVad` knobs (`onset_frames`, `hangover_frames`, `prefill_frames`, Silero `threshold`) in `settings.rs`. Sensible defaults: onset 3 (90ms), hangover 25 (~750ms), prefill 10 (300ms), threshold 0.5.
- **Files**: `settings.rs`, `audio_toolkit/vad/smoothed.rs` (no logic change, only construction), `audio_toolkit/vad/silero.rs`, settings UI hooks in `src/` (minimal: a "VAD sensitivity" preset Low/Med/High mapping to the four params).
- **Acceptance**:
  - [ ] Settings struct has `vad_sensitivity: VadSensitivity` enum with 3 presets.
  - [ ] Recording uses presets, not hard-coded numbers.
  - [ ] Default behavior identical to current (preset "Med" == current values).
- **Risk**: LOW.

### T1.3 — Partial transcript event while speaking
- **What**: While user is mid-utterance (VAD `Speech` ongoing), every N frames (~500ms) run a lightweight transcribe over the accumulated buffer and emit `meeting_partial_transcript`. Keep `meeting_live_transcript` as final-per-utterance.
- **Implementation note**: Only enabled for Whisper (cheap), gated by `enable_partial_transcripts: bool` setting (default ON). Skip if a partial transcribe is still running (drop, don't queue).
- **Files**: `managers/meeting/manager.rs` (recording loop), `managers/transcription.rs` (add `transcribe_quick(samples, lang)` that uses smaller beam), `settings.rs`, frontend listener (new event).
- **Acceptance**:
  - [ ] `meeting_partial_transcript` emits ~2 Hz while speaking, stops at utterance end.
  - [ ] CPU overhead < 15% on M1 with `small` model.
  - [ ] Toggle works; OFF → no behavior change vs today.
- **Risk**: MEDIUM (CPU). Verify with T1.1 traces.

### T1.4 — RNNoise audio denoise (opt-in, gated by spike S1)
- **What**: After resample to 16kHz, before VAD, optionally run `nnnoiseless` (`RnNoise::process_frame(20ms)`). Add setting `audio_denoise: bool` (default OFF until S1 confirms).
- **Files**: `Cargo.toml` (`nnnoiseless`), new `audio_toolkit/audio/denoise.rs`, `audio_toolkit/mixed_recorder.rs` (insert stage), `settings.rs`.
- **Acceptance**:
  - [ ] Setting toggle works at runtime (per-meeting).
  - [ ] When OFF, byte-identical samples flow to VAD vs today.
  - [ ] Spike S1 documents WER delta on 3 test clips.
- **Risk**: MEDIUM. Gate on spike.

---

## Phase 2 — STT Service Trait & Cloud Providers

### T2.1 — Introduce `SttService` trait
- **What**: New module `managers/stt/`. Define:
  ```rust
  #[async_trait::async_trait]
  pub trait SttService: Send + Sync {
      async fn transcribe(&self, audio: &[f32], lang: Option<&str>) -> Result<Transcript>;
      fn capabilities(&self) -> SttCapabilities;
      fn id(&self) -> &str;
  }
  pub struct Transcript { pub text: String, pub segments: Vec<Segment>, pub language: Option<String> }
  ```
  Implement `LocalWhisperStt` and `LocalParakeetStt` wrapping current `transcribe-rs` engines. Keep `TranscriptionManager` API surface but route through the trait internally.
- **Files**: `managers/stt/mod.rs`, `managers/stt/local_whisper.rs`, `managers/stt/local_parakeet.rs`, `managers/transcription.rs` (delegate).
- **Acceptance**:
  - [ ] All existing call sites compile unchanged.
  - [ ] `LoadedEngine` enum removed; replaced by `Box<dyn SttService>`.
  - [ ] Golden test: same audio in → same transcript out vs pre-refactor.
- **Risk**: MEDIUM (touches hot path).

### T2.2 — Keychain wiring (spike S2 first)
- **What**: Add `keyring` crate. Helpers `set_api_key(service, value)`, `get_api_key(service)`. Used by Phase 2.3 and Phase 4 LLM providers.
- **Files**: `Cargo.toml`, new `helpers/secrets.rs`, settings UI input fields write-only (never read back to UI).
- **Acceptance**:
  - [ ] macOS Keychain works; Windows Credential Manager + Linux Secret Service stubbed with feature flags.
  - [ ] Keys never persisted to settings JSON.
- **Risk**: MEDIUM.

### T2.3 — Groq Whisper cloud STT provider
- **What**: Implement `GroqWhisperStt: SttService` using `reqwest` POST to `https://api.groq.com/openai/v1/audio/transcriptions` with `whisper-large-v3-turbo` model. Reads key via T2.2.
- **Files**: `managers/stt/groq.rs`, `managers/transcription.rs` (selector by id), model registry entry as `GroqWhisper` engine type (display only, no download).
- **Acceptance**:
  - [ ] Selecting "Groq Whisper" in settings routes utterances to cloud.
  - [ ] Fallback to local on network error (configurable).
  - [ ] Privacy banner shown when cloud STT active.
- **Risk**: LOW.

### T2.4 — Provider selector UX
- **What**: Settings UI groups STT engines into "Local" and "Cloud (opt-in)". Cloud engines require API key + explicit consent toggle.
- **Files**: `src/` settings panel, `commands/` getter for engine list.
- **Acceptance**:
  - [ ] Cloud engines disabled until key + consent.
  - [ ] Privacy state surfaced in tray/menu.
- **Risk**: LOW.

---

## Phase 3 — Manager Split & Optional Pipeline Abstraction

### T3.1 — Carve `meeting/manager.rs` into modules
- **What**: Split 2235-line file into:
  - `meeting/orchestrator.rs` — lifecycle (start/stop/pause/resume), public API.
  - `meeting/recording.rs` — audio worker thread, VAD loop, utterance dispatch.
  - `meeting/store.rs` — DB writes, WAV finalization, file moves.
  - `meeting/events.rs` — emit helpers, typed payload structs.
- **Files**: `managers/meeting/*` reorg.
- **Acceptance**:
  - [ ] No file > 500 lines.
  - [ ] Public API of `MeetingSessionManager` unchanged (commands compile).
  - [ ] Golden test passes (start → 30s record → stop, transcript identical).
- **Risk**: HIGH. Land behind small PRs.

### T3.2 — Minimal `Frame`/`Processor` abstraction (only if T3.1 reveals duplication)
- **What**: Introduce:
  ```rust
  pub enum Frame { Audio(Arc<[f32]>), SpeechStart, SpeechEnd, Partial(String), Final(Transcript), Control(Ctrl) }
  #[async_trait] pub trait Processor { async fn on_frame(&mut self, f: Frame, tx: &Sender<Frame>) -> Result<()>; }
  ```
  Wire as `mpsc` chain. Replace recording loop's hand-rolled state machine.
- **Files**: New `audio_toolkit/pipeline/`, `meeting/recording.rs`.
- **Acceptance**:
  - [ ] 1 processor per stage; behavior identical.
  - [ ] Adding a new stage = 1 file, no edits to others.
- **Risk**: MEDIUM. Defer if not justified after T3.1.

---

## Phase 4 — LLM Trait & Structured Summaries

### T4.1 — `LlmService` trait
- **What**: Unify `llm_client.rs` and `ollama.rs` behind trait:
  ```rust
  #[async_trait] pub trait LlmService: Send + Sync {
      async fn complete(&self, system: &str, user: &str) -> Result<String>;
      async fn complete_structured<T: DeserializeOwned>(&self, system: &str, user: &str, schema: &str) -> Result<T>;
  }
  ```
  Adapters: `OllamaLlm`, `OpenAiLlm`, `GroqLlm`, `AnthropicLlm` (subset based on existing `llm_client.rs` providers).
- **Files**: `llm/mod.rs`, move/rewrite `llm_client.rs` + `ollama.rs`, `commands/meeting.rs` (call via trait).
- **Acceptance**:
  - [ ] Existing summary flow works through trait.
  - [ ] Adding a provider = one file.
- **Risk**: MEDIUM.

### T4.2 — Sectional summaries (Pipecat Flows pattern)
- **What**: Every 5 minutes of recording, run a section summarizer producing JSON `{topics, decisions, action_items[]}`. Append to a structured note attached to the session. At stop, run final summary referencing sections.
- **Files**: `managers/meeting/summarizer.rs` (new), `commands/meeting.rs`, DB migration to store section JSON.
- **Acceptance**:
  - [ ] Sections visible in UI as collapsible blocks.
  - [ ] Final summary includes action items list.
  - [ ] Toggle in settings: `structured_summary: bool`.
- **Risk**: MEDIUM.

### T4.3 — Custom prompt templates per meeting type
- **What**: Reuse existing templates feature; ensure trait-based LLM honors per-template system prompt.
- **Files**: `settings.rs` (already has templates), `managers/meeting/summarizer.rs`.
- **Acceptance**:
  - [ ] Switching template changes both prompts and structure.
- **Risk**: LOW.

---

## Phase 5 — Optional Advanced (gated, only if Phases 1–4 land cleanly)

### T5.1 — Streaming STT via Deepgram (spike S3 first)
- **What**: Implement `DeepgramStreamingStt: SttService` that opens a WebSocket and emits `Partial`/`Final` frames in near real time (~300ms latency). Replaces utterance-batched flow for cloud users.
- **Files**: `managers/stt/deepgram.rs`, `pipeline` integration (T3.2 required).
- **Risk**: MEDIUM.

### T5.2 — Realtime voice assistant (OpenAI Realtime API)
- **What**: Hotkey "Ask AI about the meeting" → opens ephemeral Realtime session with transcript-so-far as context; user speaks question, AI speaks answer.
- **Files**: New `managers/assistant/`, frontend mic capture for assistant turn.
- **Risk**: HIGH. Behind feature flag.

### T5.3 — Diarization
- **What**: Plug `sherpa-onnx` speaker embedding + clustering after STT to assign speaker labels.
- **Files**: New `managers/stt/diarize.rs`.
- **Risk**: HIGH.

### T5.4 — OpenTelemetry exporter (dev builds)
- **What**: Behind `--features otel`, ship `tracing-opentelemetry` with OTLP export to local Jaeger.
- **Files**: `Cargo.toml`, `lib.rs` init.
- **Risk**: LOW.

---

## Out of Scope

- Replacing the frontend with Pipecat Voice UI Kit.
- Multi-agent (Pipecat Subagents).
- ESP32 / mobile SDKs.
- Embedding any Python in the app.

