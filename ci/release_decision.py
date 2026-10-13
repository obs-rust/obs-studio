#!/usr/bin/env -S uv run --script
"""Decisions for release-rust.yaml, kept out of the YAML so they can be tested.

    release_decision.py check   --event E --ref R --repair true|false \
                                --tag-exists true|false --tag-date YYYYMMDD --today YYYYMMDD
    release_decision.py publish --gate S --build S --appimage S
    release_decision.py assets  --version V DIR

`check` prints skip=, repair= and fails (exit 1) on a refused run; `publish`
and `assets` exit 1 when the release must not ship (obs-rust/obs-studio#622,
#623).
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

MASTER_REF = "refs/heads/master"


def required_assets(version: str) -> list[str]:
    """Every file a complete daily beta carries."""
    p = f"OBS-Studio-Rust-{version}"
    return [
        f"{p}-linux-x86_64.deb",
        f"{p}-linux-x86_64.AppImage",
        f"{p}-macos-arm64.dmg",
        f"{p}-windows-x64.zip",
    ]


def missing_assets(version: str, present: list[str]) -> list[str]:
    have = set(present)
    return [a for a in required_assets(version) if a not in have]


def may_publish(gate: str, build: str, appimage: str) -> bool:
    """Ship only when the gate and every platform build succeeded."""
    return gate == build == appimage == "success"


def check(event: str, ref: str, repair: bool, tag_exists: bool, tag_date: str, today: str) -> tuple[bool, bool]:
    """Return (skip, repair); raise ValueError for a refused run."""
    if ref != MASTER_REF:
        raise ValueError(f"refusing to release {ref}: only {MASTER_REF} may publish the daily tag")
    if tag_date != today:
        raise ValueError(f"tag date {tag_date} is not today ({today}, UTC)")
    if repair and event != "workflow_dispatch":
        raise ValueError("repair is only available from a manual dispatch")
    if not tag_exists:
        return False, False
    if repair:
        return False, True
    return True, False


def _bool(s: str) -> bool:
    return s.strip().lower() == "true"


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("check")
    c.add_argument("--event", required=True)
    c.add_argument("--ref", required=True)
    c.add_argument("--repair", default="false")
    c.add_argument("--tag-exists", required=True)
    c.add_argument("--tag-date", required=True)
    c.add_argument("--today", required=True)
    p = sub.add_parser("publish")
    for n in ("gate", "build", "appimage"):
        p.add_argument(f"--{n}", required=True)
    a = sub.add_parser("assets")
    a.add_argument("--version", required=True)
    a.add_argument("dir")
    args = ap.parse_args(argv)

    if args.cmd == "check":
        try:
            skip, repair = check(args.event, args.ref, _bool(args.repair), _bool(args.tag_exists), args.tag_date, args.today)
        except ValueError as e:
            print(f"::error::{e}", file=sys.stderr)
            return 1
        print(f"skip={str(skip).lower()}")
        print(f"repair={str(repair).lower()}")
        return 0
    if args.cmd == "publish":
        if may_publish(args.gate, args.build, args.appimage):
            return 0
        print(f"::error::not publishing: gate={args.gate} build={args.build} appimage={args.appimage}")
        return 1
    missing = missing_assets(args.version, [f.name for f in Path(args.dir).iterdir()])
    if missing:
        print("::error::missing release assets: " + ", ".join(missing))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
