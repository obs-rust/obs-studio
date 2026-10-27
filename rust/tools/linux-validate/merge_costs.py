#!/usr/bin/env -S uv run --script
"""Merge the CTestCostData.txt files the Tier 2 shards of one mode wrote (#930).

    uv run --script rust/tools/linux-validate/merge_costs.py COST_FILE... > merged

Every shard starts from the same data and bumps the run count only of the tests
it ran, so each test takes the line with the highest run count. Failed tests
are the union, in first-seen order. Output: the merged test lines sorted, then
`---`, then the failed tests.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

_SEPARATOR = "---"
_NUMBER = re.compile(r"^[-+]?(?:\d+\.?\d*|\.\d+)")


def number(text: str) -> float:
    """The leading number of a field, as awk's `$2 + 0` reads it."""
    match = _NUMBER.match(text)
    return float(match.group()) if match else 0.0


def merge(paths: list[Path]) -> str:
    runs: dict[str, float] = {}
    lines: dict[str, str] = {}
    failed: dict[str, None] = {}
    for path in paths:
        failed_part = False
        for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
            if line == _SEPARATOR:
                failed_part = True
                continue
            fields = line.split()
            if failed_part:
                if fields and fields[0] not in failed:
                    failed[fields[0]] = None
                continue
            if len(fields) >= 3:
                name, count = fields[0], number(fields[1])
                if name not in runs or count > runs[name]:
                    runs[name] = count
                    lines[name] = line
    merged = "".join(line + "\n" for line in sorted(lines.values()))
    return merged + _SEPARATOR + "\n" + "".join(name + "\n" for name in failed)


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    try:
        sys.stdout.write(merge([Path(arg) for arg in argv[1:]]))
    except OSError as error:
        print(f"merge_costs: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
