#!/bin/bash

# Meeting Mode Core Foundation - Development Environment Setup
# This script initializes the development environment for the Meetdy app

set -e

echo "=== Meeting Mode Core Foundation - Dev Environment Setup ==="
echo ""

# Navigate to project root
cd "$(dirname "$0")/../../.."
PROJECT_ROOT=$(pwd)
echo "Project root: $PROJECT_ROOT"

# Check for required tools
echo ""
echo "Checking required tools..."

# Node.js
if ! command -v node &> /dev/null; then
    echo "ERROR: Node.js is not installed. Please install Node.js 18+ first."
    exit 1
fi
echo "  Node.js: $(node --version)"

# npm
if ! command -v npm &> /dev/null; then
    echo "ERROR: npm is not installed."
    exit 1
fi
echo "  npm: $(npm --version)"

# Rust/Cargo
if ! command -v cargo &> /dev/null; then
    echo "ERROR: Rust/Cargo is not installed. Please install Rust first."
    echo "  Visit: https://rustup.rs/"
    exit 1
fi
echo "  Rust: $(rustc --version)"
echo "  Cargo: $(cargo --version)"

# Tauri CLI (check if available via npm scripts)
echo ""
echo "Checking Tauri CLI..."
if ! npm list @tauri-apps/cli --depth=0 &> /dev/null; then
    echo "  Installing Tauri CLI..."
    npm install -D @tauri-apps/cli
fi
echo "  Tauri CLI: available"

# Install frontend dependencies
echo ""
echo "Installing frontend dependencies..."
npm install

# Check Rust dependencies
echo ""
echo "Checking Rust dependencies..."
cd src-tauri
cargo check --quiet 2>/dev/null || {
    echo "  Running cargo build to fetch dependencies..."
    cargo build --quiet
}
cd ..

# Verify key files exist
echo ""
echo "Verifying project structure..."
REQUIRED_FILES=(
    "src/App.tsx"
    "src/stores/settingsStore.ts"
    "src/components/Sidebar.tsx"
    "src-tauri/src/lib.rs"
    "src-tauri/src/managers/mod.rs"
    "src-tauri/src/managers/audio.rs"
    "src-tauri/src/managers/history.rs"
    "src-tauri/src/managers/transcription.rs"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo "  $file"
    else
        echo "  WARNING: Missing $file"
    fi
done

# Display development commands
echo ""
echo "=== Setup Complete ==="
echo ""
echo "Development commands:"
echo "  npm run dev          - Start Vite + Tauri dev servers"
echo "  cargo tauri dev      - Alternative: Start Tauri in dev mode"
echo "  cargo check          - Check Rust code for errors"
echo "  cargo test           - Run Rust tests"
echo ""
echo "Key directories:"
echo "  src/                 - React/TypeScript frontend"
echo "  src/stores/          - Zustand state stores"
echo "  src/components/      - React components"
echo "  src-tauri/src/       - Rust backend"
echo "  src-tauri/src/managers/ - Backend managers"
echo "  src-tauri/src/commands/ - Tauri commands"
echo ""
echo "Files to create for Meeting Mode:"
echo "  src-tauri/src/managers/meeting.rs"
echo "  src-tauri/src/commands/meeting.rs"
echo "  src/stores/meetingStore.ts"
echo "  src/components/meeting/MeetingMode.tsx"
echo "  src/components/meeting/MeetingControls.tsx"
echo "  src/components/meeting/MeetingStatusIndicator.tsx"
echo "  src/components/meeting/MeetingTitleEditor.tsx"
echo ""
echo "Ready to start development!"
