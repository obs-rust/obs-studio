#!/usr/bin/env -S uv run --script
"""Prove which C sources a Tier 2 target compiles, from the Ninja graph.

    ported_proof.py --build B --src S ported OBJ...
    ported_proof.py --build B --src S compiles TARGET SOURCE OBJECT

`ported`: every OBJ (a path below libobs/, as source-sets.d names it, e.g.
util/bmem.c.o) must be compiled into libobs-c-reference and not into hybrid
libobs. Prints the first violation as validate.sh always has and exits 1.

`compiles`: exit 0 when TARGET compiles SOURCE (a path below the source
tree, or `-`) as OBJECT, else 1.

A target compiles a source when any of these holds:
- Ninja lists `<target>.dir/<object>` among the target's inputs;
- one of the target's unity groups (BUILD.md, `build/<name>+.h.c`) includes
  `"<source>"`;
- one of CMake's Unity batches for the target includes a path ending in
  `/<source>`.

It queries the current Ninja graph and generated batches, not stale objects
on a persistent build dir. Each target's inputs, groups and batches are read
once. The shell loop this replaces spawned a grep per object per group and
took 176 s for 469 objects on the Dell (#4954).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Optional

INCLUDE = re.compile(r'^#include "(.*)"$', re.MULTILINE)


@dataclass
class TargetLists:
    inputs: str
    groups: list[str]
    unity: list[str]


def read_lists(build: Path, target: str) -> TargetLists:
    try:
        inputs = subprocess.run(
            ["ninja", "-C", str(build), "-t", "inputs", target],
            capture_output=True, text=True, check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError):
        inputs = ""
    group = re.compile(rf".*CMakeFiles/{re.escape(target)}\.dir/(build/.*\+\.h\.[cm])\.o$")
    groups = [m.group(1) for line in inputs.splitlines() if (m := group.match(line))]
    unity: list[str] = []
    batches = sorted(p for p in build.rglob("Unity")
                     if p.is_dir() and p.parent.name == f"{target}.dir"
                     and p.parent.parent.name == "CMakeFiles")
    if batches:
        for batch in sorted(batches[0].glob("*.c")):
            unity += INCLUDE.findall(batch.read_text(errors="replace"))
    return TargetLists(inputs, groups, unity)


def bre_literal(text: str) -> str:
    """`text` as grep's basic regex reads it: only `.` is special in a path."""
    return "".join("." if c == "." else re.escape(c) for c in text)


def compiled(lists: TargetLists, target: str, src: str, obj: str,
             read_group: Callable[[str], Optional[str]]) -> bool:
    if f"{target}.dir/{obj}" in lists.inputs:
        return True
    quoted = f'"{src}"'
    for group in lists.groups:
        text = read_group(group)
        if text is not None and quoted in text:
            return True
    tail = re.compile("/" + bre_literal(src) + "$")
    return any(tail.search(line) for line in lists.unity)


class Graph:
    def __init__(self, build: Path, src: Path):
        self.build, self.src = build, src
        self.lists: dict[str, TargetLists] = {}
        self.groups: dict[str, Optional[str]] = {}

    def read_group(self, group: str) -> Optional[str]:
        if group not in self.groups:
            try:
                self.groups[group] = (self.src / "libobs" / group).read_text(errors="replace")
            except OSError:
                self.groups[group] = None
        return self.groups[group]

    def compiles(self, target: str, src: str, obj: str) -> bool:
        if target not in self.lists:
            self.lists[target] = read_lists(self.build, target)
        return compiled(self.lists[target], target, src, obj, self.read_group)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--src", type=Path, required=True)
    sub = parser.add_subparsers(dest="cmd", required=True)
    ported = sub.add_parser("ported")
    ported.add_argument("objects", nargs="*")
    one = sub.add_parser("compiles")
    one.add_argument("target")
    one.add_argument("source")
    one.add_argument("object")
    args = parser.parse_args(argv)
    graph = Graph(args.build, args.src)
    if args.cmd == "compiles":
        return 0 if graph.compiles(args.target, args.source, args.object) else 1
    for obj in args.objects:
        src = f"libobs/{obj.removesuffix('.o')}"
        if graph.compiles("libobs", src, obj):
            print(f"FAIL: {obj} was compiled into hybrid libobs", flush=True)
            return 1
        if not graph.compiles("libobs-c-reference", src, obj):
            print(f"FAIL: {obj} was not compiled into libobs-c-reference", flush=True)
            return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
