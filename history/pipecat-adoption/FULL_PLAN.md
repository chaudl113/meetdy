# Full Plan: Bring Pipecat-Style Voice Pipeline Into meetdy

Source reference: https://github.com/pipecat-ai/pipecat

## 1. Objective

Make meetdy's meeting recorder/transcriber feel more real-time, more extensible, and easier to evolve, while keeping the current local-first desktop app identity.

The goal is **not** to port Pipecat into meetdy. Pipecat is Python/server-first and built for multimodal agents. meetdy is Rust/Tauri/local-first. We should copy the useful ideas:

- Frame/pipeline thinking.
- Tunable VAD and utterance boundaries.
- Pluggable STT/LLM providers.
- Optional cloud providers with explicit consent.
- Audio preprocessing before STT.
- Structured tracing and debuggability.
- Structured summaries and action items.

## 2. Current State

Relevant code today:

| Area | File | Notes |
|---|---|---|
| Meeting lifecycle | `src-tauri/src/managers/meeting/manager.rs` | 2235 lines; owns recording, VAD, STT dispatch, DB updates, events. Main tech debt. |
| STT | `src-tauri/src/managers/transcription.rs` | Loads local `transcribe-rs` engines: Whisper + Parakeet. |
| Model registry | `src-tauri/src/managers/model.rs` | Local model download/listing. |
| Mixed recording | `src-tauri/src/audio_toolkit/mixed_recorder.rs` | Combines mic + system audio; emits audio stats. |
| VAD | `src-tauri/src/audio_toolkit/vad/*` | Already has Silero + smoothed VAD. Good foundation. |
| Summary LLM | `src-tauri/src/llm_client.rs`, `src-tauri/src/ollama.rs` | Cloud and local summary paths exist but are not unified behind a trait. |
| Meeting commands | `src-tauri/src/commands/meeting.rs` | Summary generation and command glue. |

Important finding: **meetdy already has Silero VAD**, so the first job is not adding VAD. The real job is tuning/exposing it and using it as a clean boundary in a better pipeline.

## 3. Target Architecture

Short-term target, without risky rewrite:

```text
Mic/System Audio
  -> MixedAudioRecorder
  -> optional Denoise
  -> SmoothedVad(SileroVad)
  -> UtteranceBuffer
  -> SttService(local Whisper / Parakeet / cloud Groq)
  -> TranscriptNormalizer
  -> MeetingStore
  -> Tauri Events
```

Medium-term target, after the manager split:

```text
MeetingSessionManager
  -> MeetingOrchestrator     // public lifecycle API
  -> MeetingRecordingWorker  // audio thread + VAD + utterances
  -> MeetingSttPipeline      // STT provider + partial/final transcript events
  -> MeetingStore            // SQLite, WAV, transcript files
  -> MeetingEvents           // typed event payloads
  -> MeetingSummarizer       // section summaries + final summary
```

Long-term optional target, only if complexity grows:

```rust
enum Frame {
    Audio(AudioChunk),
    SpeechStart,
    SpeechEnd,
    TranscriptPartial(Transcript),
    TranscriptFinal(Transcript),
    SummarySection(SummarySection),
    Control(ControlEvent),
}

trait Processor {
    async fn process(&mut self, frame: Frame, out: Sender<Frame>) -> Result<()>;
}
```

Do **not** start with this full frame system. It is useful later, but it would be over-engineering today.

## 4. Product Principles

- Local-first remains default.
- Cloud STT/LLM must be opt-in.
- API keys must not be stored in plain settings files.
- Existing event names must remain compatible.
- Each milestone must be shippable independently.
- Prefer small refactors over a big-bang rewrite.
- Add observability before changing hot-path behavior.

## 5. Milestone Plan

## Milestone 0: Baseline And Safety Net

Purpose: create a reliable baseline before touching the audio/STT path.

### Tasks

1. Add a reproducible smoke script/checklist for meeting recording.
2. Capture current timing numbers for: VAD boundary, STT duration, end-to-event latency.
3. Add at least one tiny fixture-based transcription test if current test infra allows.
4. Document current event payloads: `meeting_live_transcript`, `meeting_audio_stats`, `meeting_completed`, `meeting_failed`.

### Acceptance Criteria

- `cargo check` passes.
- Manual baseline recorded: start meeting, speak 30 seconds, stop, transcript saved.
- Current default model behavior documented.
- No product behavior changed.

### Risk

Low.

## Milestone 1: Observability First

Purpose: make latency visible before adding denoise, partials, or cloud providers.

### Tasks

1. Add `tracing` and `tracing-subscriber`.
2. Initialize tracing in `src-tauri/src/lib.rs`.
3. Add spans around:
   - audio chunk receive
   - VAD decision
   - utterance finalize
   - STT call
   - transcript normalization
   - event emit
   - DB/file save
4. Keep existing `log::*` output working.
5. In dev builds, write JSONL traces to app data or stdout with an env filter.

### Files

- `src-tauri/Cargo.toml`
- `src-tauri/src/lib.rs`
- `src-tauri/src/managers/meeting/manager.rs`
- `src-tauri/src/managers/transcription.rs`
- `src-tauri/src/managers/meeting_logger.rs`

### Acceptance Criteria

- One utterance produces a trace with STT duration and total latency.
- Release build is not noisy by default.
- `cargo check` passes.
- No user-visible behavior change.

### Risk

Low.

## Milestone 2: VAD Presets And Better Utterance Control

Purpose: improve speech boundary quality using the VAD code already in the repo.

### Tasks

1. Add `VadSensitivity` setting with presets:
   - `Conservative`: fewer false starts, longer silence before end.
   - `Balanced`: current behavior/default.
   - `Responsive`: faster start/end, better realtime feel.
2. Map presets to:
   - Silero threshold
   - onset frames
   - hangover frames
   - prefill frames
3. Use settings when constructing `SmoothedVad`.
4. Add UI selector in audio/meeting settings.

### Suggested Defaults

| Preset | Threshold | Onset | Hangover | Prefill | Notes |
|---|---:|---:|---:|---:|---|
| Conservative | 0.60 | 4 | 35 | 12 | Noisy rooms. |
| Balanced | 0.50 | current/default | current/default | current/default | Preserve behavior. |
| Responsive | 0.42 | 2 | 18 | 8 | Faster live captions. |

Actual current values should be read from the constructor call and preserved for `Balanced`.

### Files

- `src-tauri/src/settings.rs`
- `src-tauri/src/audio_toolkit/vad/silero.rs`
- `src-tauri/src/audio_toolkit/vad/smoothed.rs`
- `src-tauri/src/managers/meeting/manager.rs`
- Frontend settings components under `src/`

### Acceptance Criteria

- Default `Balanced` matches current behavior.
- Changing preset affects new recordings.
- Existing meetings/transcripts are unaffected.
- `cargo check` and frontend build pass.

### Risk

Low.

## Milestone 3: Partial Transcript Events

Purpose: make the UI feel realtime without replacing the current final transcript flow.

### Current Behavior

`meeting_live_transcript` exists but behaves like utterance-final text. It is not true mid-speech partial transcription.

### Target Behavior

- Add new event: `meeting_partial_transcript`.
- Keep `meeting_live_transcript` as the final committed text event.
- Partial text is temporary and can be replaced.
- Final text is appended to the meeting transcript.

### Tasks

1. Add setting `enable_partial_transcripts`, default `true` if performance is acceptable, otherwise `false`.
2. While VAD is inside speech, every ~500ms emit partial text.
3. Never queue multiple partial STT jobs. If one is running, skip the next partial tick.
4. Final utterance STT remains authoritative.
5. Frontend renders partial text separately from committed transcript.

### Implementation Options

| Option | Pros | Cons | Recommendation |
|---|---|---|---|
| Local Whisper partial over accumulated buffer | Simple, uses current stack | Can be CPU-heavy | Start here, gated by tracing. |
| Cloud streaming provider | Best UX | Requires cloud + WebSocket | Later milestone. |
| Fake partial from audio/VAD only | Cheap | Not useful text | Do not do. |

### Files

- `src-tauri/src/managers/meeting/manager.rs`
- `src-tauri/src/managers/transcription.rs`
- `src-tauri/src/settings.rs`
- Frontend meeting transcript store/components

### Acceptance Criteria

- Partial event appears while speaking.
- Final event still appears at utterance end.
- Turning partials off restores current behavior.
- CPU overhead measured with Milestone 1 traces.
- No duplicate final transcript lines.

### Risk

Medium due to CPU overhead.

## Milestone 4: Optional Audio Denoise

Purpose: improve STT accuracy in noisy rooms before audio reaches VAD/STT.

### Spike First

Before implementation, run a spike with `nnnoiseless` or another RNNoise-compatible crate.

Test clips:

- Quiet office voice.
- Fan/AC noise.
- Bluetooth/headset compression.
- Keyboard typing background.

Measure:

- WER or manual error count.
- STT latency delta.
- VAD false positive/negative changes.

### Tasks

1. Add `audio_denoise_enabled` setting, default off unless spike is clearly positive.
2. Add `audio_toolkit/audio/denoise.rs`.
3. Insert denoise after resampling and before VAD.
4. Ensure denoise can be bypassed without altering samples.
5. Add tracing metric for denoise duration.

### Files

- `src-tauri/Cargo.toml`
- `src-tauri/src/audio_toolkit/audio/denoise.rs`
- `src-tauri/src/audio_toolkit/mixed_recorder.rs`
- `src-tauri/src/settings.rs`

### Acceptance Criteria

- OFF path preserves current behavior.
- ON path does not clip speech.
- Latency overhead is acceptable.
- Spike results documented in `history/pipecat-adoption/`.

### Risk

Medium.

## Milestone 5: STT Provider Trait

Purpose: make local and cloud STT providers pluggable.

### Target API

```rust
pub struct Transcript {
    pub text: String,
    pub language: Option<String>,
    pub segments: Vec<TranscriptSegment>,
}

pub struct TranscriptSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub text: String,
    pub speaker: Option<String>,
}

pub struct SttCapabilities {
    pub streaming: bool,
    pub local: bool,
    pub diarization: bool,
}

#[async_trait::async_trait]
pub trait SttService: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn capabilities(&self) -> SttCapabilities;
    async fn transcribe(&self, audio: &[f32], language: Option<&str>) -> anyhow::Result<Transcript>;
}
```

### Tasks

1. Create `src-tauri/src/managers/stt/`.
2. Implement `LocalWhisperStt` wrapping current Whisper engine path.
3. Implement `LocalParakeetStt` wrapping current Parakeet path.
4. Update `TranscriptionManager` to hold `Box<dyn SttService>` instead of `LoadedEngine` enum.
5. Preserve public `TranscriptionManager::transcribe()` API for call sites.
6. Add capability metadata to settings/model UI.

### Files

- `src-tauri/src/managers/stt/mod.rs`
- `src-tauri/src/managers/stt/local_whisper.rs`
- `src-tauri/src/managers/stt/local_parakeet.rs`
- `src-tauri/src/managers/transcription.rs`
- `src-tauri/src/managers/model.rs`

### Acceptance Criteria

- Local Whisper works exactly as before.
- Local Parakeet works exactly as before.
- Adding a new STT provider no longer requires changing a central enum.
- `cargo check` passes.

### Risk

Medium.

## Milestone 6: Secure Secrets And Groq Whisper Provider

Purpose: add fast cloud STT without compromising privacy.

### Tasks

1. Add keychain helper using the `keyring` crate.
2. Add commands:
   - `set_secret(service, value)`
   - `delete_secret(service)`
   - `has_secret(service) -> bool`
3. Never return raw API keys to frontend.
4. Implement `GroqWhisperStt` using OpenAI-compatible audio transcription endpoint.
5. Add provider selector:
   - Local Whisper
   - Local Parakeet
   - Groq Whisper Cloud
6. Add explicit privacy consent toggle for cloud STT.
7. Add fallback behavior option:
   - fail loudly
   - fallback to local

### Files

- `src-tauri/src/helpers/secrets.rs`
- `src-tauri/src/managers/stt/groq.rs`
- `src-tauri/src/managers/transcription.rs`
- `src-tauri/src/settings.rs`
- `src-tauri/src/commands/*`
- Frontend settings UI

### Acceptance Criteria

- API key is stored in OS keychain, not settings JSON.
- Cloud STT cannot run without explicit consent.
- Network errors are surfaced clearly.
- Local mode remains default.

### Risk

Medium.

## Milestone 7: Split Meeting Manager

Purpose: reduce tech debt and prepare for future pipeline work.

### Current Problem

`manager.rs` is 2235 lines and combines multiple responsibilities. This makes every new feature risky.

### Target Modules

| New Module | Responsibility |
|---|---|
| `manager.rs` | Public `MeetingSessionManager` facade only. |
| `orchestrator.rs` | start/stop/pause/resume lifecycle. |
| `recording.rs` | audio worker thread, VAD loop, utterance buffers. |
| `events.rs` | typed Tauri event payloads and emit helpers. |
| `store.rs` | DB writes, WAV handling, transcript persistence. |
| `summarizer.rs` | later: section/final summaries. |

### Tasks

1. Move event payload structs and emit helpers first.
2. Move DB/file-store methods second.
3. Move recording worker third.
4. Keep `MeetingSessionManager` public methods unchanged.
5. Add regression tests around lifecycle.

### Acceptance Criteria

- No meeting module file over 500 lines if practical.
- Existing Tauri commands compile unchanged.
- Meeting start/pause/resume/stop works.
- Transcript and WAV output paths unchanged.

### Risk

High. Do only after Milestones 1-6 stabilize.

## Milestone 8: LLM Provider Trait And Structured Summaries

Purpose: apply the same provider pattern to summary generation and make notes more useful.

### Target API

```rust
#[async_trait::async_trait]
pub trait LlmService: Send + Sync {
    fn id(&self) -> &'static str;
    async fn complete(&self, prompt: LlmPrompt) -> anyhow::Result<String>;
    async fn complete_json<T: serde::de::DeserializeOwned>(&self, prompt: LlmPrompt) -> anyhow::Result<T>;
}
```

### Tasks

1. Wrap current `llm_client.rs` cloud flow in `CloudLlmService`.
2. Wrap `ollama.rs` in `OllamaLlmService`.
3. Add structured summary schema:
   ```json
   {
     "overview": "string",
     "topics": ["string"],
     "decisions": ["string"],
     "action_items": [
       { "task": "string", "owner": "string|null", "due": "string|null" }
     ]
   }
   ```
4. Add section summaries every N minutes.
5. Final summary uses section summaries + final transcript.

### Files

- `src-tauri/src/llm/mod.rs`
- `src-tauri/src/llm_client.rs`
- `src-tauri/src/ollama.rs`
- `src-tauri/src/commands/meeting.rs`
- `src-tauri/src/managers/meeting/summarizer.rs`
- DB migration for structured summary data if needed

### Acceptance Criteria

- Existing summary still works.
- Structured summary can be toggled on/off.
- Action items are extracted into stable JSON.
- Local Ollama path still works.

### Risk

Medium.

## Milestone 9: Optional Streaming STT

Purpose: best realtime UX for users who opt into cloud.

### Candidate Provider

Deepgram or AssemblyAI, because they support WebSocket interim transcripts.

### Tasks

1. Spike 5-minute WebSocket streaming session.
2. Add `StreamingSttService` extension trait if needed.
3. Emit partial/final transcript events directly from provider.
4. Add reconnect and backoff.
5. Keep local utterance-batched path unchanged.

### Acceptance Criteria

- Partial latency below 500ms in stable network.
- Reconnect does not corrupt final transcript.
- User can switch back to local instantly.

### Risk

Medium-high.

## Milestone 10: Optional Advanced Agent Features

Only after the core pipeline is stable.

### Features

- Realtime meeting assistant: ask questions about the current meeting by voice.
- Speaker diarization: label speakers in transcript.
- OpenTelemetry exporter for dev/debug builds.
- Pipeline debugger UI similar in spirit to Pipecat Whisker.

### Recommendation

Do not start here. These are product-expansion items, not foundation work.

## 6. Concrete Ship Order

Recommended order:

1. Milestone 0: baseline and safety net.
2. Milestone 1: tracing.
3. Milestone 2: VAD presets.
4. Milestone 3: partial transcript events.
5. Milestone 5: STT provider trait.
6. Milestone 6: secure secrets + Groq Whisper.
7. Milestone 4: denoise, only if spike passes.
8. Milestone 7: split meeting manager.
9. Milestone 8: LLM trait + structured summaries.
10. Milestones 9-10: optional advanced work.

Reasoning:

- Tracing first makes later performance regressions visible.
- VAD/partials improve UX quickly.
- STT trait unlocks provider flexibility.
- Groq gives a clear performance win while preserving local default.
- Denoise should be evidence-based, not assumed.
- Manager split becomes easier after behavior is traced and tested.

## 7. Test Strategy

### Automated

- `cargo check` for every change.
- Unit tests for VAD preset mapping.
- Unit tests for settings migration/defaults.
- Mock HTTP tests for Groq provider.
- Secret helper tests using a test service name.
- Summary JSON parsing tests.

### Manual Smoke

For each milestone:

1. Start app.
2. Start meeting.
3. Speak 30 seconds.
4. Pause/resume once.
5. Stop meeting.
6. Verify transcript text appears.
7. Verify WAV exists.
8. Generate summary.
9. Restart app and verify meeting history persists.

### Performance

Track these metrics from tracing:

- VAD start latency.
- VAD end latency.
- STT duration per utterance.
- Partial transcript interval.
- Event emit latency.
- Total speech-end to UI latency.
- CPU overhead with partials on/off.

## 8. Rollback Plan

Each risky feature gets a setting flag:

| Feature | Rollback |
|---|---|
| Partial transcripts | Disable `enable_partial_transcripts`. |
| Denoise | Disable `audio_denoise_enabled`. |
| Cloud STT | Switch provider back to local. |
| VAD presets | Default back to `Balanced`. |
| Structured summaries | Disable structured mode; keep old prompt path. |
| Manager split | No runtime flag; must be protected by tests and small PRs. |

## 9. What Not To Do

- Do not import Pipecat directly.
- Do not rewrite the full app around `FrameProcessor` immediately.
- Do not make cloud STT default.
- Do not store API keys in normal settings.
- Do not remove existing transcript events.
- Do not start diarization before the core pipeline is stable.

## 10. Definition Of Done

The full adoption is considered successful when:

- Local Whisper/Parakeet still work as before.
- User can choose VAD sensitivity.
- Partial transcript UX exists and can be disabled.
- Optional cloud STT provider works with secure API key storage.
- Meeting manager is split into maintainable modules.
- Summary generation is provider-based and can output structured notes.
- Traces show where latency is spent.
- Default user experience remains local-first and privacy-preserving.
