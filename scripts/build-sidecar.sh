#!/bin/bash
# Build the Python sidecar and place it in src-tauri/binaries/ with the correct target triple.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SIDECAR_DIR="$PROJECT_ROOT/sidecar"
BINARIES_DIR="$PROJECT_ROOT/src-tauri/binaries"

TARGET_TRIPLE=$(rustc --print host-tuple)
echo "Building sidecar for target: $TARGET_TRIPLE"

# Build
cd "$SIDECAR_DIR"
./build.sh

# Place
mkdir -p "$BINARIES_DIR"
cp dist/tbhd-sidecar "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}"
chmod +x "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}"

echo "Sidecar placed: $BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}"
echo "Size: $(du -h "$BINARIES_DIR/tbhd-sidecar-${TARGET_TRIPLE}" | cut -f1)"
