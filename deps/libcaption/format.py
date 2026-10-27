#!/usr/bin/env -S uv run --script
"""Reformat the libcaption sources with astyle (obs-rust/obs-studio#962, GEN-005).

    uv run deps/libcaption/format.py

Ported off the 3-line shell script `format.sh`. It is the developer helper that
runs astyle over the vendored libcaption tree; it has no CI caller, so nothing
else in the tree changes when it moves.

The shell ran, from its own directory,

    find . \\( -name '*.cpp' -o -name '*.c' -o -name '*.h' -o -name '*.hpp' \\
        -o -name '*.re2c' \\) -exec astyle <options> {} \\;

which is one astyle invocation per matching source. This keeps that shape: the
same suffix set, the same astyle options token for token, one run per source,
and the working directory set to this directory so the `./`-relative paths
resolve as the shell's `cd "$(dirname "$0")"` did. The targets are sorted so the
run order is deterministic.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

# The shell's astyle invocation, kept verbatim.
ASTYLE = "astyle"
STYLE = (
    "--style=stroustrup",
    "--attach-extern-c",
    "--break-blocks",
    "--pad-header",
    "--pad-paren-out",
    "--unpad-paren",
    "--add-brackets",
    "--keep-one-line-blocks",
    "--keep-one-line-statements",
    "--convert-tabs",
    "--align-pointer=type",
    "--align-reference=type",
    "--suffix=none",
    "--lineend=linux",
    "--max-code-length=180",
)
SUFFIXES = (".cpp", ".c", ".h", ".hpp", ".re2c")


def targets(root: Path) -> list[str]:
    """The `./`-relative sources the shell's `find` handed to astyle, sorted."""
    found = (
        path for path in root.rglob("*") if path.is_file() and path.suffix in SUFFIXES
    )
    return sorted(f"./{path.relative_to(root).as_posix()}" for path in found)


def argv(source: str) -> list[str]:
    """The astyle command line the shell built for one source."""
    return [ASTYLE, *STYLE, source]


def main() -> int:
    """Run astyle on every libcaption source from this directory."""
    for source in targets(HERE):
        result = subprocess.run(argv(source), cwd=HERE, check=False)
        if result.returncode != 0:
            return result.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
