#!/usr/bin/env -S uv run --script
"""Read the canonical native source inventory for Tier 2 graph validation."""
import argparse
import json
from pathlib import Path
import re

MEMBER = re.compile(r'^\s*#include\s+"libobs/([^"\n]+\.(?:c|m)\.h)"\s*$', re.MULTILINE)
HOSTS = ("linux", "windows", "macos")


def source_sets(root: Path) -> dict[str, list[str]]:
    build = root / "libobs/build"
    sets = json.loads((build / "source-sets.json").read_text())
    for fragment in sorted((build / "ported-source-sets").glob("*.json")):
        additions = json.loads(fragment.read_text())
        for name, members in additions.items():
            if name not in sets or not name.endswith("_PORTED_SOURCES"):
                raise ValueError(f"unknown ported source set in {fragment}: {name}")
            duplicates = set(members) & set(sets[name])
            if len(members) != len(set(members)) or duplicates:
                raise ValueError(f"duplicate ported source in {fragment}: {sorted(duplicates)}")
            sets[name].extend(members)
    for name in sets:
        if name.endswith("_PORTED_SOURCES"):
            sets[name].sort()
    return sets


def group_members(root: Path, group: str) -> list[str]:
    """Implementation files (relative to libobs/) that one unity group includes."""
    return MEMBER.findall((root / "libobs" / group).read_text())


def ported_set_names(host: str, hevc: bool = True, pulse: bool = True,
                     wayland: bool = True, gio: bool = True) -> list[str]:
    if host not in HOSTS:
        raise ValueError(f"unknown native host: {host}")
    names = ["LIBOBS_PORTED_SOURCES", f"LIBOBS_{host.upper()}_PORTED_SOURCES"]
    if hevc:
        names.append("LIBOBS_HEVC_PORTED_SOURCES")
    # ENABLE_PULSEAUDIO, Linux (and FreeBSD) only.
    if pulse and host == "linux":
        names.append("LIBOBS_PULSE_PORTED_SOURCES")
    if gio and host == "linux":
        names.append("LIBOBS_NIX_GIO_PORTED_SOURCES")
    # ENABLE_WAYLAND, Linux (and FreeBSD) only.
    if wayland and host == "linux":
        names.append("LIBOBS_WAYLAND_PORTED_SOURCES")
        # The GlobalShortcuts portal needs GIO too.
        if gio:
            names.append("LIBOBS_WAYLAND_GIO_PORTED_SOURCES")
    return names


def ported_objects(root: Path, host: str, hevc: bool = True, pulse: bool = True,
                   wayland: bool = True, gio: bool = True) -> list[str]:
    names = ported_set_names(host, hevc, pulse, wayland, gio)
    sets = source_sets(root)
    objects = set()
    for name in names:
        for group in sets[name]:
            members = group_members(root, group)
            if not members:
                raise ValueError(f"ported group contains no implementation members: {group}")
            objects.update(member + ".o" for member in members)
    return sorted(objects)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--host", required=True, choices=list(HOSTS))
    parser.add_argument("--without-hevc", action="store_true")
    parser.add_argument("--without-pulse", action="store_true")
    parser.add_argument("--without-wayland", action="store_true")
    parser.add_argument("--without-gio", action="store_true")
    arguments = parser.parse_args()
    print("\n".join(ported_objects(arguments.root, arguments.host, not arguments.without_hevc,
                                    not arguments.without_pulse, not arguments.without_wayland,
                                    not arguments.without_gio)))
