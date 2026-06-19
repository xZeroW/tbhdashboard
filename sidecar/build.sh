#!/bin/bash
# Compatibility wrapper. The sidecar is now the Rust Hudsucker crate.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

"$PROJECT_ROOT/scripts/build-sidecar.sh"
