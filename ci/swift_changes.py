#!/usr/bin/env -S uv run --script
"""Whether this run has to check Swift formatting (#427, #962).

swift-format needs a macOS runner, one of the five the plan allows, so it
runs only when a Swift file changed. On a push and on a merge-queue run there
is no base to compare with, so the check always runs.

    uv run --no-project ci/swift_changes.py

Writes `swift=true|false` to $GITHUB_OUTPUT for the `swift-format` job.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def swift_changed(base: str) -> bool:
    """True when HEAD changes a .swift file relative to base."""
    changed = subprocess.run(
        ["git", "diff", "--name-only", base, "HEAD", "--", "*.swift"],
        capture_output=True,
        text=True,
    )
    if changed.returncode != 0:
        raise SystemExit(f"swift-changes: git diff {base} HEAD failed:\n{changed.stderr}")
    return bool(changed.stdout.strip())


def main() -> int:
    base = os.environ.get("BASE", "")
    needed = not base or swift_changed(base)
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as handle:
            handle.write(f"swift={str(needed).lower()}\n")
    print(f"Swift files changed: {str(needed).lower()}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
