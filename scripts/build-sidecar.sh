#!/bin/bash
# Build the Rust Hudsucker sidecar and place it in src-tauri/binaries/ with the correct target triple.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BINARIES_DIR="$PROJECT_ROOT/src-tauri/binaries"

TARGET_TRIPLE=$(rustc --print host-tuple)
echo "Building sidecar for target: $TARGET_TRIPLE"

cargo build --release -p tbhd-sidecar --manifest-path "$PROJECT_ROOT/Cargo.toml"

mkdir -p "$BINARIES_DIR"
if [[ "${TARGET_TRIPLE}" == *"windows"* ]]; then
  cp "$PROJECT_ROOT/target/release/tbhd-sidecar.exe" "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}.exe"
  chmod +x "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}.exe"
  echo "Sidecar placed: $BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}.exe"
  echo "Size: $(du -h "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}.exe" | cut -f1)"
else
  cp "$PROJECT_ROOT/target/release/tbhd-sidecar" "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}"
  chmod +x "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}"
  echo "Sidecar placed: $BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}"
  echo "Size: $(du -h "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}" | cut -f1)"
fi
