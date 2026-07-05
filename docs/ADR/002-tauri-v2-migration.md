# ADR-002: Tauri 2.x Migration

**Status:** Accepted
**Date:** 2025-07-01
**Decision Maker:** Development Team

## Context
The project was initially built on Tauri 1.x. Tauri 2.x offers an improved plugin system, mobile platform support, better IPC performance, and a stronger security model. However, it introduces breaking API changes and requires migrating several dependencies.

## Decision
Full migration to Tauri 2.9.x. All plugins (global-shortcut, clipboard, store, dialog, shell, fs) use the Tauri 2.x plugin architecture.

## Alternatives Considered
- **Stay on Tauri 1.x:** Avoids migration cost but misses plugin architecture improvements and mobile readiness. Tauri 1.x is in maintenance mode.
- **Electron:** Abandoned early — too heavy, no Rust backend, larger binary size.
- **Partial migration (hybrid 1.x/2.x):** Not possible; the plugin APIs are fundamentally different.

## Consequences
- All `tauri::command` and plugin APIs rewritten to Tauri 2.x signatures.
- Mobile support becomes architecturally possible (iOS/Android), though not an immediate priority.
- Some dependencies (specta) are still on release candidates, introducing minor stability risk.
- The global-shortcut plugin in Tauri 2.x handles macOS accessibility permissions differently, requiring updated onboarding flows.
