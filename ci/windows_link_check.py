#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Link every workspace test binary for Windows MSVC from Linux (queue tier).

    uv run --no-project ci/windows_link_check.py

The cheap pre-merge signal for Windows link defects. The merge queue runs the
deferred tier, which skips rust-cross-lint, Rust Tests (windows) and Windows
Tier 2, so a COFF-only link failure passed the queue and reddened master
twice: `lld-link: error: undefined symbol: obs_encoder_get_caps` (merge_group
run 42390201886 green, push run 42393067345 red on the same tree; fixed by
#3919). ELF archive resolution on the Linux Fast Gate cannot see it; a COFF
link can, and soldr's self-contained lld-link plus xwin SDK does it on a
Linux runner with no Windows host.

This runs only the link step of the `cross` lane (ci/local_gate.py
cross_lint): no clippy, no Dylint, no macOS target, nothing executed. The
full cross lane, native Windows tests and Tier 2 stay on push/dispatch.
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
