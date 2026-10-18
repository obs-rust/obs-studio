#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["websockets==15.0.1"]
# ///
"""Full-app recording evidence for the 5% recording release gate (#1305)."""

from __future__ import annotations


class RunError(Exception):
    pass


def run_metrics(samples: list[dict], rec_start: dict, rec_end: dict) -> dict:
    raise NotImplementedError


def report(runs: list[dict], min_runs: int = 5) -> dict:
    raise NotImplementedError


def format_report(rep: dict) -> str:
    raise NotImplementedError


def encoder_lines(log: str, expect: str) -> list[str]:
    raise NotImplementedError
