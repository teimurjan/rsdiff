#!/usr/bin/env bash
set -euo pipefail

# ---- Config ----
BIN_NAME=${BIN_NAME:-rsdiff}
DIST_DIR=${DIST_DIR:-../packages/bin/binaries}
export RUSTFLAGS=${RUSTFLAGS:-}

mkdir -p "$DIST_DIR"

# Ensure rustup/cargo present
command -v rustup >/dev/null || { echo "rustup is required"; exit 1; }
command -v cargo  >/dev/null || { echo "cargo is required";  exit 1; }

echo "Building for all platforms..."

echo "Building with build_darwin.sh..."
./build_darwin.sh
echo "Building with build_linux.sh..."
./build_linux.sh
echo "Building with build_win.sh..."
./build_win.sh


# ---- Done ----
echo "All artifacts in $DIST_DIR/:"
ls -al "$DIST_DIR"