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

# Install helpers if missing
ensure_installed() {
  local cmd="$1" hint="$2"
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "Installing $cmd..."
    eval "$hint"
  fi
}

# ---- Linux (portable) via musl + zig ----
ensure_installed zig "brew install zig"
ensure_installed cargo-zigbuild "cargo install cargo-zigbuild"

rustup target add x86_64-unknown-linux-musl || true
rustup target add aarch64-unknown-linux-musl || true

echo "Building Linux x64 (musl, static)"
cargo zigbuild --release --target x86_64-unknown-linux-musl
cp "target/x86_64-unknown-linux-musl/release/$BIN_NAME" "$DIST_DIR/${BIN_NAME}-linux-x64"

echo "Building Linux arm64 (musl, static)"
cargo zigbuild --release --target aarch64-unknown-linux-musl
cp "target/aarch64-unknown-linux-musl/release/$BIN_NAME" "$DIST_DIR/${BIN_NAME}-linux-arm64"

# ---- Linux x64 (glibc, x86-64-v3) ----
cargo install cross --git https://github.com/cross-rs/cross
ensure_installed cross "cargo install cross --git https://github.com/cross-rs/cross"

echo "Building Linux x64 (glibc, x86-64-v3)"
RUSTFLAGS="-C target-cpu=x86-64-v3 -C lto=thin -C codegen-units=1" \
  cross build --release --target x86_64-unknown-linux-gnu
cp target/x86_64-unknown-linux-gnu/release/$BIN_NAME \
   "$DIST_DIR/${BIN_NAME}-linux-x64-gnu-v3"

# ---- Windows x64 (MSVC) via cargo-xwin ----
ensure_installed cargo-xwin "cargo install cargo-xwin"
rustup target add x86_64-pc-windows-msvc || true

echo "Building Windows x64 (MSVC)"
cargo xwin build --release --target x86_64-pc-windows-msvc
cp "target/x86_64-pc-windows-msvc/release/${BIN_NAME}.exe" "$DIST_DIR/${BIN_NAME}-windows-x64.exe"

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
echo "Artifacts in $DIST_DIR/:"
ls -al "$DIST_DIR"