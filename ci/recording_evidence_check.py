#!/usr/bin/env -S uv run --script
"""Check that a recording evidence run used a hardware renderer and the expected libobs (#1305)."""

from __future__ import annotations

from pathlib import Path


class CheckError(Exception):
    pass


def renderer(log: str) -> dict:
    raise NotImplementedError


def libraries(maps: str, *, expect: str, prefix: Path | None = None) -> dict:
    raise NotImplementedError


def main(argv: list[str]) -> int:
    raise NotImplementedError
