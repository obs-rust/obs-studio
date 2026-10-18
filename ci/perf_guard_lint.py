#!/usr/bin/env -S uv run --script
"""Perf guards (#1303): RED stub, finds nothing."""

from __future__ import annotations

import sys
from pathlib import Path


def load_guards(path: Path) -> list[dict]:
    return []


def check_source(text: str, guard: dict) -> list[str]:
    return []


def check_repo(root: Path) -> list[str]:
    return []


def main() -> int:
    return 0


if __name__ == "__main__":
    sys.exit(main())
