#!/usr/bin/env -S uv run --script
"""Write the soldr cache session to a job summary; never fail the job.

soldr nests every counter under `last_session`. `hit_rate` is a 0-1 fraction.
SOLDR_RUSTC_WRAPPER=none takes rustc away from the compiler cache, so a session
then says nothing about Rust: that is "n/a (bypassed)", never 0%.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys

NO_SESSION = "no soldr cache session recorded"
TIMEOUT_SECONDS = 60


def format_report(raw: str, wrapper: str | None) -> str:
    bypassed = wrapper == "none"
    rust = "n/a (bypassed)" if bypassed else f"active (wrapper {wrapper or 'default'})"
    lines = ["### soldr cache", f"- Rust compiler cache: {rust}"]
    try:
        data = json.loads(raw)
        session = data.get("last_session") if isinstance(data, dict) else None
        if not isinstance(session, dict):
            return "\n".join([*lines, f"- {NO_SESSION}"])
        hits, misses = session.get("hits", 0), session.get("misses", 0)
        compilations = session.get("compilations", 0)
        if compilations:
            label = "hit rate (non-Rust compilers)" if bypassed else "hit rate"
            rate = float(session.get("hit_rate") or 0.0) * 100
            lines.append(f"- {label}: **{rate:.1f}%** ({hits} hits / {misses} misses)")
        else:
            lines.append(f"- hit rate: n/a ({'bypassed' if bypassed else 'no compilations'})")
        saved = float(session.get("time_saved_ms") or 0) / 1000
    except (TypeError, ValueError) as error:
        return "\n".join([*lines, f"- {NO_SESSION} (unreadable report: {type(error).__name__})"])
    lines += [
        f"- time saved: {saved:.1f} s",
        f"- compilations: {compilations} (non-cacheable {session.get('non_cacheable', 0)})",
        f"- status: {session.get('status', 'unknown')} (errors {session.get('errors', 0)})",
        f"- zccache: {data.get('managed_zccache_version', 'unknown')}",
        f"- session source: {data.get('session_source', 'unknown')}",
    ]
    return "\n".join(lines)


def main() -> int:
    try:
        raw = subprocess.run(
            ["soldr", "cache", "report", "--json"],
            capture_output=True,
            check=True,
            encoding="utf-8",
            timeout=TIMEOUT_SECONDS,
        ).stdout
    except (OSError, subprocess.SubprocessError, UnicodeError):
        raw = "null"
    print(format_report(raw, os.environ.get("SOLDR_RUSTC_WRAPPER")))
    return 0


if __name__ == "__main__":
    sys.exit(main())
