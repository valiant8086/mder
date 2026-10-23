"""Writes a release's notes for GitHub, from that version's changelog.json
entry: its summary, its changes, and which file to download.

    python3 scripts/release-notes.py 0.2.0 > notes.md

Also checks the version: it fails if changelog.json's newest entry isn't
this one, so a release can't go out without its changes written down.
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    version = sys.argv[1].removeprefix("v")
    entries = json.loads((ROOT / "changelog.json").read_text(encoding="utf-8"))
    if not entries or entries[0].get("version") != version:
        newest = entries[0].get("version") if entries else "nothing"
        print(f"changelog.json's newest entry is {newest}, not {version}", file=sys.stderr)
        return 1
    entry = entries[0]

    lines = []
    if entry.get("blurb"):
        lines += [entry["blurb"], ""]
    if entry.get("changes"):
        lines += ["## What's new", ""]
        lines += [f"- {change}" for change in entry["changes"]]
        lines.append("")
    lines += [
        "## Which file do I download?",
        "",
        f"- **Windows:** `mder{version}-windows-x64.exe`, or `mder{version}-windows-arm64.exe` for Windows on ARM. "
        "It's a self-extractor: run it and choose a folder for mder. mder needs the WebView2 runtime, "
        "which comes with Windows 11, and with Windows 10 through Windows Update.",
        f"- **Mac:** `mder{version}-mac-universal.zip`, for Apple silicon and Intel Macs, macOS 12 or later. "
        "Move mder.app where you want to keep it, such as Applications, before opening it. "
        "mder isn't signed by Apple, so the first time, macOS refuses to open it: go to System Settings, "
        "Privacy & Security, and choose \"Open Anyway\".",
        f"- **Linux:** `mder{version}-linux-x64.tar.gz`, or `mder{version}-linux-arm64.tar.gz` for ARM machines "
        "such as a Raspberry Pi with a 64-bit system. It needs GTK 3 and WebKitGTK 4.1.",
        "",
        "Once installed, mder can update itself: Help, Check for updates. "
        "The [README](https://github.com/valiant8086/mder#readme) has the details.",
    ]
    print("\n".join(lines))
    return 0


if __name__ == "__main__":
    sys.exit(main())
