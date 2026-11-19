#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Link every Windows MSVC Rust category binary from Linux, running none.

    uv run --no-project ci/windows_link_check.py

Selected Tier 2 fixtures run independently in the two cmocka-link matrix jobs;
CI Gate requires both aliases even on fully attested member PRs.
"""

from __future__ import annotations

import sys

import local_gate

TARGET = "x86_64-pc-windows-msvc"


def link_args(target: str = TARGET) -> list[str]:
    """The cargo arguments: build and link every test binary, run none.

    The C oracle is included on purpose: its archive members are exactly what
    COFF selection pulls into an unrelated harness (#3918).
    """
    return ["test", "--workspace", "--no-run", "--locked", "--target", target]


def main() -> int:
    local_gate.add_target(TARGET)
    local_gate.soldr_cargo(link_args())
    return 0


if __name__ == "__main__":
    sys.exit(main())
