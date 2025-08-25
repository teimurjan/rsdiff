#!/usr/bin/env bash
set -euo pipefail

# ---- Config ----
BIN_NAME=${BIN_NAME:-rsdiff}
DIST_DIR=${DIST_DIR:-packages/bin/binaries}
export RUSTFLAGS=${RUSTFLAGS:-}

mkdir -p "$DIST_DIR"

# Ensure rustup/cargo present
command -v rustup >/dev/null || { echo "rustup is required"; exit 1; }
command -v cargo  >/dev/null || { echo "cargo is required";  exit 1; }

echo "Building for all platforms..."

# Build for current platform
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Detected macOS, building with build_darwin.sh..."
    ./build_darwin.sh
elif [[ "$OSTYPE" == "linux-gnu"* ]] || [[ "$OSTYPE" == "linux-musl"* ]]; then
    echo "Detected Linux, building with build_linux.sh..."
    ./build_linux.sh
elif [[ "$OSTYPE" == "msys"* ]] || [[ "$OSTYPE" == "cygwin"* ]]; then
    echo "Detected Windows, building with build_win.sh..."
    ./build_win.sh
else
    echo "Unknown platform: $OSTYPE"
    echo "Please run the appropriate build script manually:"
    echo "  ./build_darwin.sh  # for macOS"
    echo "  ./build_linux.sh   # for Linux"
    echo "  ./build_win.sh     # for Windows"
    exit 1
fi

# ---- Done ----
echo "All artifacts in $DIST_DIR/:"
ls -al "$DIST_DIR"