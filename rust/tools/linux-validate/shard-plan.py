#!/usr/bin/env -S uv run --script
"""Split the Tier 2 tests into balanced shards by recorded duration (#1048).

    uv run --script shard-plan.py COST_FILE NAMES_FILE SHARDS OUT_DIR

Splits the tests in NAMES_FILE (one per line, from `ctest -N`) into SHARDS
balanced sets using ctest's own per-test durations (COST_FILE, the
CTestCostData.txt format: "name runs avg-seconds" lines, then "---" and failed
tests). Longest test first, each into the lightest shard; a test with no
recorded cost counts as DEFAULT_COST seconds (default 1). Writes
OUT_DIR/<i>-of-<SHARDS>.regex, an exact-name regex for `ctest -R`, and prints
each shard's load.

Replaces `ctest -I <i>,,<n>` striping, which ignored durations (#1048).
Tested by ci/test_tier2_shard_plan.py.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path


def number(text: str) -> float:
    """A recorded cost, or 0 for the junk a half-written file can hold."""
    try:
        return float(text)
    except ValueError:
        return 0.0


def costs(path: Path) -> dict[str, float]:
    """The average seconds of every test above the `---` failure separator."""
    recorded: dict[str, float] = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        fields = line.split()
        if line == "---":
            break
        if len(fields) >= 3:
            recorded[fields[0]] = number(fields[2])
    return recorded


def ordered(names_file: Path, recorded: dict[str, float], default: float) -> list[tuple[str, float]]:
    """Every listed test with its cost, longest first, then by name."""
    tests = []
    for line in names_file.read_text(encoding="utf-8", errors="replace").splitlines():
        fields = line.split()
        if fields:
            tests.append((fields[0], recorded.get(fields[0], default)))
    tests.sort(key=lambda test: (-test[1], test[0]))
    return tests


def plan(tests: list[tuple[str, float]], shards: int) -> tuple[list[list[str]], list[float]]:
    """Longest first, each into the lightest shard (lowest index on a tie)."""
    assigned: list[list[str]] = [[] for _ in range(shards)]
    loads = [0.0] * shards
    for name, cost in tests:
        lightest = min(range(shards), key=lambda index: (loads[index], index))
        assigned[lightest].append(name)
        loads[lightest] += cost
    return assigned, loads


def write(assigned: list[list[str]], loads: list[float], out: Path) -> None:
    """One exact-name regex per shard, then the load it carries."""
    out.mkdir(parents=True, exist_ok=True)
    shards = len(assigned)
    for index, (names, load) in enumerate(zip(assigned, loads), 1):
        (out / f"{index}-of-{shards}.regex").write_text(f"^({'|'.join(names)})$\n", encoding="utf-8")
        print(f"shard {index}/{shards}: {load:.1f} s")


def main(argv: list[str]) -> int:
    if len(argv) != 5:
        print("usage: shard-plan.py COST_FILE NAMES_FILE SHARDS OUT_DIR", file=sys.stderr)
        return 2
    cost_file, names_file, shards, out = Path(argv[1]), Path(argv[2]), int(argv[3]), Path(argv[4])
    default = number(os.environ.get("DEFAULT_COST", "1"))
    assigned, loads = plan(ordered(names_file, costs(cost_file), default), max(shards, 1))
    write(assigned, loads, out)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
