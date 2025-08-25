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

# Install helpers if missing
ensure_installed() {
  local cmd="$1" hint="$2"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Installing $cmd..."
    eval "$hint"
  fi
}

# ---- Windows x64 (MSVC) via cargo-xwin ----
ensure_installed cargo-xwin "cargo install cargo-xwin"
rustup target add x86_64-pc-windows-msvc || true

echo "Building Windows x64 (MSVC)"
cargo xwin build --release --target x86_64-pc-windows-msvc
cp "../target/x86_64-pc-windows-msvc/release/${BIN_NAME}.exe" "$DIST_DIR/${BIN_NAME}-windows-x64.exe"

# ---- Done ----
echo "Windows artifacts in $DIST_DIR/:"
ls -al "$DIST_DIR" | grep windows
