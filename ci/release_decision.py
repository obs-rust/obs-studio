#!/usr/bin/env -S uv run --script
"""Decisions for release-rust.yaml, kept out of the YAML so they can be tested.

    release_decision.py check   --event E --ref R --repair true|false \
                                --tag-exists true|false --tag-date YYYYMMDD --today YYYYMMDD
                                [--stable true]   (no --tag-date: obs-studio-rust-v<crate>)
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
    """Every file a complete release (daily beta or stable) carries."""
    p = f"OBS-Studio-Rust-{version}"
    return [
        f"{p}-ubuntu-26.04-x86_64.deb",
        f"{p}-linux-x86_64.AppImage",
        f"{p}-linux-x86_64.flatpak",
        f"{p}-macos-arm64.dmg",
        f"{p}-macos-x86_64.dmg",
        f"{p}-windows-x64.zip",
        f"{p}-windows-arm64.zip",
    ]


def missing_assets(version: str, present: list[str]) -> list[str]:
    have = set(present)
    return [a for a in required_assets(version) if a not in have]


def may_publish(gate: str, build: str, appimage: str, verify_only: bool = False) -> bool:
    """Ship only when the gate and every platform build succeeded."""
    return not verify_only and gate == build == appimage == "success"


def _require_today(tag_date: str, today: str) -> None:
    if tag_date != today:
        raise ValueError(f"tag date {tag_date} is not today ({today}, UTC)")


def check_stable(event: str, ref: str, repair: bool, tag_exists: bool, verify_only: bool = False) -> tuple[bool, bool]:
    """A stable release (obs-studio-rust-v<crate>, no beta date) is a deliberate
    maintainer dispatch from master. It never skips: an existing tag is an
    error unless the dispatch repairs it."""
    if event != "workflow_dispatch":
        raise ValueError("a stable release is only available from a manual dispatch")
    if verify_only:
        raise ValueError("stable and verify_only are exclusive; verify a branch without stable")
    if ref != MASTER_REF:
        raise ValueError(f"refusing to release {ref}: only {MASTER_REF} may publish a stable release")
    if tag_exists and not repair:
        raise ValueError("the stable tag already exists; dispatch with repair=true to replace it")
    return False, tag_exists


def check(
    event: str, ref: str, repair: bool, tag_exists: bool, tag_date: str, today: str, verify_only: bool = False,
    stable: bool = False,
) -> tuple[bool, bool]:
    """Return (skip, repair); raise ValueError for a refused run."""
    if stable:
        return check_stable(event, ref, repair, tag_exists, verify_only)
    if verify_only:
        if event != "workflow_dispatch":
            raise ValueError("verification is only available from a manual dispatch")
        if repair:
            raise ValueError("verification cannot repair or replace a release")
        _require_today(tag_date, today)
        return False, False
    if ref != MASTER_REF:
        raise ValueError(f"refusing to release {ref}: only {MASTER_REF} may publish the daily tag")
    _require_today(tag_date, today)
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
    c.add_argument("--tag-date", default="")
    c.add_argument("--today", required=True)
    c.add_argument("--verify-only", default="false")
    c.add_argument("--stable", default="false")
    p = sub.add_parser("publish")
    for n in ("gate", "build", "appimage"):
        p.add_argument(f"--{n}", required=True)
    p.add_argument("--verify-only", default="false")
    a = sub.add_parser("assets")
    a.add_argument("--version", required=True)
    a.add_argument("dir")
    args = ap.parse_args(argv)

    if args.cmd == "check":
        try:
            skip, repair = check(
                args.event, args.ref, _bool(args.repair), _bool(args.tag_exists), args.tag_date, args.today,
                verify_only=_bool(args.verify_only), stable=_bool(args.stable),
            )
        except ValueError as e:
            print(f"::error::{e}", file=sys.stderr)
            return 1
        print(f"skip={str(skip).lower()}")
        print(f"repair={str(repair).lower()}")
        print(f"verify_only={str(_bool(args.verify_only)).lower()}")
        print(f"stable={str(_bool(args.stable)).lower()}")
        return 0
    if args.cmd == "publish":
        if may_publish(args.gate, args.build, args.appimage, verify_only=_bool(args.verify_only)):
            return 0
        print(
            f"::error::not publishing: gate={args.gate} build={args.build} appimage={args.appimage} "
            f"verify_only={args.verify_only}"
        )
        return 1
    missing = missing_assets(args.version, [f.name for f in Path(args.dir).iterdir()])
    if missing:
        print("::error::missing release assets: " + ", ".join(missing))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
