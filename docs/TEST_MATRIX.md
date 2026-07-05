# Test Matrix

## Backend (Rust)

| Module | Unit Tests | Integration | Notes |
|--------|-----------|-------------|-------|
| managers/meeting | ✅ tests.rs | ❌ | Session lifecycle, state transitions |
| managers/audio | ❌ | ❌ | Needs cpal device mocking |
| managers/model | ❌ | ❌ | Needs HTTP mocking |
| managers/transcription | ❌ | ❌ | Needs model binary |
| audio_toolkit/vad | ❌ | ❌ | Needs ONNX model |
| commands/ | ❌ | ❌ | Covered by manager tests |

## Frontend (TypeScript/React)

| Area | Component Tests | E2E | Notes |
|------|----------------|-----|-------|
| stores (Zustand) | ❌ | ❌ | No test runner configured |
| components | ❌ | ❌ | Needs vitest + jsdom |
| hooks | ❌ | ❌ | Needs React Testing Library |

## Platform Coverage

| Platform | CI Build | CI Test | Manual Test |
|----------|---------|---------|-------------|
| macOS ARM64 | ✅ | ❌ | ✅ |
| macOS Intel | ✅ | ❌ | ❌ |
| Windows x64 | ✅ | ❌ | ✅ |
| Windows ARM64 | ✅ | ❌ | ❌ |
| Linux Ubuntu 22.04 | ✅ | ✅ | ❌ |
| Linux Ubuntu 24.04 | ✅ | ❌ | ❌ |

## Test Gaps & Priorities

1. **HIGH**: Add vitest + basic store tests (settingsStore, meetingStore)
2. **HIGH**: Platform-specific unit tests in CI (macOS, Windows)
3. **MEDIUM**: VAD pipeline tests with sample audio
4. **MEDIUM**: Model download integration tests
5. **LOW**: E2E transcription tests (require Whisper model)
6. **LOW**: Component rendering tests

## Running Tests

```bash
# Backend
cd src-tauri && cargo test --lib

# Frontend (future)
bun run test
```
