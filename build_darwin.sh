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

# ---- macOS (native cross-arch) ----
# Requires Xcode CLTs. Cross-compiling between Apple archs is supported.
rustup target add aarch64-apple-darwin || true
rustup target add x86_64-apple-darwin  || true

echo "Building macOS arm64"
cargo build --release --target aarch64-apple-darwin
cp "target/aarch64-apple-darwin/release/$BIN_NAME" "$DIST_DIR/${BIN_NAME}-macos-arm64"

echo "Building macOS x64"
cargo build --release --target x86_64-apple-darwin
cp "target/x86_64-apple-darwin/release/$BIN_NAME" "$DIST_DIR/${BIN_NAME}-macos-x64"

# ---- Done ----
echo "macOS artifacts in $DIST_DIR/:"
ls -al "$DIST_DIR" | grep macos
