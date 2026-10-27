#!/usr/bin/env -S uv run --script
"""Run astyle over the vendored libcaption sources, the port of format.sh (#962):

    uv run --no-project deps/libcaption/format.py

The shell script `cd`d to its own directory and ran astyle once per file that
`find .` matched, with a fixed argument list. This keeps the invocation token for
token (the same flags, the same `find` suffixes, astyle's working directory the
script's own) so a developer running it gets the tree formatted exactly as
before. It is a developer formatter with no caller in the build, so the port
needs no interpreter in a container image.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

# The flags on the shell's single `-exec astyle ... {} \;` line, in order.
ASTYLE_ARGS = [
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
]

# The `-name` predicates: `find . \( -name '*.cpp' -o -name '*.c' -o
# -name '*.h' -o -name '*.hpp' -o -name '*.re2c' \)`.
SUFFIXES = frozenset({".cpp", ".c", ".h", ".hpp", ".re2c"})


def collect_sources(root: Path) -> list[str]:
    """Every matching file below ``root`` as the shell's ``find .`` named it.

    ``find .`` printed ``./<path>`` for each match, recursing into subdirectories
    and skipping everything whose suffix is not in :data:`SUFFIXES`.
    """
    found: list[str] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for name in sorted(filenames):
            if Path(name).suffix in SUFFIXES:
                rel = Path(dirpath, name).relative_to(root)
                found.append("./" + rel.as_posix())
    return found


def run(root: Path, astyle: str = "astyle") -> int:
    """Format every source below ``root``; returns the first non-zero exit code.

    The shell let ``find`` return non-zero when astyle failed, so a failure is
    propagated rather than swallowed.
    """
    code = 0
    for source in collect_sources(root):
        result = subprocess.run([astyle, *ASTYLE_ARGS, source], cwd=root, check=False)
        if result.returncode != 0 and code == 0:
            code = result.returncode
    return code


def main(argv: list[str] | None = None) -> int:
    del argv  # the shell script took no arguments; it didn't even look at "$@"
    return run(Path(__file__).resolve().parent)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
