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
# obs-rust's Sparkle feed (the latest certified release) and EdDSA key; release-rust.yaml's certify job writes the feed.
UPDATE_FEED_PREFIX = "https://github.com/obs-rust/obs-studio/releases/latest/download/"
SPARKLE_PUBLIC_KEY = "x7PaevvHxyhTaZdEFDhjr6+SqKf631gWXuIHowm6D3I="


def check_appdir(root: Path, *, deb: bool = False) -> None:  # noqa: C901
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
    links = {url.get("type"): url.text or "" for url in metadata.findall("url")}
    for kind in ("homepage", "bugtracker", "vcs-browser"):
        if not links.get(kind, "").startswith("https://github.com/obs-rust/"):
            raise RuntimeError(f"packaged metainfo {kind} is {links.get(kind)!r}, not obs-rust (#2302)")
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


def check_deb_recommends(field: str) -> None:
    """The .deb recommends xdg-open and a portal with a backend (#2301)."""
    names = {alt.strip().split(" ")[0] for item in field.split(",") for alt in item.split("|")}
    for need in ("xdg-utils", "xdg-desktop-portal", "xdg-desktop-portal-backend"):
        if need not in names:
            raise RuntimeError(f"Debian package Recommends {field!r} lacks {need}")


def check_deb_commands(names: list[str]) -> None:
    """Every command the .deb puts in /usr/bin is branded, so it installs
    next to the distribution's obs-studio (#2296)."""
    for name in names:
        if not name.startswith("obs-studio-rust"):
            raise RuntimeError(f"Debian package installs /usr/bin/{name}, which upstream OBS ships too")


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
    feed = metadata.get("SUFeedURL", "")
    if feed not in {f"{UPDATE_FEED_PREFIX}updates_{arch}_v2.xml" for arch in ("arm64", "x86_64")}:
        raise RuntimeError(f"packaged SUFeedURL is {feed!r}, expected obs-rust's update feed")
    if metadata.get("SUPublicEDKey") != SPARKLE_PUBLIC_KEY:
        raise RuntimeError("packaged SUPublicEDKey is not obs-rust's Sparkle key")
    if not (root / "Contents/Frameworks/Sparkle.framework").is_dir():
        raise RuntimeError("packaged app has an update feed but no Sparkle.framework")
    check_browser_helpers(root, metadata["CFBundleExecutable"])


# Chromium finds the browser source's helper apps by the main executable's
# name, and their bundle IDs follow the app's (#2386).
BROWSER_HELPER_FLAVORS = (("", ""), (" (GPU)", ".gpu"), (" (Plugin)", ".plugin"), (" (Renderer)", ".renderer"))


def check_browser_helpers(root: Path, executable: str) -> None:
    frameworks = root / "Contents/Frameworks"
    helpers = sorted(path.name for path in frameworks.glob("* Helper*.app"))
    if not helpers:
        return  # built without the browser source
    expected = {f"{executable} Helper{suffix}.app": f"{APP_ID}.helper{plist}" for suffix, plist in BROWSER_HELPER_FLAVORS}
    if sorted(expected) != helpers:
        raise RuntimeError(f"browser helpers are {helpers}, expected {sorted(expected)}: Chromium cannot launch them")
    for name, bundle_id in expected.items():
        with (frameworks / name / "Contents/Info.plist").open("rb") as stream:
            helper = plistlib.load(stream)
        if helper.get("CFBundleExecutable") != name.removesuffix(".app") or helper.get("CFBundleIdentifier") != bundle_id:
            raise RuntimeError(
                f"browser helper {name} has executable {helper.get('CFBundleExecutable')!r} and bundle ID "
                f"{helper.get('CFBundleIdentifier')!r}, expected {name.removesuffix('.app')!r} and {bundle_id!r}"
            )


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
        check_deb_recommends(
            subprocess.check_output(
                ["dpkg-deb", "--field", str(args.path), "Recommends"], text=True
            ).strip()
        )
        with tempfile.TemporaryDirectory(prefix="obs-deb-identity-") as scratch:
            subprocess.run(
                ["dpkg-deb", "--extract", str(args.path), scratch], check=True
            )
            check_appdir(Path(scratch), deb=True)
            check_deb_commands(sorted(p.name for p in (Path(scratch) / "usr/bin").iterdir()))
    elif args.kind == "bundle":
        check_bundle(args.path)
    else:
        check_appdir(args.path)
    print(f"packaged Rust app identity verified: {args.kind} {args.path.name}")


if __name__ == "__main__":
    main()
