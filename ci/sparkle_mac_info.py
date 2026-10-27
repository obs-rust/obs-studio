#!/usr/bin/env -S uv run --script
"""Write the macOS half of an update feed's input (`ci/update_feed.py --mac-info`).

    uv run --no-project --script ci/sparkle_mac_info.py \
        <dist> <sparkle-ed-key-file> <sign_update> <out.json>

For each DMG in <dist> (`*-macos-arm64.dmg`, `*-macos-x86_64.dmg`) it reads the
app's CFBundleVersion, CFBundleShortVersionString and LSMinimumSystemVersion out
of the mounted image and signs the DMG with Sparkle's `sign_update` (EdDSA).
macOS only (hdiutil, plutil). release-rust.yaml's certify (stable feed) and
nightly_feed jobs share it (#1742, #1759).
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ARCHS = ("arm64", "x86_64")
# The plist key behind each field of a feed entry.
PLIST_KEYS = {
    "bundle_version": "CFBundleVersion",
    "short_version": "CFBundleShortVersionString",
    "minimum_system_version": "LSMinimumSystemVersion",
}
_SIGNATURE = re.compile(r'sparkle:edSignature="([^"]*)"')


def run(argv: list[str]) -> str:
    """Run a macOS helper, returning its stdout; a failure ends the step."""
    result = subprocess.run(argv, capture_output=True, text=True)
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"sparkle_mac_info: {' '.join(argv)} failed")
    return result.stdout


def plist_value(plist: Path, key: str) -> str:
    """One raw value, as the shell read it: trailing newlines stripped."""
    return run(["plutil", "-extract", key, "raw", "-o", "-", str(plist)]).rstrip("\n")


def app_plist(mount: Path) -> Path:
    apps = sorted(mount.glob("*.app"))
    if len(apps) != 1:
        raise SystemExit(f"sparkle_mac_info: expected one app in {mount}, found {len(apps)}")
    return apps[0] / "Contents" / "Info.plist"


def fields(dmg: Path) -> dict[str, str]:
    """The image's app identity, mounted read-only for the read."""
    mount = Path(tempfile.mkdtemp())
    try:
        run(["hdiutil", "attach", "-nobrowse", "-readonly", "-mountpoint", str(mount), str(dmg)])
        try:
            plist = app_plist(mount)
            return {field: plist_value(plist, key) for field, key in PLIST_KEYS.items()}
        finally:
            # A failed read still leaves the image mounted for the runner.
            subprocess.run(["hdiutil", "detach", str(mount)], capture_output=True)
    finally:
        shutil.rmtree(mount, ignore_errors=True)


def signature(sign_update: Path, key: Path, dmg: Path) -> str:
    """The EdDSA signature `sign_update` prints for this image."""
    match = _SIGNATURE.search(run([str(sign_update), "--ed-key-file", str(key), str(dmg)]))
    if not match or not match.group(1):
        raise SystemExit(f"::error::sign_update produced no signature for {dmg}")
    return match.group(1)


def dmg_for(dist: Path, arch: str) -> Path:
    """The one DMG this architecture ships."""
    matches = sorted(dist.glob(f"*-macos-{arch}.dmg"))
    if len(matches) != 1:
        raise SystemExit(f"sparkle_mac_info: expected exactly one *-macos-{arch}.dmg in {dist},"
                         f" found {len(matches)}")
    return matches[0]


def write(path: Path, info: dict[str, dict[str, str]]) -> None:
    """Replace the file in one step, so a reader never sees half an entry."""
    temporary = path.with_name(path.name + ".new")
    temporary.write_text(json.dumps(info), encoding="utf-8")
    os.replace(temporary, path)


def main(argv: list[str]) -> int:
    if len(argv) != 5:
        print(__doc__, file=sys.stderr)
        return 2
    dist, key, sign_update, out = Path(argv[1]), Path(argv[2]), Path(argv[3]), Path(argv[4])
    write(out, {})
    info: dict[str, dict[str, str]] = {}
    for arch in ARCHS:
        dmg = dmg_for(dist, arch)
        info[arch] = dict(fields(dmg), ed_signature=signature(sign_update, key, dmg))
        write(out, info)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
