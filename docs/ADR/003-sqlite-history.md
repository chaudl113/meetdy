# ADR-003: SQLite for Transcription History

**Status:** Accepted
**Date:** 2025-07-01
**Decision Maker:** Development Team

## Context
Meetdy needs persistent local storage for transcription history, meeting sessions, and user notes. The data must remain on-device (privacy-first), survive app restarts, and support queries like "show me last week's transcriptions."

## Decision
Use rusqlite with bundled SQLite, running in WAL mode. The History Manager owns the connection and exposes typed queries via a repository pattern.

## Alternatives Considered
- **JSON/plaintext files:** No query capability, poor performance with large histories, no transactional safety.
- **sled (embedded Rust DB):** Key-value only; no relational queries for filtering by date, session, or content search.
- **Postgres/SQLite via Tauri sidecar:** Overkill for a desktop app; adds binary size and startup complexity.
- **Cloud sync (Firebase/Supabase):** Violates the privacy-first requirement. Users expect transcription data to stay local.

## Consequences
- Zero external dependencies — SQLite is bundled in the binary.
- WAL mode enables concurrent reads during writes, important for live transcription logging.
- Schema migrations must be managed (we'll use a simple versioned migration system in the History Manager).
- No cloud sync by design — if users want backups, they export manually.
- Connection management is the History Manager's responsibility; no connection pooling needed for single-user desktop use.
