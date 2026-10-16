#!/usr/bin/env -S uv run --script
"""Which workspace crates a PR's unit tests must run (#992 fast gate).

    uv run ci/local_gate.py --test-diff <base-sha>

A change confined to crate dirs under rust/ tests those crates and everything
that depends on them. Anything that can reach every crate (libobs C, which the
C oracle compiles, dylints, Cargo files, the toolchain) tests the workspace.
Files that no Rust test reads (docs, frontend, plugins other than nvenc, ...)
skip the tests: the lint lane still ran. Qt is never involved.
"""

from __future__ import annotations

WORKSPACE_PREFIXES = ("libobs/", "dylints/", "deps/", ".cargo/", "rust/tools/", "plugins/obs-nvenc/")
WORKSPACE_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}


def affected(files: list[str], crates: dict[str, tuple[str, set[str]]]):
    """`crates` maps name -> (dir like "rust/obs-util", names it depends on).
    Returns ("skip", []), ("packages", sorted names) or ("workspace", [])."""
    hit: set[str] = set()
    for path in files:
        if path.startswith(WORKSPACE_PREFIXES) or path in WORKSPACE_FILES:
            return "workspace", []
        if not path.startswith("rust/"):
            continue
        owner = [n for n, (d, _) in crates.items() if path.startswith(d + "/")]
        if not owner:
            return "workspace", []
        hit.update(owner)
    if not hit:
        return "skip", []
    grew = True
    while grew:
        grew = False
        for name, (_, deps) in crates.items():
            if name not in hit and deps & hit:
                hit.add(name)
                grew = True
    return "packages", sorted(hit)
