#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Link Windows MSVC Rust categories and selected Tier 2 fixtures from Linux.

    uv run --no-project ci/windows_link_check.py

Every non-docs fallback PR and merge group links the eight category binaries,
including the C oracle (#3919). For libobs and registration/support changes,
LANES["cmocka_link"] also selects hybrid libobs and every configured cmocka
`*_rust` executable (#5189). The latter uses the real registrations, deferred
support fragments, soldr's clang-cl/LLD and Windows SDK. Nothing is executed.

OBS_CMOCKA_LINK=false keeps category-only coverage for unrelated changes.
Complete member admission may defer the PR job; selected queue jobs still run.
"""

from __future__ import annotations

import os
import sys

import cmocka_link_check

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
    if os.environ.get("OBS_CMOCKA_LINK", "true") == "true":
        cmocka_link_check.link("win-x64", cmocka_link_check.ROOT / "build_cmocka_link_win-x64", 4)
    return 0


if __name__ == "__main__":
    sys.exit(main())
