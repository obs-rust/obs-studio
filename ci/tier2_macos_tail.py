#!/usr/bin/env -S uv run --script
"""Overlap independent macOS Tier 2 work on one runner (#5282)."""

from pathlib import Path


def run_tail(validator: Path, arguments: list[str], log_dir: Path) -> int:
    raise NotImplementedError("macOS Tier 2 still runs its tail serially")
