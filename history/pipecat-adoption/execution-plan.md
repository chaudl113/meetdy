# Execution Plan: Adopt Pipecat Patterns in meetdy

Epic: `pipecat-adoption`
Mode: solo or small team. Tracks are designed so 2 contributors can parallelize after T1.1 lands.

## Sequencing Overview

```
              ┌─ T1.2 VAD settings ─┐
T1.1 tracing ─┤                     ├─> T1.3 partial transcripts ─┐
              └─ S1 spike RNNoise ──┴─> T1.4 denoise (gated) ─────┤
                                                                  │
S2 spike keychain ─> T2.2 keychain ─┐                              │
                                    ├─> T2.3 Groq ─> T2.4 UX       │
T2.1 SttService trait ──────────────┘                              │
                                                                  │
                                    T3.1 manager split ───────────┤
                                                                  │
T4.1 LlmService trait ─> T4.2 sectional summaries ─> T4.3 templates
                                                                  │
                                    (Phase 5 optional, post-MVP) ─┘
```

## Tracks

| Track | Agent       | Beads (in order)                      | File Scope (no overlap)                                    |
| ----- | ----------- | ------------------------------------- | ---------------------------------------------------------- |
| 1     | BlueRiver   | S4 → T1.1 → T1.2 → T1.3               | `src-tauri/src/managers/meeting/**`, `audio_toolkit/vad/**`, `settings.rs` (additive) |
| 2     | GreenForest | S1 → T1.4                             | `src-tauri/src/audio_toolkit/audio/**`                     |
| 3     | RedStone    | S2 → T2.1 → T2.2 → T2.3 → T2.4        | `src-tauri/src/managers/stt/**`, `helpers/secrets.rs`, `managers/transcription.rs` (delegate-only edit) |
| 4     | PurpleHill  | T3.1                                  | `src-tauri/src/managers/meeting/**` (after T1.3 lands)     |
| 5     | OrangeMoon  | T4.1 → T4.2 → T4.3                    | `src-tauri/src/llm/**`, `commands/meeting.rs` (summary fn) |
| 6     | YellowSky   | S3 → T5.1 → T5.2 → T5.3 → T5.4        | Phase 5 only, after Phases 1–4 stable                      |

### Conflict policy

- Tracks 1 and 4 both touch `meeting/**`. **Track 4 starts only after Track 1 finishes T1.3.**
- Track 3's edits to `managers/transcription.rs` are limited to swapping engine construction to trait; coordinate with Track 1 if both touch the same week.
- Track 2 is fully independent.

## Track Details

### Track 1: BlueRiver — Observability + VAD + Partial Transcripts

**Scope**: `managers/meeting/**`, `audio_toolkit/vad/**`, additive `settings.rs`.

**Beads**:
1. `S4-tracing` — Spike: confirm `tracing` + JSON file subscriber. Output: working snippet in `.spikes/pipecat-adoption/tracing/`.
2. `T1.1` — Adopt `tracing` per-stage in recording path.
3. `T1.2` — Expose VAD presets (Low/Med/High) in settings.
4. `T1.3` — Emit `meeting_partial_transcript` while speaking.

**Exit criteria**: Traces visible per-utterance, VAD presets switchable, partial events visible in dev tools, default UX unchanged.

### Track 2: GreenForest — Audio Denoise (gated)

**Scope**: `audio_toolkit/audio/**`.

**Beads**:
1. `S1-rnnoise` — Spike: A/B WER with `nnnoiseless` on 3 clips (quiet/noisy/bluetooth).
2. `T1.4` — Implement denoise stage, toggle in settings.

**Gate**: Only proceed to T1.4 if S1 shows ≥10% WER improvement on noisy clip and no regression on quiet clip.

### Track 3: RedStone — STT Trait + Cloud Provider

**Scope**: `managers/stt/**`, `helpers/secrets.rs`, delegate-only edits to `managers/transcription.rs`.

**Beads**:
1. `S2-keychain` — Spike: `keyring` crate store+read on macOS.
2. `T2.1` — `SttService` trait + local Whisper/Parakeet adapters.
3. `T2.2` — Keychain helpers.
4. `T2.3` — Groq Whisper provider.
5. `T2.4` — Settings UI for provider selection + consent.

**Exit criteria**: User can switch between Local Whisper, Local Parakeet, and Groq Cloud at runtime. Keys never appear in settings file.

### Track 4: PurpleHill — Meeting Manager Split

**Scope**: `managers/meeting/**` (after Track 1 T1.3 done).

**Beads**:
1. `T3.1` — Split `manager.rs` into `orchestrator.rs` / `recording.rs` / `store.rs` / `events.rs`. Optionally `T3.2` if duplication is obvious.

**Exit criteria**: No file > 500 lines; behavior preserved (golden tests).

### Track 5: OrangeMoon — LLM Trait + Structured Summaries

**Scope**: `llm/**`, `commands/meeting.rs` (summary path only).

**Beads**:
1. `T4.1` — `LlmService` trait + adapters (Ollama, OpenAI, Groq, Anthropic).
2. `T4.2` — Sectional summarizer (5-min sections + final).
3. `T4.3` — Per-template system prompts wired through trait.

**Exit criteria**: Section blocks render in UI; action items extracted; toggle works.

### Track 6: YellowSky — Phase 5 Optional

Run only after Phases 1–4 stable in main. Each bead independently shippable behind feature flags.

## Cross-Track Dependencies

- Track 4 waits for Track 1 (T1.3).
- Track 5 (T4.2 structured summary) benefits from Track 3 trait pattern; not blocking but should land after T2.1 for consistency.
- Track 6 (T5.1 streaming STT) requires Track 3 (T2.1 trait) + Track 4 (T3.1 split) + ideally T3.2 (pipeline).

## Key Learnings Slots (filled after spikes)

- `S1-rnnoise`: TBD (WER numbers, latency impact).
- `S2-keychain`: TBD (crate version, macOS service name format).
- `S3-deepgram-ws`: TBD (interim handling, reconnect strategy).
- `S4-tracing`: TBD (subscriber config, env filter syntax).

## Validation Checklist Before Each PR Merge

- [ ] `cargo check` clean.
- [ ] `cargo test` passes (golden recording test for Tracks 1/4).
- [ ] Manual smoke: start → 30s record → stop → transcript + summary.
- [ ] No new warnings on touched files.
- [ ] Setting defaults preserve current UX.

## Ship Order Summary

1. **Ship 1 (Phase 1)**: T1.1 + T1.2 + T1.3 (+ T1.4 if S1 passes).
2. **Ship 2 (Phase 2)**: T2.1 → T2.2 → T2.3 → T2.4.
3. **Ship 3 (Phase 3)**: T3.1 (+ T3.2 if justified).
4. **Ship 4 (Phase 4)**: T4.1 → T4.2 → T4.3.
5. **Ship 5+ (Phase 5)**: case-by-case behind feature flags.

