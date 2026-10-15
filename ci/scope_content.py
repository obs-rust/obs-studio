"""Content-aware platform scoping for ci_scope (obs-rust/obs-studio#698).

A path alone cannot say that a hunk in a shared C file sits inside
`#ifdef __APPLE__`. This module reads `git diff -U0` text and, for a changed
C/C++/ObjC/CMake file, answers which of macOS and Windows its hunks touch:

- a changed line that itself names a platform macro touches that platform;
- otherwise the new file is scanned up to the hunk, keeping a stack of open
  `#if`/`if()` blocks; a block that names a platform (or its negation, or its
  `#else`) makes the hunk touch the platforms that branch can select;
- anything undecidable touches nothing: Linux still covers it and the
  release gate covers the rest.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

MAC, WIN = "macos", "windows"

C_MAC = re.compile(r"\b(?:__APPLE__|TARGET_OS_\w+|OS_MACOS)\b")
C_WIN = re.compile(r"\b(?:_WIN32|_WIN64|WIN32|_MSC_VER|OS_WINDOWS)\b")
CMAKE_MAC = re.compile(r"\b(?:OS_MACOS|APPLE)\b|PLATFORM_ID:[^>]*\bDarwin\b")
CMAKE_WIN = re.compile(r"\b(?:OS_WINDOWS|WIN32|MSVC)\b|PLATFORM_ID:[^>]*\bWindows\b")
# Neither macOS nor Windows: only matters for the branches that exclude it
# (`#else` of `#ifdef __linux__`, `#ifndef __linux__`), which are theirs.
NIX = "nix"
C_NIX = re.compile(r"\b(?:__linux__|__linux|__unix__|__FreeBSD__|__DragonFly__|__OpenBSD__|__NetBSD__|OS_LINUX|OS_FREEBSD|OS_POSIX)\b")
CMAKE_NIX = re.compile(r"\b(?:OS_LINUX|OS_FREEBSD|OS_OPENBSD|OS_POSIX)\b|PLATFORM_ID:[^>]*\b(?:Linux|FreeBSD)\b")
ALL = frozenset((MAC, WIN, NIX))

C_SUFFIXES = (".c", ".h", ".cpp", ".hpp", ".cc", ".cxx", ".hh", ".m", ".mm")
C_DIRECTIVE = re.compile(r"^\s*#\s*(ifdef|ifndef|if|elif|else|endif)\b(.*)")
CMAKE_DIRECTIVE = re.compile(r"^\s*(if|elseif|else|endif)\s*\((.*)", re.I)
NEG_BEFORE = re.compile(r"(?:!\s*(?:defined\s*)?\(?\s*|\bNOT\s+\(?\s*)$", re.I)


def kind_of(path: str) -> str | None:
    if path.endswith(C_SUFFIXES):
        return "c"
    if path.endswith(".cmake") or path.rsplit("/", 1)[-1] == "CMakeLists.txt":
        return "cmake"
    return None


def _patterns(kind: str):
    return (C_MAC, C_WIN, C_NIX) if kind == "c" else (CMAKE_MAC, CMAKE_WIN, CMAKE_NIX)


def mentions(text: str, kind: str) -> set[str]:
    mac, win, _ = _patterns(kind)
    found = set()
    if mac.search(text):
        found.add(MAC)
    if win.search(text):
        found.add(WIN)
    return found


def selected(cond: str, kind: str, negated: bool = False) -> set[str]:
    """Platforms a branch with this condition can select."""
    out: set[str] = set()
    for platform, pattern in zip((MAC, WIN, NIX), _patterns(kind)):
        for m in pattern.finditer(cond):
            neg = negated or bool(NEG_BEFORE.search(cond[: m.start()]))
            out |= ALL - {platform} if neg else {platform}
    return out


def _complement(platforms: set[str]) -> set[str]:
    """What an `#else` selects after branches that named `platforms`."""
    return set(ALL - platforms) if platforms else set()


@dataclass
class _Block:
    prior: set[str] = field(default_factory=set)  # union of earlier branches
    current: set[str] = field(default_factory=set)


def enclosing(lines: list[str], kind: str) -> set[str]:
    """Platforms selected by the blocks still open after `lines`."""
    stack: list[_Block] = []
    directive = C_DIRECTIVE if kind == "c" else CMAKE_DIRECTIVE
    for line in lines:
        m = directive.match(line)
        if not m:
            continue
        word, rest = m.group(1).lower(), m.group(2)
        if word in ("if", "ifdef", "ifndef"):
            stack.append(_Block(current=selected(rest, kind, word == "ifndef")))
        elif word in ("else", "elif", "elseif") and stack:
            block = stack[-1]
            block.prior |= block.current
            block.current = _complement(block.prior)
            if word != "else":
                block.current |= selected(rest, kind)
        elif word == "endif" and stack:
            stack.pop()
    out: set[str] = set()
    for block in stack:
        out |= block.current
    return out - {NIX}


@dataclass
class Hunk:
    new_start: int
    new_len: int
    changed: list[str]


def parse_diff(diff: str) -> dict[str, list[Hunk]]:
    """`git diff -U0` text -> {path: hunks}. Deleted files key on the old path."""
    files: dict[str, list[Hunk]] = {}
    path = None
    old = None
    for line in diff.splitlines():
        if line.startswith("--- "):
            old = line[4:].removeprefix("a/") if line != "--- /dev/null" else None
        elif line.startswith("+++ "):
            new = line[4:].removeprefix("b/") if line != "+++ /dev/null" else None
            path = new or old
            if path:
                files.setdefault(path, [])
        elif line.startswith("@@") and path:
            m = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@", line)
            if m:
                files[path].append(Hunk(int(m.group(1)), int(m.group(2) or 1), []))
        elif path and files[path] and line[:1] in "+-":
            files[path][-1].changed.append(line[1:])
    return files


def touched(path: str, hunks: list[Hunk], new_text: str | None) -> set[str]:
    """Platforms a shared file's hunks touch ({} when undecidable)."""
    kind = kind_of(path)
    if kind is None:
        return set()
    out: set[str] = set()
    lines = new_text.splitlines() if new_text is not None else None
    for hunk in hunks:
        out |= mentions("\n".join(hunk.changed), kind)
        if lines is not None and out != {MAC, WIN}:
            # -U0: new_len 0 means the change sits after line new_start.
            before = hunk.new_start if hunk.new_len == 0 else hunk.new_start - 1
            out |= enclosing(lines[:before], kind)
    return out
