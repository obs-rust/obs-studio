#!/usr/bin/env -S uv run --script
"""Read the canonical native source inventory for Tier 2 graph validation."""
import argparse
import json
from pathlib import Path
import re

MEMBER = re.compile(r'^\s*#include\s+"libobs/([^"\n]+\.(?:c|m)\.h)"\s*$', re.MULTILINE)


def ported_objects(root: Path, host: str, hevc: bool = True, pulse: bool = True) -> list[str]:
    if host not in {"linux", "windows", "macos"}:
        raise ValueError(f"unknown native host: {host}")
    libobs = root / "libobs"
    sets = json.loads((libobs / "build/source-sets.json").read_text())
    names = ["LIBOBS_PORTED_SOURCES", f"LIBOBS_{host.upper()}_PORTED_SOURCES"]
    if hevc:
        names.append("LIBOBS_HEVC_PORTED_SOURCES")
    # ENABLE_PULSEAUDIO, Linux (and FreeBSD) only.
    if pulse and host == "linux":
        names.append("LIBOBS_PULSE_PORTED_SOURCES")
    objects = set()
    for name in names:
        for group in sets[name]:
            members = MEMBER.findall((libobs / group).read_text())
            if not members:
                raise ValueError(f"ported group contains no implementation members: {group}")
            objects.update(member + ".o" for member in members)
    return sorted(objects)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--host", required=True, choices=["linux", "windows", "macos"])
    parser.add_argument("--without-hevc", action="store_true")
    parser.add_argument("--without-pulse", action="store_true")
    arguments = parser.parse_args()
    print("\n".join(ported_objects(arguments.root, arguments.host, not arguments.without_hevc,
                                    not arguments.without_pulse)))
