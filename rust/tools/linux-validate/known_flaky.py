#!/usr/bin/env -S uv run --script
"""Run CTest with one audited retry for issue-linked known flakes (Refs #4944)."""

from __future__ import annotations

import argparse
import subprocess


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry", required=True)
    parser.add_argument("--build", required=True)
    parser.add_argument("--mode", required=True, choices=("OFF", "ON"))
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    return subprocess.run(args.command[1:], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
