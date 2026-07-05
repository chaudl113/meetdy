# ADR-005: VAD Pipeline Design

**Status:** Accepted
**Date:** 2025-07-01
**Decision Maker:** Development Team

## Context
The audio pipeline captures a continuous stream from the microphone. Without filtering, silent segments are fed to the model, wasting inference time and producing garbage output. We need Voice Activity Detection that is accurate, fast, and cross-platform.

## Decision
Use Silero VAD via ONNX runtime with a smoothing pipeline: prefill (accumulate initial speech frames), hangover (keep recording after speech ends to avoid cutting off), and onset smoothing (prevent false starts from brief noise). The VAD runs on 30ms audio frames.

## Alternatives Considered
- **Energy-based VAD (RMS threshold):** Too crude — fails in noisy environments, can't distinguish speech from background noise.
- **WebRTC VAD:** Lightweight but less accurate than Silero, especially for non-English speech.
- **Silero VAD via PyTorch:** Accurate but requires Python runtime — unacceptable for a Rust/Tauri app.
- **No VAD (record fixed chunks):** Users must manually trim silence — terrible UX for a push-to-talk workflow.

## Consequences
- ONNX runtime dependency adds ~10MB to the binary (static linking).
- Silero VAD model (~2MB) must be downloaded on first run (bundled in `src-tauri/resources/models/`).
- 30ms frame processing adds ~1-3ms CPU overhead per frame — negligible on modern hardware.
- Smoothing parameters (prefill 300ms, hangover 500ms, onset threshold) are configurable in Settings.
- The VAD pipeline lives in `src-tauri/src/audio_toolkit/vad/` as a standalone module consumed by the Audio Manager.
