#!/bin/bash
set -euo pipefail
echo "Cleaning build artifacts..."
rm -rf src-tauri/target/
rm -rf dist/
rm -rf sidecar/dist/
rm -rf sidecar/build/
rm -rf sidecar/main.build/
rm -rf sidecar/main.dist/
echo "Done"
