# ADR-004: LLM Post-Processing Integration

**Status:** Accepted
**Date:** 2025-07-01
**Decision Maker:** Development Team

## Context
Users want AI-powered text refinement after transcription — summarization, formatting, action-item extraction, translation. Multiple LLM providers exist with different privacy, cost, and capability trade-offs.

## Decision
Support multiple providers (OpenAI, Anthropic, local Ollama, Apple Intelligence, ChatGPT Plus) through a unified LLM interface. Users configure their provider and API key in Settings. A configurable system prompt template controls the post-processing behavior.

## Alternatives Considered
- **Single provider (OpenAI only):** Simplest but forces users into one ecosystem, requires internet, and has privacy implications.
- **Local-only (Ollama only):** Private but limits users who don't want to run local models. No access to frontier models.
- **No post-processing:** Users must manually refine text in another tool — a poor UX for a productivity app.
- **Prompt marketplace / community prompts:** Out of scope for MVP but architecturally compatible if added later.

## Consequences
- Multiple API clients to maintain (OpenAI SDK, Anthropic SDK, Ollama REST, Apple Intelligence framework).
- Token costs are the user's responsibility; we must show estimated costs before processing.
- Apple Intelligence integration is macOS-only and requires macOS 15+.
- The unified interface must handle streaming vs. non-streaming, different token limits, and error modes.
- Configurable prompts are stored locally alongside other settings.
