#!/usr/bin/env -S uv run --script
"""The host CI code runs on, read in one place (ci-lint LAYOUT-001).

A `sys.platform` comparison anywhere else in `ci/` is a boundary violation:
the facade is the file `[allow].platform-code` exempts, so the host knowledge
that the tooling depends on can be read and changed in one spot.
"""

import sys


def is_macos() -> bool:
    return sys.platform == "darwin"


def is_windows() -> bool:
    return sys.platform == "win32"


def is_linux() -> bool:
    return sys.platform == "linux"
