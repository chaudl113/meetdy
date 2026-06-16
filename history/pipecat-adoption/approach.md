# Approach: Adopt Pipecat Patterns in meetdy

## Gap Analysis

| Component | Have | Need | Gap |
|---|---|---|---|
| Audio capture | `MixedAudioRecorder` | Same | None |
| VAD | `SileroVad` + `SmoothedVad` | Tunable params, expose in settings | Settings + UI |
| Audio denoise | None (only text-level regex) | Optional RNNoise pre-VAD | New module + setting |
| STT engine selection | `enum LoadedEngine { Whisper, Parakeet }` | Trait + plug-in providers | Refactor to trait, keep enum-free |
| Cloud STT | None | Groq / Deepgram opt-in | New providers behind trait |
| Partial transcripts | Only utterance-final via `meeting_live_transcript` | True interim while speaking | New event `meeting_partial_transcript` |
| LLM provider | Cloud (`llm_client`) + Ollama, conditional `if` | `LlmService` trait | Refactor for symmetry, low risk |
| Pipeline abstraction | Monolithic `manager.rs` (2235L) | Optional Frame/Processor chain | Phase 3, only if ≥3 providers |
| Manager size | `manager.rs` 2235L | Split into orchestrator/pipeline/store | Tech-debt fix, phase 3 |
| Tracing | `log::*` + ad-hoc `log_performance_metric` | `tracing` spans per stage | Adopt `tracing` crate |
| Realtime S2S | None | OpenAI Realtime / Gemini Live | Optional, phase 5 |
| Diarization | None | Speaker labels | Optional, phase 5 |

## Recommended Approach

**Incremental, value-first.** Start with high-impact/low-risk wins, defer the Frame/Processor rewrite until justified.

### Approach: 5-Phase, value-decreasing order

1. **Phase 1 – Quick wins**: VAD tuning, partial transcripts, `tracing` adoption, RNNoise denoise.
2. **Phase 2 – STT service trait**: Wrap existing local engines in trait; add Groq Whisper provider; keychain.
3. **Phase 3 – Manager split**: Carve `manager.rs` into `orchestrator/pipeline/store`. Optionally introduce minimal `Frame`/`Processor` if it removes boilerplate.
4. **Phase 4 – LLM trait + structured summary**: Unify LLM clients; emit structured section summaries (Flows pattern).
5. **Phase 5 – Optional advanced**: Streaming STT (Deepgram), realtime S2S agent, diarization, OTel exporter (dev only).

Ship after each phase; each phase leaves the app in a fully working state.

### Alternative Approaches

1. **Big-bang pipeline rewrite (rejected).** Port Pipecat's `FrameProcessor` model wholesale to Rust upfront. Tradeoff: clean architecture but huge regression risk and weeks of no user-visible value. Not justified at current provider count (1 STT family, 2 LLM backends).
2. **Cloud-first STT (rejected).** Make Groq/Deepgram default. Tradeoff: fastest UX but breaks "local-first" positioning. Cloud must remain opt-in.
3. **Frontend SDK approach (rejected).** Use Pipecat client SDK directly; meetdy becomes a thin shell. Tradeoff: replaces the product. Out of scope.

## Risk Map

| Component | Risk | Reason | Verification |
|---|---|---|---|
| RNNoise integration | MEDIUM | New DSP, may affect Whisper accuracy on quiet speakers | Spike: A/B WER on 3 test clips |
| `manager.rs` split | HIGH | 2235L, touches lifecycle + DB + events | Golden test (start→record→pause→stop), keep behavior identical |
| Groq Whisper provider | LOW | HTTP-only, well-documented | Type-check + 1 integration test with mock |
| Keychain for API keys | MEDIUM | Tauri keyring crate quirks per OS | Spike: store+read on macOS first; Windows/Linux follow-on |
| Partial transcript event | LOW | Pure additive emit | Frontend stays compat (new event name) |
| `tracing` adoption | LOW | Drop-in replacement for `log::*` paths we touch | Compile + run |
| Streaming STT (Deepgram WS) | MEDIUM | Network resilience, interim handling | Spike: 5-min stable session |
| Frame/Processor abstraction | MEDIUM | Easy to over-engineer | Only adopt if Phase 3 split reveals duplicated wiring |
| OTel exporter | LOW | Dev-only build flag | feature flag gate |

## Decisions Locked In

- **Local-first stays default.** Cloud STT/LLM is opt-in, key in OS keychain.
- **Backward-compat event names.** Keep `meeting_live_transcript`; add `meeting_partial_transcript` as new event.
- **No Python.** All work stays in Rust crates; no embedding pipecat itself.
- **Tracing crate** (`tracing` + `tracing-subscriber`) replaces ad-hoc perf logs in touched paths only. No global rewrite.
- **Defer Frame/Processor** until ≥3 STT providers or pipeline complexity demands it.

## Spike Plan

Run these before committing to the related phase work.

| Spike | Question | Time-box | Output |
|---|---|---|---|
| `S1-rnnoise` | Does `nnnoiseless` improve Whisper WER on a noisy clip without harming a quiet clip? | 2h | A/B WER numbers in `.spikes/pipecat-adoption/rnnoise/` |
| `S2-keychain` | Can we store+read a Groq API key via `keyring` crate on macOS from Tauri? | 1h | Working snippet |
| `S3-deepgram-ws` | Does a 5-min Deepgram streaming session stay stable with interim events? | 2h | Trace + sample transcript |
| `S4-tracing` | Confirm `tracing` + JSON subscriber emits per-stage spans we can read in a file. | 30m | Working config |

S1 and S2 are mandatory before Phase 1.1 and Phase 2 respectively. S3 only if Phase 5 streaming is approved.

