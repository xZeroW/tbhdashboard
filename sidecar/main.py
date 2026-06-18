"""Entry point for the Tauri sidecar mitmproxy process."""

import sys

from addon import response

__all__ = ["response"]

if __name__ == "__main__":
    print("This script is designed to be run as: mitmdump -s main.py", file=sys.stderr)
    print("It requires mitmproxy to be installed: pip install mitmproxy", file=sys.stderr)
