#!/usr/bin/env -S uv run --script
"""Which CI jobs a change needs (obs-rust/obs-studio#436).

    uv run ci/ci_scope.py <base-ref>        # writes core/macos/windows to $GITHUB_OUTPUT

The Linux Rust tests and Tier 2 always run. On top of them:
- macos:   macOS code (or shared build/CI config) changed -> macOS Rust
           tests and Tier 2;
- windows: Windows code (or shared build/CI config) changed -> Windows Rust
           tests and Tier 2;
- core:    libobs or the Rust workspace changed (informational).
Every OBS app build (Ubuntu, macOS, Windows, Flatpak) runs only in the
release gate (`full`). A job that is skipped here can't hide a break: it
runs at the release gate.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from dataclasses import dataclass

TOKEN = r"(?:^|[/_.\-])({})(?:[/_.\-]|$)"
MACOS = re.compile(
    TOKEN.format("mac|macos|osx|cocoa|apple|darwin|coreaudio|avcapture|syphon|videotoolbox|unix"),
    re.I,
)
WINDOWS = re.compile(TOKEN.format("win|win32|windows|wasapi|dshow|d3d9|d3d11|d3d12|dxgi|w32-pthreads"), re.I)
MACOS_SUFFIXES = (".m", ".mm", ".plist", ".xcconfig", ".entitlements")
WINDOWS_SUFFIXES = (".rc", ".manifest", ".ps1")
CORE_PREFIXES = ("libobs/", "rust/")
CORE_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
# Build and CI configuration every platform consumes.
SHARED_PREFIXES = (".github/", "cmake/common/", "cmake/finders/", "libobs/cmake/")
SHARED_FILES = {"CMakeLists.txt", "CMakePresets.json", "buildspec.json"}
SHARED_PATTERN = re.compile(r"^rust/[^/]+/src/platform/mod\.rs$")
# Every platform's Tier 2 compiles these with its own compiler and C++
# library; plain C tests stay Linux-only (#610).
SHARED_TEST = re.compile(r"^test/cmocka/(CMakeLists\.txt|tests\.d/[^/]+\.cmake|[^/]+\.(cpp|hpp))$")


@dataclass(frozen=True)
class Scope:
    core: bool
    macos: bool
    windows: bool


def scope(files: list[str]) -> Scope:
    core = macos = windows = False
    for path in files:
        if path.startswith(CORE_PREFIXES) or path in CORE_FILES:
            core = True
        if path.startswith(SHARED_PREFIXES) or path in SHARED_FILES or SHARED_PATTERN.match(path) or SHARED_TEST.match(path):
            macos = windows = True
            continue
        if path.startswith("cmake/macos/") or path.endswith(MACOS_SUFFIXES) or MACOS.search(path):
            macos = True
        if path.startswith("cmake/windows/") or path.endswith(WINDOWS_SUFFIXES) or WINDOWS.search(path):
            windows = True
    return Scope(core, macos, windows)


def changed_files(base: str) -> list[str]:
    out = subprocess.run(
        ["git", "diff", "--name-only", f"{base}...HEAD"], check=True, capture_output=True, text=True
    ).stdout
    return [line for line in out.splitlines() if line]


def main() -> int:
    files = changed_files(sys.argv[1])
    result = scope(files)
    lines = [f"{k}={str(v).lower()}" for k, v in vars(result).items()]
    print("\n".join(lines))
    if out := os.environ.get("GITHUB_OUTPUT"):
        with open(out, "a") as f:
            f.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
