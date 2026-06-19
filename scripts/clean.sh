#!/bin/bash
set -euo pipefail
echo "Cleaning build artifacts..."
rm -rf src-tauri/target/
rm -rf dist/
echo "Done"
