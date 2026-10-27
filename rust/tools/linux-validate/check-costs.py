#!/usr/bin/env -S uv run --script
"""Refuse an empty Tier 2 timing cache.

Both implementations must have measured tests before the merged cost data is
cached: an empty file would plan every test at ctest's 1 s default again.

    uv run --script rust/tools/linux-validate/check-costs.py .tier2-cost
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

COUNT = re.compile(r"^[0-9]+$")
TIME = re.compile(r"^[0-9]+(\.[0-9]+)?([eE][-+]?[0-9]+)?$")


def measured(data: str) -> int:
    """Count ctest cost rows; ctest's failure list follows the "---" line."""
    count = 0
    for line in data.splitlines():
        if line == "---":
            break
        fields = line.split()
        if (
            len(fields) == 3
            and COUNT.match(fields[1])
            and int(fields[1]) > 0
            and TIME.match(fields[2])
        ):
            count += 1
    return count


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: check-costs.py COST_DIR", file=sys.stderr)
        return 2
    directory = Path(argv[1])
    for mode in ("OFF", "ON"):
        path = directory / f"ctest-cost-{mode}.txt"
        data = path.read_text(encoding="utf-8") if path.exists() else ""
        if not data:
            print(f"Missing Tier 2 timing data for {mode}: {path}", file=sys.stderr)
            return 1
        count = measured(data)
        if count == 0:
            print(f"No measured Tier 2 tests for {mode}: {path}", file=sys.stderr)
            return 1
        print(f"{mode}: {count} measured tests ({len(data.encode())} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
