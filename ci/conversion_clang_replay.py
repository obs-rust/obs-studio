#!/usr/bin/env -S uv run --script
"""Replay a build tree's unity groups with clang's conversion diagnostics (#3118).

    uv run python -m unittest ci/test_conversion_clang_replay.py
    uv run --no-project ci/conversion_clang_replay.py --compdb build/compile_commands.json

The conversion ratchet (#2625, `ci/conversion_ratchet.py`) is checked by the
configuration that compiles the target, and every one of those on Linux is gcc.
gcc has no `-Wimplicit-int-conversion` and no
`-Wimplicit-int-conversion-on-negation`, and reports a narrower
`-Wsign-conversion` set, so a target can pass its gcc replay and still fail the
nightly's AppleClang app build (#2921, #3090, #3084 in one week).

This replays one unity group per compile entry with `clang -fsyntax-only` and
the three conversion warnings, with the global `-Wno-conversion` pins removed
exactly as the ratchet's opt-in removes them. It reads the flags from a
configured tree's `compile_commands.json`, so nothing is hand-kept: a group
whose flags change is replayed with the new flags. It builds nothing and links
nothing.

A ratcheted directory the configuration compiled no group for is reported
rather than passed silently: a target that opts in but is not in the tree's
compdb would otherwise look clean because nothing was checked.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time
from typing import TypedDict

from conversion_ratchet import repo_ratcheted_dirs

# Unity groups are the files CMake generates for a target's source group.
UNITY_SUFFIX = "+.h.c"
# The global pins the ratchet's opt-in removes (cmake/linux/compilerconfig.cmake).
PINS = frozenset({"-Wno-conversion", "-Wno-sign-conversion", "-Wno-float-conversion"})
# The group a target names to leave the ratchet.
WARNINGS = ("-Wconversion", "-Wsign-conversion", "-Wfloat-conversion")
# Clang splits gcc's -Wconversion into these; all of them are the class here.
CONVERSION_OPTIONS = frozenset(
    {
        "-Wconversion",
        "-Wsign-conversion",
        "-Wfloat-conversion",
        "-Wimplicit-int-conversion",
        "-Wimplicit-int-conversion-on-negation",
        "-Wimplicit-float-conversion",
        "-Wshorten-64-to-32",
    }
)
# Compile flags that name an output and have no meaning under -fsyntax-only.
OUTPUT_FLAGS = frozenset({"-o", "-MF", "-MT", "-MQ", "-include-pch"})
# Flags that only make sense when compiling, not when parsing.
COMPILE_ONLY_FLAGS = frozenset({"-c", "-MD", "-MMD", "-MP", "-Winvalid-pch"})
# The diagnostic's own suffix is the only reliable way to tell a conversion
# finding from a replay artifact: clang also warns about unused -L/-Wl arguments.
DIAGNOSTIC = re.compile(
    r"^(?P<path>.+?):(?P<line>\d+):(?P<column>\d+): "
    r"(?P<kind>warning|error): (?P<message>.*?)"
    r" \[(?P<option>-W[a-z0-9-]+)\]$"
)


class Replay(TypedDict):
    """One unity group's replay."""

    group: str
    directory: str
    seconds: float
    returncode: int
    diagnostics: list[str]
    failure: str


def unity_group(entry: dict[str, str]) -> bool:
    """Whether a compile entry's input is a unity group."""
    return entry["file"].endswith(UNITY_SUFFIX)


def group_directory(entry: dict[str, str], root: Path) -> str:
    """The target's directory, as `ratcheted_dirs()` spells it.

    A unity group lives at `<target dir>/build/<target>.<group>+.h.c`; the
    directory above `build/` is what the ratchet matches a changed source
    against.
    """
    relative = Path(entry["file"]).resolve().relative_to(root.resolve())
    return relative.parent.parent.as_posix()


def clang_command(entry: dict[str, str], compiler: str = "clang") -> list[str]:
    """The entry's own flags, replayed by clang with the conversion group on.

    The pins and `-Werror` are dropped so every site is reported rather than
    stopping the group at the first one, and the output flags are replaced by
    `-fsyntax-only`: nothing is written, so a replay cannot disturb a build.
    """
    tokens = shlex.split(entry["command"])
    command = [compiler]
    skip_next = False
    started = False
    for token in tokens[1:]:
        if skip_next:
            skip_next = False
            continue
        # A compiler launcher (the sanitizer tree configures zccache) sits in
        # front of the compiler path, so the first non-flag token is not ours.
        if not started and not token.startswith("-"):
            continue
        if token in OUTPUT_FLAGS:
            skip_next = True
            continue
        # Any other local pin (e.g. -Wno-implicit-fallthrough) is the target's
        # real configuration and stays.
        if token in COMPILE_ONLY_FLAGS or token in PINS or token == "-Werror":
            continue
        started = True
        command.append(token)
    command.extend(["-fsyntax-only", *WARNINGS, entry["file"]])
    return command


def conversion_diagnostics(stderr: str, root: Path) -> list[str]:
    """The conversion findings in one replay's stderr, source paths relative.

    Findings outside `root` are dropped: a warning inside a dependency's header
    is not the target's to fix.
    """
    findings: list[str] = []
    seen: set[str] = set()
    for line in stderr.splitlines():
        match = DIAGNOSTIC.match(line.strip())
        if match is None or match.group("option") not in CONVERSION_OPTIONS:
            continue
        path = Path(match.group("path"))
        if not path.is_absolute():
            continue
        try:
            relative = path.resolve().relative_to(root.resolve())
        except ValueError:
            continue
        finding = f"{relative.as_posix()}:{match.group('line')}:{match.group('column')}: {match.group('message')}"
        if finding in seen:
            # A unity group includes a source once per including TU.
            continue
        seen.add(finding)
        findings.append(finding)
    return findings


def replay(entry: dict[str, str], root: Path, compiler: str = "clang") -> Replay:
    """Compile one unity group with clang and collect its conversion findings."""
    started = time.monotonic()
    result = subprocess.run(
        clang_command(entry, compiler), cwd=root, capture_output=True, text=True
    )
    elapsed = time.monotonic() - started
    diagnostics = conversion_diagnostics(result.stderr, root)
    failure = "" if result.returncode == 0 or diagnostics else first_error(result.stderr)
    return {
        "group": Path(entry["file"]).name,
        "directory": group_directory(entry, root),
        "seconds": round(elapsed, 2),
        "returncode": result.returncode,
        "diagnostics": diagnostics,
        "failure": failure,
    }


def first_error(stderr: str) -> str:
    """The first line that explains why a group did not parse.

    A replay that dies before the sources are parsed (a missing header, a
    killed process) reports no conversion finding, and a silent pass would read
    as "this group is clean" (#3118).
    """
    for line in stderr.splitlines():
        if " error: " in line or line.startswith("clang: error:"):
            return line.strip()
    return "the replay produced no output"


def unexplained(replays: list[Replay]) -> list[Replay]:
    """Replays that failed without naming a conversion site."""
    return [item for item in replays if item["failure"]]


def uncovered(ratcheted: tuple[str, ...], directories: set[str]) -> tuple[str, ...]:
    """Ratcheted directories the configuration compiled no unity group for."""
    return tuple(directory for directory in ratcheted if directory not in directories)


def exit_status(
    findings: list[Replay],
    missing: tuple[str, ...],
    unexplained_replays: list[Replay],
    allow_uncovered: bool = False,
) -> int:
    """The replay's verdict: findings and parse failures are always red.

    `missing` is a ratcheted directory the configuration compiled no group for.
    A full app tree must cover every ratcheted target, but a lane tree does not
    build them all (the sanitizer build turns vlc, vst, webrtc and the decklink
    plugins off), so the lane passes --allow-uncovered and those directories are
    reported without failing it. The unit test keeps the strict default.
    """
    if findings or unexplained_replays:
        return 1
    if missing and not allow_uncovered:
        return 1
    return 0


def repo_compdb(compdb: Path) -> list[dict[str, str]]:
    """The compile entries of one configured tree."""
    entries: list[dict[str, str]] = json.loads(compdb.read_text())
    return entries


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Replay unity groups with clang")
    parser.add_argument(
        "--compdb",
        type=Path,
        default=None,
        help="a configured tree's compile_commands.json (default: every build*/ tree found)",
    )
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--compiler", default="clang")
    parser.add_argument(
        "--allow-uncovered",
        action="store_true",
        help="report a ratcheted directory the configuration did not build without failing",
    )
    args = parser.parse_args(argv)
    root: Path = args.root.resolve()

    if args.compdb is None:
        parser.error("--compdb is required: a replay reads a configured tree's flags")
    entries = [entry for entry in repo_compdb(args.compdb) if unity_group(entry)]
    entries.sort(key=lambda entry: entry["file"])
    ratcheted = repo_ratcheted_dirs(root)

    started = time.monotonic()
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        replays = list(pool.map(lambda entry: replay(entry, root, args.compiler), entries))
    wall = time.monotonic() - started

    findings = [item for item in replays if item["diagnostics"]]
    for item in findings:
        for finding in item["diagnostics"]:
            print(f"{finding}  [{item['directory']}]")
    for item in unexplained(replays):
        print(f"replay failed without a conversion finding: {item['group']}: {item['failure']}")
    missing = uncovered(ratcheted, {item["directory"] for item in replays})
    for directory in missing:
        print(f"no unity group compiled for ratcheted directory: {directory}")
    print(
        f"replayed {len(replays)} group(s) with {args.compiler} in {wall:.1f}s: "
        f"{len(findings)} group(s) with conversion findings, "
        f"{len(missing)} uncovered, {len(unexplained(replays))} failed to parse"
    )
    return exit_status(findings, missing, unexplained(replays), args.allow_uncovered)


if __name__ == "__main__":
    sys.exit(main())
