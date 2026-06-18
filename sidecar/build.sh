#!/bin/bash
# Build the Python sidecar as a standalone binary using Nuitka.
#
# Prerequisites:
#   pip install nuitka mitmproxy
#
# Usage:
#   ./sidecar/build.sh
#
# Output: sidecar/dist/tbhd-sidecar

set -euo pipefail

cd "$(dirname "$0")"

echo "Installing dependencies..."
pip install -r requirements.txt nuitka

echo "Building sidecar binary with Nuitka..."
python -m nuitka \
    --mode=onefile \
    --output-filename=tbhd-sidecar \
    --include-package=mitmproxy \
    --include-package=mitmproxy.tools \
    --include-data-files=addon.py=addon.py \
    --nofollow-import-to=tkinter,matplotlib,numpy,pandas \
    --follow-imports \
    main.py

echo "Built: dist/tbhd-sidecar"
echo "Next: rename with target triple and place in src-tauri/binaries/"
