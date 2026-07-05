# Runbook

Operational procedures for Meetdy development and deployment.

## Development

```bash
bun install
bun run tauri dev
# If cmake error on macOS:
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev
```

## Building

```bash
bun run tauri build
```

## Model Setup

```bash
mkdir -p src-tauri/resources/models
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
```

## Linting & Formatting

```bash
bun run lint              # ESLint
bun run lint:fix          # ESLint auto-fix
bun run format            # Prettier + cargo fmt
bun run format:check      # Check only
```

## Platform Notes

- **macOS**: Metal GPU acceleration, accessibility permissions required
- **Windows**: Vulkan GPU acceleration, code signing
- **Linux**: OpenBLAS + Vulkan, limited Wayland support, overlay disabled
