#!/usr/bin/env -S uv run --script
"""Fail when a pinned CEF's Chromium is older than extended stable (#2380).

CEF releases are named by Chromium branch (7871, 8078), and Chrome versions
carry the branch as their third part (156.0.8078.48), so a pin is supported
while its branch is at least the extended-stable channel's. Pins come from
CMakePresets.json (`dependencies.cef`: a default `version`, per-platform
`versions` overrides) and the Flatpak manifest's CEF download.

A platform can only move to a branch once the obs-rust/cef mirror publishes
an archive for it (Windows and Linux aarch64 have no 8078 build host yet,
#1864). An outdated pin fails when the mirror already has a supported build
for that platform; otherwise it is reported as a warning, since no pin change
here could fix it.

    uv run ci/cef_support.py                  # fetch Chrome's and the mirror's release data
    uv run ci/cef_support.py FILE.json [MIRROR.json]  # or read them from files
"""

import json
from typing import Any
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PRESETS = ROOT / "CMakePresets.json"
FLATPAK = ROOT / "build-aux" / "io.github.obs_rust.OBSStudioRust.json"
MIRROR = "https://api.github.com/repos/obs-rust/cef/releases?per_page=100"
# Platform -> the platform part of the mirror's cef_binary_<branch>_<part> archive names.
ASSET_PLATFORMS = {
    "flatpak": "linux_x86_64",
    "ubuntu-x86_64": "linux_x86_64",
    "ubuntu-aarch64": "linux_aarch64",
    "macos-x86_64": "macos_x86_64",
    "macos-arm64": "macos_arm64",
    "windows-x64": "windows_x64",
    "windows-arm64": "windows_arm64",
}
RELEASES = "https://versionhistory.googleapis.com/v1/chrome/platforms/win/channels/extended/versions?pageSize=20"


def pins() -> dict[str, int]:
    """Platform -> pinned CEF branch."""
    presets = json.loads(PRESETS.read_text(encoding="utf-8"))
    cef = next(
        vendor["dependencies"]["cef"]
        for preset in presets["configurePresets"]
        if "cef"
        in (
            vendor := preset.get("vendor", {}).get("obsproject.com/obs-studio", {})
        ).get("dependencies", {})
    )
    found = {
        platform: int(cef.get("versions", {}).get(platform, cef["version"]))
        for platform in cef["hashes"]
    }
    branch = flatpak_branch(FLATPAK.read_text(encoding="utf-8"))
    if branch is not None:
        found["flatpak"] = branch
    return found


def flatpak_branch(text: str) -> int | None:
    """The CEF branch of the obs-rust/cef release a URL in text downloads.

    A rebuild of a branch is its own release, <branch>-r<n> (8078-r2)."""
    match = re.search(r"/cef/releases/download/(\d+)(?:-r\d+)?/", text)
    return int(match.group(1)) if match else None


def extended_stable(data: dict[str, Any]) -> tuple[int, int]:
    """(Chromium major, branch) of the newest extended-stable release."""
    versions = [
        tuple(int(part) for part in v["version"].split(".")) for v in data["versions"]
    ]
    newest = max(versions)
    return newest[0], newest[2]


def published(releases: list[dict[str, Any]]) -> dict[str, set[int]]:
    """Mirror archive platform (linux_x86_64, ...) -> CEF branches published for it."""
    found: dict[str, set[int]] = {}
    for release in releases:
        for asset in release.get("assets", []):
            match = re.fullmatch(
                r"cef_binary_(\d+)_([a-z0-9_]+?)\.(?:tar\.xz|zip)", asset["name"]
            )
            if match:
                found.setdefault(match.group(2), set()).add(int(match.group(1)))
    return found


def check(
    pinned: dict[str, int], stable: tuple[int, int], mirror: dict[str, set[int]]
) -> tuple[list[str], list[str]]:
    """(failures, warnings): an outdated pin fails only if the mirror has a supported build for it."""
    major, branch = stable
    failures: list[str] = []
    warnings: list[str] = []
    for platform, pin in sorted(pinned.items()):
        if pin >= branch:
            continue
        message = f"{platform}: CEF {pin} is older than Chrome's extended stable (Chromium {major}, branch {branch})"
        available = sorted(
            b
            for b in mirror.get(ASSET_PLATFORMS.get(platform, platform), set())
            if b >= branch
        )
        if available:
            failures.append(
                f"{message}; the mirror publishes CEF {available[-1]} for it"
            )
        else:
            warnings.append(
                f"{message}; the mirror has no supported build for it yet (#1864)"
            )
    return failures, warnings


def fetch(source: str):
    """JSON from a URL, or from a file path."""
    if source.startswith("https://"):
        request = urllib.request.Request(
            source, headers={"User-Agent": "obs-rust-cef-support"}
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)
    return json.loads(Path(source).read_text(encoding="utf-8"))


def main(argv: list[str]) -> int:
    data = fetch(argv[1] if len(argv) > 1 else RELEASES)
    mirror = published(fetch(argv[2] if len(argv) > 2 else MIRROR))
    stable = extended_stable(data)
    pinned = pins()
    print(f"Chrome extended stable: Chromium {stable[0]}, branch {stable[1]}")
    for platform, pin in sorted(pinned.items()):
        print(f"  {platform}: CEF {pin}")
    failures, warnings = check(pinned, stable, mirror)
    for warning in warnings:
        print(f"::warning::{warning}")
    for failure in failures:
        print(f"::error::{failure}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
