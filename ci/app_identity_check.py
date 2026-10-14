#!/usr/bin/env -S uv run --script
"""Check the installed Rust app identity in an AppDir, Debian package, or bundle."""

from __future__ import annotations

import argparse
import plistlib
import subprocess
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

APP_ID = "io.github.obs_rust.OBSStudioRust"
GUI = "obs-studio-rust-gui"


def check_appdir(root: Path, *, deb: bool = False) -> None:
    desktop = root / f"usr/share/applications/{APP_ID}.desktop"
    fields = {}
    for line in desktop.read_text().splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            fields[key] = value
    expected = {
        "Name": "OBS-Studio-Rust",
        "Exec": GUI,
        "Icon": APP_ID,
        "StartupWMClass": GUI,
    }
    for key, value in expected.items():
        if fields.get(key) != value:
            raise RuntimeError(
                f"packaged desktop {key} is {fields.get(key)!r}, expected {value!r}"
            )
    metadata = ET.parse(root / f"usr/share/metainfo/{APP_ID}.metainfo.xml").getroot()
    if (
        metadata.findtext("id") != APP_ID
        or metadata.findtext("launchable") != f"{APP_ID}.desktop"
    ):
        raise RuntimeError("packaged metainfo does not identify the Rust app")
    if not (root / f"usr/bin/{GUI}").is_file():
        raise RuntimeError("packaged Rust GUI executable is missing")
    if (root / "usr/bin/obs").exists():
        raise RuntimeError("Rust package would install over the upstream GUI")
    libraries = list((root / "usr").glob("lib*/**/obs-studio-rust/libobs.so*"))
    if not libraries:
        raise RuntimeError("Rust package has no private libobs installation")
    if not (root / "usr/share/obs-studio-rust/obs-studio").is_dir():
        raise RuntimeError("Rust package has no private frontend data")
    if deb:
        for relative in (
            "usr/share/obs",
            "usr/share/applications/com.obsproject.Studio.desktop",
        ):
            if (root / relative).exists():
                raise RuntimeError(
                    f"Debian package installs over upstream files: {relative}"
                )
        for library in (root / "usr").glob("lib*/**/libobs.so*"):
            if "obs-studio-rust" not in library.relative_to(root).parts:
                raise RuntimeError(
                    f"Debian package installs over upstream libobs: {library.relative_to(root)}"
                )


def check_bundle(root: Path) -> None:
    with (root / "Contents/Info.plist").open("rb") as stream:
        metadata = plistlib.load(stream)
    for key, expected in {
        "CFBundleIdentifier": APP_ID,
        "CFBundleExecutable": "OBS-Studio-Rust",
        "CFBundleDisplayName": "OBS-Studio-Rust",
    }.items():
        if metadata.get(key) != expected:
            raise RuntimeError(
                f"packaged {key} is {metadata.get(key)!r}, expected {expected!r}"
            )
    if (
        root.name != "OBS-Studio-Rust.app"
        or not (root / "Contents/MacOS/OBS-Studio-Rust").is_file()
    ):
        raise RuntimeError("Rust bundle would use the upstream app's installation name")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kind", choices=("appdir", "deb", "bundle"))
    parser.add_argument("path", type=Path)
    args = parser.parse_args()
    if args.kind == "deb":
        name = subprocess.check_output(
            ["dpkg-deb", "--field", str(args.path), "Package"], text=True
        ).strip()
        if name != "obs-studio-rust":
            raise RuntimeError(
                f"Debian package identity is {name!r}, expected obs-studio-rust"
            )
        with tempfile.TemporaryDirectory(prefix="obs-deb-identity-") as scratch:
            subprocess.run(
                ["dpkg-deb", "--extract", str(args.path), scratch], check=True
            )
            check_appdir(Path(scratch), deb=True)
    elif args.kind == "bundle":
        check_bundle(args.path)
    else:
        check_appdir(args.path)
    print(f"packaged Rust app identity verified: {args.kind} {args.path.name}")


if __name__ == "__main__":
    main()
