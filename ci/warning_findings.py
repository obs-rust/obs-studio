#!/usr/bin/env -S uv run --script
"""Replay C translation units and report the warnings grouped by directory (#2689).

    uv run ci/warning_findings.py --compdb /tmp/compdb.json \
        --drop -Wno-float-conversion --drop -Wno-sign-conversion

A warning class is reviewable as a whole: fixing one call-site-wide class in a
per-directory series costs one full `Pull` run (~35 min) per directory and
re-invalidates the compile cache on every slice (#2689). This helper is the
cheap measurement that makes one PR per warning class practical.

It reads the Tier 2 build tree's `compile_commands.json`:

    docker run --rm -v obs-rust-linux-validate-build-<id>:/build <image> \
        sh -c 'cd /build/tree && ninja -t compdb' > /tmp/compdb.json

then, for each selected translation unit, strips the compiler-cache wrapper,
drops the `-Wno-*` flags named with `--drop`, removes the object output
(`-o`, `-MF`, `-MT`, `-MQ`, `-MD`, `-MMD`, `-Werror`, `-c`), appends
`-fsyntax-only` and runs it. Nothing is linked and no object is written, so
the whole class is measured in minutes.

Exit code 1 when a finding is reported (usable as RED), 0 when the replay is
clean.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import subprocess
import sys
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# `file:line:col: warning: message [-Woption]`, from GCC or Clang. A `-Werror`
# finding prints as `error:` under the same option, so both are kept.
DONE = ("warning", "error")
DIAGNOSTIC = re.compile(
    r"^(?P<file>.+?):(?P<line>\d+):(?:(?P<col>\d+):)?\s*"
    r"(?P<severity>warning|error|note):\s+(?P<message>.*?)"
    r"(?:\s+\[(?P<option>-W[^\]]+)\])?$"
)
# Compiler-cache and launcher wrappers the Tier 2 tree prefixes a command with.
WRAPPERS = frozenset({"zccache", "ccache", "sccache", "distcc", "time", "env"})
# Their argument is dropped with them, since no object is written.
OUTPUT_FLAGS = frozenset({"-o", "-MF", "-MT", "-MQ"})
# No object file, no dependency file, and no early failure: enumerate instead.
NO_OUTPUT_FLAGS = frozenset({"-c", "-MD", "-MMD", "-Werror", "-fsyntax-only"})
SOURCE = re.compile(r"\.(?:c|m)$")
# Their values are flag-shaped (`-Wno-float-conversion`), so they need `=` to parse.
DASH_VALUE_OPTIONS = ("--drop", "--select")


@dataclass(frozen=True)
class Finding:
    """One compiler diagnostic, with the option that produced it."""

    file: str
    line: int
    severity: str
    message: str
    option: str

    def directory(self, root: Path) -> str:
        """The source's directory relative to the repository root."""
        path = Path(self.file)
        try:
            return str(path.resolve().parent.relative_to(root))
        except ValueError:
            return str(path.parent)


@dataclass
class Replay:
    """The result of one `-fsyntax-only` replay."""

    file: str
    returncode: int
    findings: list[Finding] = field(default_factory=list)
    stderr: str = ""


@dataclass
class Row:
    """The findings of one directory, for the table."""

    directory: str
    warnings: int
    units: int
    options: Counter[str]


def command(entry: dict) -> list[str]:
    """The entry's command as an argv list (`arguments` when the tree has it)."""
    if "arguments" in entry:
        return [str(arg) for arg in entry["arguments"]]
    return shlex.split(entry["command"])


def strip_wrappers(argv: list[str]) -> list[str]:
    """Drop a leading `zccache`/`ccache`/`time`/`env` wrapper and its operands."""
    args = list(argv)
    while args and Path(args[0]).name in WRAPPERS:
        args.pop(0)
        while args and (args[0].startswith("-") or "=" in args[0]):
            args.pop(0)
    return args


def replay_argv(argv: list[str], drop: frozenset[str]) -> list[str]:
    """The same compile with `drop` removed, no output, syntax check only."""
    replay: list[str] = []
    skip_next = False
    for arg in argv:
        if skip_next:
            skip_next = False
            continue
        if arg in OUTPUT_FLAGS:
            skip_next = True
            continue
        if arg in NO_OUTPUT_FLAGS or arg.split("=")[0] in drop or arg in drop:
            continue
        replay.append(arg)
    replay.append("-fsyntax-only")
    return replay


def parse_diagnostics(stderr: str) -> list[Finding]:
    """Every `warning:`/`error:` line of a compiler's diagnostics."""
    findings = []
    for line in stderr.splitlines():
        match = DIAGNOSTIC.match(line.strip())
        if match and match.group("severity") in DONE:
            findings.append(
                Finding(
                    file=match.group("file"),
                    line=int(match.group("line")),
                    severity=match.group("severity"),
                    message=match.group("message"),
                    option=match.group("option") or "",
                )
            )
    return findings


def selects(entry: dict, patterns: list[str]) -> bool:
    """True when the entry is C and matches one of `patterns` (all, when none)."""
    path = Path(entry["file"])
    if not SOURCE.search(path.name):
        return False
    if not patterns:
        return True
    return any(path.match(pattern) or str(path).endswith(pattern) for pattern in patterns)


def replay(entry: dict, drop: frozenset[str], timeout: float) -> Replay:
    """Run one translation unit with `-fsyntax-only` and collect its diagnostics."""
    argv = replay_argv(strip_wrappers(command(entry)), drop)
    try:
        proc = subprocess.run(
            argv,
            cwd=entry["directory"],
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        return Replay(file=entry["file"], returncode=-1, stderr=str(error))
    return Replay(
        file=entry["file"],
        returncode=proc.returncode,
        findings=parse_diagnostics(proc.stderr),
        stderr=proc.stderr,
    )


def rows_by_directory(replays: list[Replay], root: Path) -> list[Row]:
    """Group the findings by directory, most findings first."""
    found: dict[str, list[Finding]] = {}
    units: Counter[str] = Counter()
    for result in replays:
        for finding in result.findings:
            found.setdefault(finding.directory(root), []).append(finding)
        for directory in {finding.directory(root) for finding in result.findings}:
            units[directory] += 1
    rows = [
        Row(
            directory=directory,
            warnings=len(findings),
            units=units[directory],
            options=Counter(finding.option for finding in findings if finding.option),
        )
        for directory, findings in found.items()
    ]
    rows.sort(key=lambda row: (-row.warnings, row.directory))
    return rows


def report(rows: list[Row], replays: list[Replay], root: Path) -> dict:
    """The machine-readable report `--json` writes and the tests read."""
    return {
        "total": sum(row.warnings for row in rows),
        "translation_units": len(replays),
        "directories": [
            {
                "directory": row.directory,
                "warnings": row.warnings,
                "translation_units": row.units,
                "options": dict(row.options.most_common()),
            }
            for row in rows
        ],
        "findings": [
            {
                "file": finding.file,
                "line": finding.line,
                "severity": finding.severity,
                "option": finding.option,
                "message": finding.message,
            }
            for result in replays
            for finding in result.findings
        ],
    }


def print_table(rows: list[Row], total: int, units: int, root: Path) -> None:
    """The human table: one row per directory, options by count."""
    print(f"{total} finding(s) in {units} translation unit(s)\n")
    if not rows:
        return
    width = max(len("directory"), *(len(row.directory) + 2 for row in rows))
    print(f"{'directory'.ljust(width)}  findings  tus  options")
    for row in rows:
        options = ", ".join(f"{name} ({count})" for name, count in row.options.most_common(3))
        print(f"{row.directory.ljust(width)}  {row.warnings:8}  {row.units:3}  {options}")
    print(f"\nrepository root: {root}")


def normalize_values(argv: list[str]) -> list[str]:
    """`--drop -Wno-x` becomes `--drop=-Wno-x`, since argparse reads it as an option."""
    normalized: list[str] = []
    index = 0
    while index < len(argv):
        arg = argv[index]
        index += 1
        if arg in DASH_VALUE_OPTIONS and index < len(argv):
            normalized.append(f"{arg}={argv[index]}")
            index += 1
        else:
            normalized.append(arg)
    return normalized


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--compdb", required=True, type=Path, help="compile_commands.json")
    parser.add_argument("--root", type=Path, default=ROOT, help="repository root (directory grouping)")
    parser.add_argument(
        "--drop",
        action="append",
        default=[],
        metavar="FLAG",
        help="a -Wno-* to remove from the replay (repeatable, e.g. -Wno-float-conversion)",
    )
    parser.add_argument(
        "--select",
        action="append",
        default=[],
        metavar="GLOB",
        help="only replay sources matching this glob (repeatable; default: every C source)",
    )
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 4, help="parallel replays")
    parser.add_argument("--timeout", type=float, default=300.0, help="seconds per translation unit")
    parser.add_argument("--json", action="store_true", help="write JSON instead of the table")
    parser.add_argument("--out", type=Path, help="write the JSON report to this file")
    parser.add_argument("--limit", type=int, default=20, help="findings listed in the table")
    return parser.parse_args(normalize_values(argv))


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    entries = json.loads(args.compdb.read_text(encoding="utf-8"))
    selected = [entry for entry in entries if selects(entry, args.select)]
    if not selected:
        print(f"no C translation unit matches {args.select or 'anything'}", file=sys.stderr)
        return 2
    drop = frozenset(args.drop)
    root = args.root.resolve()
    with ThreadPoolExecutor(max_workers=max(1, args.jobs)) as pool:
        replays = list(pool.map(lambda entry: replay(entry, drop, args.timeout), selected))
    rows = rows_by_directory(replays, root)
    payload = report(rows, replays, root)
    if args.json or args.out:
        text = json.dumps(payload, indent=2)
        if args.out:
            args.out.write_text(text + "\n", encoding="utf-8")
        if args.json:
            print(text)
    if not args.json:
        print_table(rows, payload["total"], payload["translation_units"], root)
        for result in replays:
            for finding in result.findings[: args.limit]:
                print(f"{finding.file}:{finding.line}: {finding.option or finding.severity}")
    return 1 if payload["total"] else 0


if __name__ == "__main__":
    sys.exit(main())
