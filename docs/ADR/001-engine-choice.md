# ADR-001: Engine Choice — Whisper vs Parakeet

**Status:** Accepted
**Date:** 2025-07-01
**Decision Maker:** Development Team

## Context
Meetdy needs local speech-to-text inference with GPU acceleration for performance on capable hardware, plus a CPU-optimized fallback for low-power or headless devices. Two engines exist: Whisper (via whisper-rs) offers Metal/Vulkan/CUDA acceleration; Parakeet V3 (via transcribe-rs) is CPU-optimized.

## Decision
Support both engines with runtime switching. Whisper is the default on GPU-capable hardware; Parakeet is the fallback for CPU-only or explicitly low-power configurations.

## Alternatives Considered
- **Whisper only:** Locks out CPU-only devices and increases minimum hardware requirements.
- **Parakeet only:** Leaves GPU acceleration on the table — unacceptable for real-time use on modern hardware.
- **Abstract over engine at build time:** Less flexible than runtime switching; users can't change preference without rebuilding.

## Consequences
- Two engine backends to maintain (whisper-rs + transcribe-rs), doubling integration surface.
- Wider hardware compatibility across macOS, Windows, and Linux.
- Runtime switching requires a unified model management and pipeline abstraction in the Model Manager.
- Each engine has its own model format and download path, increasing Model Manager complexity.
