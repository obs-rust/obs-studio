#!/usr/bin/env -S uv run --script
"""Snapshot/compare native and cross CTest registries on exactly one commit."""

from __future__ import annotations
import argparse
import json
from pathlib import Path
import subprocess


def normalized(snapshot: dict) -> dict:
    def paths(value):
        if isinstance(value, str):
            return value.replace(snapshot["build_root"], "<BUILD>").replace(
                snapshot["source_root"], "<SOURCE>"
            )
        if isinstance(value, list):
            return [paths(item) for item in value]
        if isinstance(value, dict):
            return {key: paths(item) for key, item in value.items()}
        return value

    tests = {}
    for test in snapshot["registry"]["tests"]:
        name = test["name"]
        if name in tests:
            raise ValueError(f"duplicate CTest name: {name}")
        tests[name] = paths(
            {
                "command": test.get("command"),
                "properties": sorted(
                    test.get("properties", []), key=lambda p: p["name"]
                ),
            }
        )
    if not tests:
        raise ValueError("empty CTest registry")
    return tests


def compare(native: dict, cross: dict) -> dict:
    if native["revision"] != cross["revision"]:
        raise ValueError("registry evidence must describe the same commit")
    left, right = normalized(native), normalized(cross)
    difference = {}
    for name, names in [
        ("missing", left.keys() - right.keys()),
        ("extra", right.keys() - left.keys()),
    ]:
        if names:
            difference[name] = sorted(names)
    changed = {
        name: {"native": left[name], "cross": right[name]}
        for name in sorted(left.keys() & right.keys())
        if left[name] != right[name]
    }
    if changed:
        difference["changed"] = changed
    return difference


def snapshot(build: Path, source: Path) -> dict:
    revision = subprocess.check_output(
        ["git", "-C", str(source), "rev-parse", "HEAD"], text=True
    ).strip()
    # A snapshot cannot claim a commit while tracked source edits are present.
    subprocess.run(
        ["git", "-C", str(source), "diff", "--exit-code", "HEAD", "--"],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    result = subprocess.run(
        [
            "ctest",
            "--test-dir",
            str(build),
            "-C",
            "RelWithDebInfo",
            "--show-only=json-v1",
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    registry = json.loads(result.stdout)
    normalized(
        {
            "registry": registry,
            "source_root": str(source.resolve()),
            "build_root": str(build.resolve()),
        }
    )
    return {
        "revision": revision,
        "source_root": str(source.resolve()),
        "build_root": str(build.resolve()),
        "registry": registry,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    take = sub.add_parser("snapshot")
    take.add_argument("--build", type=Path, required=True)
    take.add_argument("--source", type=Path, required=True)
    take.add_argument("--output", type=Path, required=True)
    diff = sub.add_parser("compare")
    diff.add_argument("--native", type=Path, required=True)
    diff.add_argument("--cross", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "snapshot":
        args.output.write_text(
            json.dumps(snapshot(args.build, args.source), indent=2) + "\n"
        )
        return 0
    difference = compare(
        json.loads(args.native.read_text()), json.loads(args.cross.read_text())
    )
    print(json.dumps(difference, indent=2))
    return int(bool(difference))


if __name__ == "__main__":
    raise SystemExit(main())
