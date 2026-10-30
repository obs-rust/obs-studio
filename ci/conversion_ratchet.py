#!/usr/bin/env -S uv run --script
"""The CMake targets that state a conversion group at their call sites (#2625, #2923).

    uv run python -m unittest ci/test_conversion_ratchet.py

`cmake/linux/compilerconfig.cmake` pins `-Wno-conversion`, `-Wno-sign-conversion`
and `-Wno-float-conversion` globally; a target drops a pin by naming the group in
its own `target_compile_options()`. Those targets are the conversion ratchet, and
its blind spot is the configuration: the group is checked where the lane that
compiles it last ran, which for a plugin target is a PR's Tier 2 or a platform
lane, never the sanitizer build. Three ratchet slices reached master that way
(`rist_sender_create` #2869/#2875, the Cocoa seams #2900, eight sites in #2923).

`ratcheted_dirs` is the ratchet as `ci/ci_scope.py` reads it: the directories of
the CMakeLists files that name a group. `hit` says whether a changed path is a
compile input below one of them (a source, or the CMakeLists that names the
group), so the PR that touches a ratcheted target selects the sanitizer lane and
pays for its own diagnostic (#2923) instead of leaving it to the night's
schedule.

The list is read from the tree the PR builds, never a hand-kept copy, so the PR
that adds a target to the ratchet selects the lane too: the flag is already in
its own CMakeLists by the time the scope runs.
"""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

# A target opts in by naming a group; the pins use the `-Wno-` spelling, so only
# the positive form counts. The three names are the global pins' groups.
GROUP = re.compile(r"(?<![\w-])-W(?:no-)?[a-z-]*conversion\b")
CALL = re.compile(r"target_compile_options\(([^)]*)\)", re.S)
# Compile inputs keep their suffix in the unity outputs (foo.cpp.hpp, bar.c.h).
SOURCE = re.compile(r"\.(?:c|h|cc|cpp|cxx|hpp|hxx|ipp|m|mm)$")
# A target's own build file: the PR that opts it into a group changes this and
# nothing else (#2956), and it is a build input of that target.
CMAKE_LISTS = re.compile(r"^(?:(?P<dir>.+)/)?CMakeLists\.txt$")
# Vendored upstream trees are never edited here, so they never ratchet.
VENDORED = ("deps/",)


def groups(body: str) -> set[str]:
    """The positive conversion groups one `target_compile_options` body names.

    A `target_compile_options(<target> PRIVATE -Wno-conversion)` keeps a pin and
    is not a ratchet entry: the pins are per-target exceptions, the ratchet is
    the target that states the conversion at its call sites.
    """
    return {flag for flag in GROUP.findall(body) if not flag.startswith("-Wno-")}


def ratcheted_dirs(lists: dict[str, str]) -> tuple[str, ...]:
    """The directories of the CMakeLists that opt a target into a group.

    `lists` maps a posix path (`frontend/CMakeLists.txt`) to its text. The
    CMakeLists's directory is what a changed source is matched against, the
    same way CMake adds a target's sources.
    """
    dirs = set()
    for path, text in lists.items():
        for body in CALL.findall(text):
            if groups(body):
                parent = Path(path).parent.as_posix()
                dirs.add("" if parent == "." else parent)
                break
    return tuple(sorted(dirs))


def repo_cmake_lists(root: Path) -> dict[str, str]:
    """Every tracked CMakeLists below `root`, vendored trees excluded."""
    try:
        listing = subprocess.run(
            ["git", "ls-files", "-z", "*CMakeLists.txt"],
            cwd=root,
            check=True,
            capture_output=True,
        ).stdout.decode()
    except (OSError, subprocess.CalledProcessError):
        listing = "\n".join(p.as_posix() for p in sorted(root.rglob("CMakeLists.txt")))
    else:
        listing = listing.replace("\0", "\n")
    lists: dict[str, str] = {}
    for path in listing.splitlines():
        if not path or path.startswith(VENDORED):
            continue
        try:
            lists[path] = (root / path).read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
    return lists


def repo_ratcheted_dirs(root: Path) -> tuple[str, ...]:
    return ratcheted_dirs(repo_cmake_lists(root))


def hit(path: str, dirs: tuple[str, ...] | frozenset[str]) -> bool:
    """True when `path` is a compile input of a ratcheted target.

    A source below a ratcheted directory is one (#2923). So is that target's own
    `CMakeLists.txt`: the PR that opts a target into a group changes only the
    CMakeLists, and the sanitizer-only diagnostic the new flags can surface is
    one only that PR's lane would report (#2956 left it to the night's
    schedule). A nested CMakeLists is another target, and a directory the
    ratchet does not name opts nothing in.
    """
    if SOURCE.search(path):
        return any(path.startswith(d + "/") for d in dirs if d)
    match = CMAKE_LISTS.match(path)
    if match is None:
        return False
    return (match.group("dir") or "") in dirs


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    for directory in repo_ratcheted_dirs(root):
        print(directory)
    return 0


# A library that states its group as an INTERFACE option hands it to every
# consumer's TUs, after the consumer's own options, so a consumer-side
# `-Wno-sign-conversion` cannot pin it (#3096). The ratchet therefore has to
# keep Darwin out of the interface itself.
INTERFACE_CALL = re.compile(
    r"target_compile_options\(\s*(?P<target>[^\s)]+)\s+INTERFACE\s+(?P<body>[^)]*)\)",
    re.S,
)
DARWIN_GUARD = "$<NOT:$<PLATFORM_ID:Darwin>>"
# The else() branch of an `if(OS_MACOS)` block: the shape that keeps Apple out of
# a positive group without relying on where a linked interface sorts.
DARWIN_BRANCH = re.compile(
    r"^if\(OS_MACOS\)$(?P<pin>.*?)^else\(\)$(?P<other>.*?)^endif\(\)$", re.M | re.S
)


def without_comments(text: str) -> str:
    """The CMake text with comment lines dropped, so a comment cannot satisfy a check."""
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


def interface_darwin_violations(lists: dict[str, str]) -> tuple[str, ...]:
    """`path: target` for every INTERFACE conversion group Apple can still reach.

    A positive group in an INTERFACE call reaches every consumer's Apple TUs, where
    Qt 6.11's headers are not converted and the lane stops on them (#3096). The
    group is exempt in two shapes: the call excludes Darwin with a
    `$<NOT:$<PLATFORM_ID:Darwin>>` genex, or it sits in the `else()` branch of an
    `if(OS_MACOS)` block whose `if()` branch states the Apple pin.
    """
    violations = []
    for path, text in lists.items():
        stripped = without_comments(text)
        exempt = [
            (m.start("other"), m.end("other")) for m in DARWIN_BRANCH.finditer(stripped)
        ]
        for call in INTERFACE_CALL.finditer(stripped):
            body = call.group("body")
            if not groups(body) or DARWIN_GUARD in body:
                continue
            if any(start <= call.start() < end for start, end in exempt):
                continue
            violations.append(f"{path}: {call.group('target')}")
    return tuple(violations)


def repo_interface_darwin_violations(root: Path) -> tuple[str, ...]:
    return interface_darwin_violations(repo_cmake_lists(root))


if __name__ == "__main__":
    raise SystemExit(main())
