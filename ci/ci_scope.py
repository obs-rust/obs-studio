#!/usr/bin/env -S uv run --script
"""Which CI jobs a change needs (obs-rust/obs-studio#436).

    uv run ci/ci_scope.py <base-ref>        # writes core/macos/windows to $GITHUB_OUTPUT

One table (LANES) maps path globs to lanes; a lane runs only when a changed
file matches its row, otherwise its job is skipped and passes CI Gate. No
label changes what runs (#992): master push, schedule, workflow_dispatch and
releases run everything. Lanes:
- lint:    always (fmt, clippy, Dylint, formatters, unit tests of the crates
           the diff touches, ci/rust_scope.py): the Fast Gate job;
- tier2:   libobs, the crates linked into libobs (derived from
           rust/libobs-rust/Cargo.toml), test/cmocka, deps, the validate
           tooling -> Tier 2 Linux (cmocka, ENABLE_RUST_LIBOBS OFF/ON);
- linux_targets: plugins/<x>/ and shared/<x>/ touched -> the Linux CMake
           targets of those modules (one configure, ENABLE_FRONTEND=OFF, no
           Qt); untouched modules are not built, nothing touched skips the job;
- full:    never from a PR path: every OBS app build (Ubuntu, macOS, Windows,
           Flatpak) runs on master push, schedule, workflow_dispatch, tags and
           the release gate only. Platform and Qt files are costly: macOS and
           Windows start only for a file of that platform or a shared build
           file (cmake/common, cmake/finders, libobs/cmake, root CMake; NOT
           .github/, whose edits run the ci/ self-tests and the changed jobs
           on master); Qt only for frontend/Qt-linked paths and the Qt job's
           own files. A frontend platform file starts no Windows/macOS Rust
           or Tier 2 job (Windows/macOS Qt lanes are a follow-up).
- docs/, ci/, .github/ and unported plugins start nothing but lint.
Platform and Qt lanes:
- macos:   macOS code (or shared build/CI config) changed -> macOS Rust
           tests and Tier 2;
- windows: Windows code (or shared build/CI config) changed -> Windows Rust
           tests and Tier 2;
- qt:      frontend/Qt code or a Qt-linked cmocka test changed -> the Linux
           Qt frontend build and its unit tests (#992);
- core:    libobs or the Rust workspace changed (informational).
Platform is decided by path alone (#897, ci/platform_paths.py): a changed
file of a platform (`win-capture/`, `platform-osx.mm`, `threading-posix.c`,
`rust/*/src/platform/windows/`, ...) starts that platform's jobs. Core files
carry no platform defines (ci/platform_ifdef_ratchet.py), so a core change
runs on Linux. `.github/`, `cmake/common/`, `cmake/finders/`, `libobs/cmake/`,
the root CMake files, `platform/mod.rs` and the C++ cmocka tests (with their
tests.d registrations) stay shared: every platform consumes them.
Every OBS app build (Ubuntu, macOS, Windows, Flatpak) runs only in the
release gate (`full`). A job that is skipped here can't hide a break: it
runs at the release gate.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from platform_paths import LINUX, MACOS, WINDOWS, platforms as legacy_platforms

CORE_PREFIXES = ("libobs/", "rust/")
CORE_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
# Build and CI configuration every platform consumes.
SHARED_PREFIXES = ("cmake/common/", "cmake/finders/", "libobs/cmake/")
SHARED_FILES = {"CMakeLists.txt", "CMakePresets.json", "buildspec.json"}
SHARED_PATTERN = re.compile(r"^rust/[^/]+/src/platform/mod\.rs$")
# Every platform's Tier 2 compiles these with its own compiler and C++
# library; plain C tests stay Linux-only (#610).
SHARED_TEST = re.compile(r"^test/cmocka/(CMakeLists\.txt|[^/]+\.(cpp|hpp))$")
# A tests.d registration is shared when it registers a C++ test.
TESTS_D = re.compile(r"^test/cmocka/tests\.d/([^/]+)\.cmake$")


@dataclass(frozen=True)
class Scope:
    core: bool
    macos: bool
    windows: bool
    qt: bool = False
    tier2: bool = False
    full: bool = False
    linux_targets: str = ""


# The path -> lane table (#992). Adding a module is adding a row. Globs use
# `**` for any depth and `*` within one path segment. Rust crates linked into
# libobs are added to tier2 at run time (linked_crates).
LANES: dict[str, tuple[str, ...]] = {
    "tier2": (
        "libobs/**",
        "test/cmocka/**",
        "deps/**",
        "rust/tools/linux-validate/**",
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
    ),
}


def _glob(pattern: str) -> re.Pattern[str]:
    out = ""
    i = 0
    while i < len(pattern):
        if pattern.startswith("**", i):
            out += ".*"
            i += 2
        elif pattern[i] == "*":
            out += "[^/]*"
            i += 1
        else:
            out += re.escape(pattern[i])
            i += 1
    return re.compile(out + r"\Z")


def lane_hit(lane: str, path: str, extra: tuple[str, ...] = ()) -> bool:
    return any(_glob(g).match(path) for g in LANES.get(lane, ()) + extra)


def linked_crates(read: Callable[[str], str], root: str = "libobs-rust") -> set[str]:
    """Crates libobs links: rust/<root> and its [dependencies] (not dev
    dependencies), transitively, read from the Cargo.toml files."""
    seen: set[str] = set()
    todo = [root]
    while todo:
        name = todo.pop()
        if name in seen:
            continue
        seen.add(name)
        section = ""
        for line in read(f"rust/{name}/Cargo.toml").splitlines():
            line = line.strip()
            if line.startswith("["):
                section = line
            elif section in ("[dependencies]", "[build-dependencies]"):
                m = re.search(r'path\s*=\s*"\.\./([^"/]+)"', line)
                if m:
                    todo.append(m.group(1))
    return seen


# The OS lanes (#992), transposed: each lists the areas that can start it. A
# changed file starts an OS lane only inside one of its areas, so a
# docs/.../platform-windows.md starts nothing. Core files start Linux only.
_ALL_AREAS = ("libobs", "rust/", "frontend/", "plugins/", "shared/", "deps/", "cmake/", "test/", "build-aux/")
OS_LANES: dict[str, tuple[str, ...]] = {
    LINUX: _ALL_AREAS,
    WINDOWS: _ALL_AREAS,
    MACOS: _ALL_AREAS,
}

# ONE platform trigger rule, matching the Rust layout: rust/<crate>/src/platform/
# {windows,macos,linux,posix}/..., shared/<x>/platform/..., foo-platform-windows.c.
# posix builds on Linux and macOS. Anything else is core: Linux lane only.
PLATFORM_RULE = re.compile(r"/platform[/-](windows|macos|linux|posix)[/.]")


def path_platforms(path: str) -> frozenset[str] | None:
    """The OS lanes a platform file starts, or None for core (Linux only)."""
    m = PLATFORM_RULE.search("/" + path)
    if m:
        return {"windows": frozenset({WINDOWS}), "macos": frozenset({MACOS}), "linux": frozenset({LINUX}), "posix": frozenset({LINUX, MACOS})}[m.group(1)]
    # TODO(obs-rust/obs-studio platform-rename issue): legacy tokens (win-capture/,
    # mac-*, unix/nix/cocoa/osx names) until those paths follow the
    # platform[/-]<os> layout; then delete this fallback.
    return legacy_platforms(path)


def os_lanes(path: str) -> frozenset[str]:
    if not path.startswith(OS_LANES[LINUX]):
        return frozenset()
    return path_platforms(path) or frozenset({LINUX})


def is_shared(path: str, exists: Callable[[str], bool]) -> bool:
    if path.startswith(SHARED_PREFIXES) or path in SHARED_FILES or SHARED_PATTERN.match(path) or SHARED_TEST.match(path):
        return True
    m = TESTS_D.match(path)
    return bool(m) and any(exists(f"test/cmocka/{m.group(1)}.{ext}") for ext in ("cpp", "hpp"))


# The Linux Qt frontend job (#992) runs only when the change can break the Qt
# frontend build or a Qt-linked unit test; master, schedule and release runs always
# run it (build-project.yaml).
QT_PREFIXES = ("frontend/", "shared/qt/", "shared/properties-view/", "plugins/frontend-tools/", "ci/linux_qt_frontend")
QT_FILES = {
    "ci/fetch_libs_asset.py",
    "ci/test_qt_fastpath.py",
    "test/cmocka/CMakeLists.txt",
    "cmake/finders/FindFFmpeg.cmake",
}
TEST_SOURCE = re.compile(r"^test/cmocka/([^/]+)\.(?:cpp|hpp|c|h)$")


def is_qt(path: str, read: Callable[[str], str]) -> bool:
    """Frontend/Qt code, the job's own files, or a Qt-linked cmocka test: a
    tests.d fragment that finds Qt6, or the test source it registers."""
    if path.startswith(QT_PREFIXES) or path in QT_FILES:
        return True
    m = TESTS_D.match(path) or TEST_SOURCE.match(path)
    return bool(m) and "Qt6" in read(f"test/cmocka/tests.d/{m.group(1)}.cmake")


# Areas whose files are Qt/frontend: a platform file there starts no OS Rust
# or Tier 2 job (Windows/macOS Qt lanes are a follow-up).
QT_AREAS = ("frontend/", "shared/qt/", "shared/properties-view/", "plugins/frontend-tools/")
# Linux CMake targets, one row per module root (#992): the module directory
# is the target name; CI filters to targets that exist and are Linux-built.
LINUX_TARGET_ROOTS = ("plugins/", "shared/")


def linux_target(path: str) -> str | None:
    if not path.startswith(LINUX_TARGET_ROOTS) or path.startswith(QT_AREAS):
        return None
    parts = path.split("/")
    if len(parts) < 3:
        return None  # plugins/CMakeLists.txt etc.: not one module
    plat = path_platforms(path)
    if plat is not None and LINUX not in plat:
        return None  # win-capture/, mac-*/ ... never build on Linux
    return parts[1]


def scope(
    files: list[str],
    exists: Callable[[str], bool] = lambda path: False,
    read: Callable[[str], str] = lambda path: "",
    submodules: frozenset[str] = frozenset(),
) -> Scope:
    """`exists(path)` says whether a file exists in the checkout, `read(path)`
    returns its text ("" when missing). `submodules` is accepted for callers
    but no path escalates to a full run any more."""
    core = macos = windows = qt = tier2 = False
    targets: set[str] = set()
    linked = tuple(f"rust/{c}/**" for c in sorted(linked_crates(read)))
    for path in files:
        qt |= is_qt(path, read)
        plat = path_platforms(path)
        # A platform-only file (libobs/**/platform-windows.c) never starts the
        # Linux Tier 2.
        if lane_hit("tier2", path, linked) and (plat is None or LINUX in plat):
            tier2 = True
        if target := linux_target(path):
            targets.add(target)
        if path.startswith(CORE_PREFIXES) or path in CORE_FILES:
            core = True
        if is_shared(path, exists):
            macos = windows = True
            continue
        if path.startswith(QT_AREAS):
            continue
        found = os_lanes(path)
        macos |= MACOS in found
        windows |= WINDOWS in found
    return Scope(core, macos, windows, qt, tier2, False, ",".join(sorted(targets)))


def changed_files(base: str) -> list[str]:
    out = subprocess.run(
        ["git", "diff", "--name-only", f"{base}...HEAD"], check=True, capture_output=True, text=True
    ).stdout
    return [line for line in out.splitlines() if line]


def read_text(path: str) -> str:
    try:
        return Path(path).read_text()
    except OSError:
        return ""


def main() -> int:
    files = changed_files(sys.argv[1])
    result = scope(files, lambda path: Path(path).is_file(), read_text)
    lines = [f"{k}={str(v).lower()}" for k, v in vars(result).items()]
    print("\n".join(lines))
    if out := os.environ.get("GITHUB_OUTPUT"):
        with open(out, "a") as f:
            f.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
