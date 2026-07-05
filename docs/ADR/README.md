# Architecture Decision Records

Durable decisions that shape Meetdy's architecture, integration patterns, and technology choices.

## Current ADRs

- [ADR-001: Engine Choice — Whisper vs Parakeet](./001-engine-choice.md)
- [ADR-002: Tauri 2.x Migration](./002-tauri-v2-migration.md)
- [ADR-003: SQLite for Transcription History](./003-sqlite-history.md)
- [ADR-004: LLM Post-Processing Integration](./004-post-processing-llm.md)
- [ADR-005: VAD Pipeline Design](./005-vad-pipeline.md)

Existing Harness infrastructure decisions live in `docs/decisions/`.

## When to create an ADR

All three conditions must be true:
1. **Hard to reverse** — the cost of changing your mind later is meaningful
2. **Surprising without context** — a future reader will wonder "why did they do it this way?"
3. **The result of a real trade-off** — there were genuine alternatives and you picked one for specific reasons

## Numbering

Sequential: `0001-slug.md`, `0002-slug.md`, etc.
