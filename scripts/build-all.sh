#!/bin/bash

# Build script for all platforms
# Usage: ./scripts/build-all.sh

set -e

echo "🚀 Starting multi-platform build for Meetdy..."
echo ""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Get version from tauri.conf.json
VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | sed 's/.*: "\(.*\)".*/\1/')
echo "📦 Building version: $VERSION"
echo ""

# Create builds directory
mkdir -p builds

# Detect current platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    CURRENT_PLATFORM="macOS"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    CURRENT_PLATFORM="Linux"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" ]]; then
    CURRENT_PLATFORM="Windows"
else
    CURRENT_PLATFORM="Unknown"
fi

echo "🖥️  Current platform: $CURRENT_PLATFORM"
echo ""

# Function to build for a target
build_target() {
    local target=$1
    local platform_name=$2

    echo -e "${BLUE}Building for $platform_name ($target)...${NC}"

    if bun run tauri build --target $target; then
        echo -e "${GREEN}✅ $platform_name build completed${NC}"
        echo ""
        return 0
    else
        echo -e "${RED}❌ $platform_name build failed${NC}"
        echo ""
        return 1
    fi
}

# macOS builds (only on macOS)
if [[ "$CURRENT_PLATFORM" == "macOS" ]]; then
    echo "🍎 Building macOS versions..."
    echo ""

    # Universal binary (recommended for distribution)
    if build_target "universal-apple-darwin" "macOS Universal (Intel + Apple Silicon)"; then
        # Copy to builds directory
        cp -r src-tauri/target/universal-apple-darwin/release/bundle/dmg/*.dmg builds/ 2>/dev/null || true
        cp -r src-tauri/target/universal-apple-darwin/release/bundle/macos/*.app builds/ 2>/dev/null || true
    fi

    # Individual architectures (optional)
    # build_target "x86_64-apple-darwin" "macOS Intel"
    # build_target "aarch64-apple-darwin" "macOS Apple Silicon"
fi

# Windows builds (only on Windows or with cross-compilation)
if [[ "$CURRENT_PLATFORM" == "Windows" ]]; then
    echo "🪟 Building Windows version..."
    echo ""

    if build_target "x86_64-pc-windows-msvc" "Windows x64"; then
        # Copy to builds directory
        cp -r src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/*.msi builds/ 2>/dev/null || true
        cp -r src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe builds/ 2>/dev/null || true
    fi
fi

# Linux builds (only on Linux)
if [[ "$CURRENT_PLATFORM" == "Linux" ]]; then
    echo "🐧 Building Linux version..."
    echo ""

    if build_target "x86_64-unknown-linux-gnu" "Linux x64"; then
        # Copy to builds directory
        cp -r src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/deb/*.deb builds/ 2>/dev/null || true
        cp -r src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/appimage/*.AppImage builds/ 2>/dev/null || true
    fi
fi

echo ""
echo -e "${GREEN}🎉 Build process completed!${NC}"
echo ""
echo "📁 Build artifacts location:"
echo "  - macOS: src-tauri/target/universal-apple-darwin/release/bundle/"
echo "  - Windows: src-tauri/target/x86_64-pc-windows-msvc/release/bundle/"
echo "  - Linux: src-tauri/target/x86_64-unknown-linux-gnu/release/bundle/"
echo ""
echo "📋 Copied builds to: ./builds/"
ls -lh builds/ 2>/dev/null || echo "  (No files copied)"
echo ""
