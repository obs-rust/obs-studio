#!/usr/bin/env -S uv run --script
"""Fail when a pinned CEF's Chromium is older than extended stable (#2380).

CEF releases are named by Chromium branch (7871, 8078), and Chrome versions
carry the branch as their third part (156.0.8078.48), so a pin is supported
while its branch is at least the extended-stable channel's. Pins come from
CMakePresets.json (`dependencies.cef`: a default `version`, per-platform
`versions` overrides) and the Flatpak manifest's CEF download.

    uv run ci/cef_support.py            # fetch Chrome's release data
    uv run ci/cef_support.py FILE.json  # or read it from a file
"""

import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PRESETS = ROOT / "CMakePresets.json"
FLATPAK = ROOT / "build-aux" / "io.github.obs_rust.OBSStudioRust.json"
RELEASES = "https://versionhistory.googleapis.com/v1/chrome/platforms/win/channels/extended/versions?pageSize=20"


def pins() -> dict[str, int]:
    """Platform -> pinned CEF branch."""
    presets = json.loads(PRESETS.read_text(encoding="utf-8"))
    cef = next(
        vendor["dependencies"]["cef"]
        for preset in presets["configurePresets"]
        if "cef" in (vendor := preset.get("vendor", {}).get("obsproject.com/obs-studio", {})).get("dependencies", {})
    )
    found = {platform: int(cef.get("versions", {}).get(platform, cef["version"])) for platform in cef["hashes"]}
    branch = flatpak_branch(FLATPAK.read_text(encoding="utf-8"))
    if branch is not None:
        found["flatpak"] = branch
    return found


def flatpak_branch(text: str) -> int | None:
    """The CEF branch of the obs-rust/cef release a URL in text downloads.

    A rebuild of a branch is its own release, <branch>-r<n> (8078-r2)."""
    match = re.search(r"/cef/releases/download/(\d+)(?:-r\d+)?/", text)
    return int(match.group(1)) if match else None


def extended_stable(data: dict) -> tuple[int, int]:
    """(Chromium major, branch) of the newest extended-stable release."""
    versions = [tuple(int(part) for part in v["version"].split(".")) for v in data["versions"]]
    newest = max(versions)
    return newest[0], newest[2]


def check(pinned: dict[str, int], stable: tuple[int, int]) -> list[str]:
    major, branch = stable
    return [
        f"{platform}: CEF {pin} is older than Chrome's extended stable (Chromium {major}, branch {branch})"
        for platform, pin in sorted(pinned.items())
        if pin < branch
    ]


def main(argv: list[str]) -> int:
    if len(argv) > 1:
        data = json.loads(Path(argv[1]).read_text(encoding="utf-8"))
    else:
        with urllib.request.urlopen(RELEASES, timeout=30) as response:
            data = json.load(response)
    stable = extended_stable(data)
    pinned = pins()
    print(f"Chrome extended stable: Chromium {stable[0]}, branch {stable[1]}")
    for platform, pin in sorted(pinned.items()):
        print(f"  {platform}: CEF {pin}")
    failures = check(pinned, stable)
    for failure in failures:
        print(f"::error::{failure}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
