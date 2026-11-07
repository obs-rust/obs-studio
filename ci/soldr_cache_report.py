#!/usr/bin/env -S uv run --script
"""RED stage: the step's original top-level parser, moved verbatim."""
import json


def format_report(raw, wrapper):
    d = json.loads(raw)
    rate = d.get("hit_rate", 0.0) or 0.0
    hits, misses = d.get("hits", 0), d.get("misses", 0)
    saved = d.get("time_saved_ms", 0) or 0
    return "\n".join([
        "### soldr cache",
        f"- hit rate: **{rate * 100:.1f}%** ({hits} hits / {misses} misses)",
        f"- time saved: {saved / 1000:.1f} s",
        f"- compilations: {d.get('compilations', 0)} (non-cacheable {d.get('non_cacheable', 0)})",
    ])


def main():
    return 0
