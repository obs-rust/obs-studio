#!/usr/bin/env -S uv run --script
"""Build the vendored w32-pthreads library with the Digital Mars Compiler (#962).

    uv run deps/w32-pthreads/builddmc.py

Ported off `builddmc.bat`: the same two `dmc` command lines, the release build
then the debug build, token for token. It has no caller -- it is the vendored
w32-pthreads build helper for the Digital Mars Compiler, and the library's
ChangeLog is the only reference to it -- so nothing else in the tree changes
when it moves.

The batch file left `%errorlevel%` at the last command's status (it checked
nothing between the two), so this returns the debug command's exit status.
"""

from __future__ import annotations

import subprocess
import sys

# `set DMCDIR=c:\dm`; the compiler the batch file ran twice.
DMCDIR = r"c:\dm"
DMC = r"c:\dm\bin\dmc"

# Everything after the program name, identical for both builds; `-g` is the
# debug build's only addition.
COMMON = (
    "-D_WIN32_WINNT",
    "-D_MT",
    "-DHAVE_PTW32_CONFIG_H",
    "-I.;c:\\dm\\include",
    "-o+all",
    "-WD",
    "pthread.c",
    "user32.lib+kernel32.lib+wsock32.lib",
    "-L/impl",
    "-L/NODEBUG",
    "-L/SU:WINDOWS",
)


def argv(debug: bool) -> list[str]:
    """The `dmc` command line for one build, exactly as the batch file wrote it."""
    return [DMC, *(["-g"] if debug else []), *COMMON]


def main() -> int:
    """Run the release build, then the debug build, and report the last status."""
    status = 0
    for debug in (False, True):
        result = subprocess.run(argv(debug), check=False)
        status = result.returncode
    return status


if __name__ == "__main__":
    sys.exit(main())
