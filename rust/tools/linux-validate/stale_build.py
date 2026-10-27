#!/usr/bin/env -S uv run --script
"""Start a Tier 2 /build dir over when the Rust toolchain pin changed (#937).

    uv run --script rust/tools/linux-validate/stale_build.py \\
        BUILD_DIR RUSTC_VERSION [SRC]

The /build volume outlives image rebuilds, and Corrosion caches the configuring
toolchain's rustc and cargo paths, so after the rust-toolchain.toml pin changes
the image no longer has those paths and reconfiguring an old build dir fails
with "Could not find toolchain". BUILD_DIR is removed unless BUILD_DIR.rustc
records RUSTC_VERSION; a tree configured from another source path (SRC, the old
/src copy, now /ro) is removed too, because CMake refuses "does not match the
source ... used to generate cache". The version is then recorded; zccache keeps
the C rebuild warm. Ported from stale-build.sh (#962).

SRC is optional, matching the shell's `${SRC:-}`: without it the source-path
check is skipped. Tested by ci/test_tier2_run.py.
"""

from __future__ import annotations

import shutil
import sys
from pathlib import Path

CACHE = "CMakeCache.txt"


def _stamp(build: Path) -> Path:
    """BUILD_DIR.rustc, the sibling file recording the configured toolchain."""
    return build.with_name(build.name + ".rustc")


def _recorded(stamp: Path) -> str:
    """The stamp's text as `$(cat ...)` read it: trailing newlines stripped."""
    try:
        return stamp.read_text(errors="replace").rstrip("\n")
    except OSError:
        return ""


def _configured_from(cache: Path, src: str) -> bool:
    """Whether the cache records src as CMAKE_HOME_DIRECTORY (grep -qx)."""
    wanted = f"CMAKE_HOME_DIRECTORY:INTERNAL={src}"
    try:
        return wanted in cache.read_text(errors="replace").splitlines()
    except OSError:
        return False


def reset_stale_build(build: Path, version: str, src: str | None = None) -> None:
    """Remove a build dir left by another toolchain or source path, then stamp it."""
    stamp = _stamp(build)
    if _recorded(stamp) != version:
        shutil.rmtree(build, ignore_errors=True)
    cache = build / CACHE
    if cache.is_file() and src and not _configured_from(cache, src):
        shutil.rmtree(build, ignore_errors=True)
    stamp.write_text(f"{version}\n")


def main(argv: list[str]) -> int:
    if len(argv) not in (3, 4):
        print("usage: stale_build.py BUILD_DIR RUSTC_VERSION [SRC]", file=sys.stderr)
        return 2
    src = (argv[3] if len(argv) == 4 else "") or None
    reset_stale_build(Path(argv[1]), argv[2], src)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
