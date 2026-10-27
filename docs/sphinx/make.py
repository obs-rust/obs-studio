#!/usr/bin/env -S uv run --script
"""Build the Sphinx documentation (#962).

    uv run docs/sphinx/make.py [target]

Ported off the 36-line batch file `make.bat`. It is the Windows counterpart of
the `Makefile` beside it and has no caller in the tree, so nothing else changes
when it moves.

The batch file's one line is kept token for token:

    %SPHINXBUILD% -M %1 %SOURCEDIR% %BUILDDIR% %SPHINXOPTS%

`SPHINXBUILD` selects the tool (`sphinx-build` by default), `%1` is the target
and defaults to `help`, `SOURCEDIR`/`BUILDDIR` are `.`/`_build`, and everything
runs with the working directory set to this directory so those two resolve as
before. A tool that is not installed prints the same install hint and fails.
"""

from __future__ import annotations

import os
import shlex
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

DEFAULT_TOOL = "sphinx-build"
SOURCEDIR = "."
BUILDDIR = "_build"
DEFAULT_TARGET = "help"

MISSING_TOOL = (
    "The 'sphinx-build' command was not found. Make sure you have Sphinx\n"
    "installed, then set the SPHINXBUILD environment variable to point\n"
    "to the full path of the 'sphinx-build' executable. Alternatively you\n"
    "may add the Sphinx directory to PATH.\n"
    "\n"
    "If you don't have Sphinx installed, grab it from\n"
    "http://sphinx-doc.org/"
)


def tool() -> str:
    """The sphinx-build the batch file would run: `SPHINXBUILD` or the default."""
    return os.environ.get("SPHINXBUILD") or DEFAULT_TOOL


def argv(target: str) -> list[str]:
    """The sphinx-build command line, exactly as the batch file wrote it."""
    return [
        tool(),
        "-M",
        target,
        SOURCEDIR,
        BUILDDIR,
        *shlex.split(os.environ.get("SPHINXOPTS", "")),
    ]


def main(args: list[str] | None = None) -> int:
    """Run sphinx-build from this directory and return its exit status."""
    target = (sys.argv[1:] if args is None else args)[:1] or [DEFAULT_TARGET]
    try:
        return subprocess.run(argv(target[0]), cwd=HERE, check=False).returncode
    except FileNotFoundError:
        print(MISSING_TOOL, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
