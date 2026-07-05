# CHANGELOG

## v0.7.0

- Initial public release
- Tauri v2 desktop shell with React/TypeScript frontend
- Whisper speech-to-text pipeline (Small, Medium, Turbo, Large models)
- Parakeet engine support for alternative model formats
- Silero VAD for smart voice activity detection
- Push-to-talk recording via global keyboard shortcuts
- System tray integration (minimized startup)
- macOS (Intel + Apple Silicon), Windows (x64 + ARM64), Linux (deb, AppImage, RPM)
- GPU acceleration: Metal (macOS), Vulkan (Windows/Linux)
- i18n support via i18next
- Automatic updater support (Tauri updater plugin)
- CI/CD: GitHub Actions for multi-platform builds, lint, format, test
- VAD model caching in CI (Silero ONNX)
- RPM packages now use xz compression
- Security auditing via cargo-audit in CI

## v0.6.x (pre-release)

- Core pipeline: Audio → VAD → Whisper → Text
- Manager pattern for Audio, Model, Transcription
- Settings persistence via Tauri store
- Global shortcut configuration
- Clipboard integration for text output
- Model download progress in frontend
- Linux AppImage libwayland-client removal fix
