#!/usr/bin/env -S uv run --script
"""Print a ctest regex matching exactly the RUN_SERIAL tests of a registry.

Usage: serial-tests.py REGISTRY.json [I/N] [--companions FILE]
(REGISTRY is ctest --show-only=json-v1 output)

With I/N, only every Nth of the sorted tests, starting at the Ith: CI runs the
serial tests on N runners (#2072).

With --companions (serial-companions.json), the regex also matches the suite
tests that file gives the serial runners (#3021): shard I's list with I/N, all
of them without. The suite part excludes that full set, so every test runs
once. ctest still runs a RUN_SERIAL test alone. Names missing from the
registry are skipped.

Tier 2 runs these tests on their own runner and the rest in parallel on
another (#2071). Fails when there are no RUN_SERIAL tests: an empty serial
part would pass without running anything.
"""

import argparse
import json
import re
import sys


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("registry")
    parser.add_argument("shard", nargs="?")
    parser.add_argument("--companions")
    args = parser.parse_args()
    tests = json.load(open(args.registry, encoding="utf-8"))["tests"]
    present = {test["name"] for test in tests}
    names = sorted(test["name"] for test in tests
                   if any(p["name"] == "RUN_SERIAL" and p["value"] for p in test.get("properties", [])))
    if not names:
        sys.exit("FAIL: no RUN_SERIAL tests in " + args.registry)
    groups = []
    if args.companions:
        layout = json.load(open(args.companions, encoding="utf-8"))
        groups = layout["companions"]
        if len(groups) != layout["shards"]:
            sys.exit(f"FAIL: {args.companions} lists {len(groups)} shards, not {layout['shards']}")
    extra = [name for group in groups for name in group]
    if args.shard:
        index, count = (int(n) for n in args.shard.split("/"))
        if not 1 <= index <= count:
            sys.exit("FAIL: shard must be I/N with 1 <= I <= N, got " + args.shard)
        if groups and count != len(groups):
            sys.exit(f"FAIL: {args.companions} is laid out for {len(groups)} shards, not {count}")
        names = names[index - 1::count]
        extra = groups[index - 1] if groups else []
        if not names:
            sys.exit("FAIL: no RUN_SERIAL tests in shard " + args.shard)
    names = sorted(set(names) | {name for name in extra if name in present})
    print("^(" + "|".join(re.escape(name) for name in names) + ")$")


if __name__ == "__main__":
    main()
