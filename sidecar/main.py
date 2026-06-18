"""Entry point for the Tauri sidecar mitmproxy process.

When compiled with Nuitka, addon.py is bundled alongside the binary.
In dev mode, it resolves relative to this script's source location.
"""

import os
import sys
from pathlib import Path


def main():
    log_path = Path(
        os.environ.get("TBH_MITM_LOG", "~/.cache/tbh_mitmproxy.log")
    ).expanduser()
    log_path.parent.mkdir(parents=True, exist_ok=True)

    if sys.stdout is None:
        sys.stdout = log_path.open("a", encoding="utf-8")
    if sys.stderr is None:
        sys.stderr = log_path.open("a", encoding="utf-8")

    # Locate addon.py relative to this executable/script
    if getattr(sys, "frozen", False):
        # Nuitka compiled binary — addon.py is next to the binary
        app_dir = Path(sys.executable).parent
    else:
        # Running as raw Python script
        app_dir = Path(__file__).resolve().parent

    addon_path = app_dir / "addon.py"
    if not addon_path.exists():
        print(f"[TBH] ERROR: addon.py not found at {addon_path}", file=sys.stderr)
        sys.exit(1)

    # Inject -s addon.py while preserving Tauri-provided mitmdump options.
    sys.argv = [sys.argv[0], "-s", str(addon_path), *sys.argv[1:]]

    from mitmproxy.tools.main import mitmdump

    mitmdump()


if __name__ == "__main__":
    main()
