#!/usr/bin/env -S uv run --script
"""Print why a Tier 2 build failed (obs-rust/obs-studio#962 port):

    uv run --no-project rust/tools/linux-validate/build_errors.py BUILD_LOG

C/C++ compiler errors and ninja FAILED lines (deduplicated), zccache cache
infrastructure reasons, then rustc errors with their source location and
build-script panics. cargo's messages ("error[E0308]: ...", "error: ...", a
build script's panic) don't match the C filter, so a failed Rust build once
showed only "FAILED: [code=101] ..." and no reason (#814). Tested by
ci/test_tier2_build_errors.py.

Ported off the 36-line shell script in #962; validate.sh runs it through
`uv run --no-project`, the same way it runs the other ported helpers of that
lane.
"""

from __future__ import annotations

import re
import sys
from collections.abc import Iterable
from pathlib import Path

# Cache infrastructure failures are not compiler errors. Print their reasons
# separately so hundreds of FAILED edges cannot crowd them out of the limit.
ZCCACHE = re.compile(r"^zccache(\[|:)")
# `grep -E ": (fatal )?error|FAILED:"`.
C_ERROR = re.compile(r": (fatal )?error|FAILED:")
# The awk rule: a rustc error line or build-script panic opens a block. It is
# printed with its "--> " location and code excerpt, and ends at a blank line,
# the next FAILED:, ordinary build output, a warning, a progress line, or after
# BLOCK_LINES lines, whichever comes first.
RUSTC_START = re.compile(r"^error(\[E[0-9]+\])?: |panicked at ")
RUSTC_END = re.compile(r"^$|^FAILED:|^ *(Compiling|Checking|Finished) |^warning: |^\[[0-9]+/[0-9]+\] ")
BLOCK_LINES = 12


def rustc_blocks(lines: Iterable[str]) -> list[str]:
    """The rustc error and build-script panic blocks of a build log, in order:
    each opening line followed by up to BLOCK_LINES lines of context."""
    kept: list[str] = []
    show = False
    n = 0
    for line in lines:
        if RUSTC_START.search(line):
            show = True
            n = 0
            kept.append(line)
            continue
        if not show:
            continue
        if RUSTC_END.search(line):
            show = False
            continue
        kept.append(line)
        n += 1
        if n >= BLOCK_LINES:
            show = False
    return kept


def report(lines: list[str]) -> list[str]:
    """The lines a failed build should show, deduplicated and limited, in the
    shell script's order: zccache reasons, C/C++ errors, rustc blocks."""
    out = sorted({line for line in lines if ZCCACHE.search(line)})[:40]
    out += sorted({line for line in lines if C_ERROR.search(line)})[:80]
    out += rustc_blocks(lines)[:200]
    return out


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        sys.stderr.write("usage: build_errors.py BUILD_LOG\n")
        return 2
    lines = Path(argv[1]).read_text(encoding="utf-8", errors="replace").splitlines()
    for line in report(lines):
        print(line)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
