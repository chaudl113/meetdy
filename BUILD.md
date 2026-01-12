# Build Guide - Meetdy

## Prerequisites

**Required:**
- [Rust](https://rustup.rs/) (latest stable)
- [Bun](https://bun.sh/) (latest)
- Platform-specific tools (see below)

**Download VAD Model:**
```bash
mkdir -p src-tauri/resources/models
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
```

## Platform-Specific Setup

### macOS
```bash
# Install Xcode Command Line Tools
xcode-select --install
```

### Windows
- Visual Studio 2022 with C++ build tools
- Windows 10/11 SDK

### Linux (Ubuntu/Debian)
```bash
sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

## Build Commands

### Install Dependencies
```bash
bun install
```

### Development
```bash
# Run in dev mode
bun run tauri dev

# Or with environment variable if cmake error
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev
```

### Production Build

**Single Platform (current platform):**
```bash
bun run tauri:build
```

**Specific Platforms:**
```bash
# macOS Universal (Intel + Apple Silicon) - Recommended
bun run tauri:build:mac

# macOS Intel only
bun run tauri:build:mac:intel

# macOS Apple Silicon only
bun run tauri:build:mac:arm

# Windows x64
bun run tauri:build:windows

# Linux x64
bun run tauri:build:linux
```

**Build All (automated script):**
```bash
./scripts/build-all.sh
```

This script will:
- Detect your current platform
- Build for appropriate targets
- Copy artifacts to `./builds/` directory

## Build Artifacts Location

After building, artifacts will be located at:

### macOS
```
src-tauri/target/universal-apple-darwin/release/bundle/
├── dmg/
│   └── Meetdy_0.6.9_universal.dmg
└── macos/
    └── Meetdy.app
```

### Windows
```
src-tauri/target/x86_64-pc-windows-msvc/release/bundle/
├── msi/
│   └── Meetdy_0.6.9_x64_en-US.msi
└── nsis/
    └── Meetdy_0.6.9_x64-setup.exe
```

### Linux
```
src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/
├── deb/
│   └── meetdy_0.6.9_amd64.deb
└── appimage/
    └── meetdy_0.6.9_amd64.AppImage
```

## Cross-Platform Building

### From macOS
- ✅ macOS (native)
- ⚠️ Windows (requires Docker/VM)
- ⚠️ Linux (requires Docker/VM)

### From Windows
- ✅ Windows (native)
- ❌ macOS (not supported)
- ⚠️ Linux (requires WSL2/Docker)

### From Linux
- ✅ Linux (native)
- ❌ macOS (not supported)
- ⚠️ Windows (requires Wine/Docker)

**Recommended:** Use GitHub Actions or platform-specific build machines for cross-platform builds.

## GitHub Actions CI/CD

The project includes GitHub Actions workflows for automated builds:

- **On Push to `main`**: Auto-build all platforms
- **On Tag (`v*`)**: Build and create GitHub Release with installers

See `.github/workflows/` for configuration.

## Troubleshooting

### macOS: cmake error
```bash
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri build
```

### Windows: Missing WebView2
Install [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)

### Linux: Missing dependencies
```bash
# Check for missing libraries
ldd src-tauri/target/release/meetdy
```

### Build fails with "resource not found"
Ensure VAD model is downloaded:
```bash
ls -lh src-tauri/resources/models/silero_vad_v4.onnx
```

## Code Signing

### macOS
```bash
# Set signing identity in tauri.conf.json
"signingIdentity": "Developer ID Application: Your Name (TEAM_ID)"

# Or use environment variable
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name"
bun run tauri build
```

### Windows
Configured in `tauri.conf.json`:
```json
"windows": {
  "signCommand": "trusted-signing-cli ..."
}
```

## Update Generation

Builds automatically generate update artifacts when:
```json
"bundle": {
  "createUpdaterArtifacts": true
}
```

Update files will be in bundle directory:
- `latest.json` - Update manifest
- `.tar.gz` / `.zip` - Update payloads

## Clean Build

```bash
# Clean Rust artifacts
cd src-tauri && cargo clean

# Clean frontend
rm -rf dist node_modules

# Full clean and rebuild
bun install && bun run tauri build
```

## Performance Tips

- **Incremental builds**: Rust caches builds, subsequent builds are faster
- **Parallel builds**: Use `--jobs N` flag for cargo
- **Release optimizations**: Enabled by default in production builds
- **LTO**: Already configured in `Cargo.toml` for release profile

## Version Management

Update version in:
1. `src-tauri/tauri.conf.json` - `"version": "x.y.z"`
2. `package.json` - `"version": "x.y.z"`
3. `src-tauri/Cargo.toml` - `version = "x.y.z"`

Then tag and build:
```bash
git tag v0.6.9
git push --tags
```

---

For more details, see:
- [Tauri Build Guide](https://v2.tauri.app/guides/building/)
- [Tauri Bundle Configuration](https://v2.tauri.app/guides/building/bundles/)
