#!/usr/bin/env -S uv run --script
"""Which CI jobs a change needs (obs-rust/obs-studio#436).

    uv run ci/ci_scope.py <base-ref> --event <name> --ref <ref>
                                            # writes core/macos/windows to $GITHUB_OUTPUT

One table (LANES) maps path globs to lanes; a lane runs only when a changed
file matches its row, otherwise its job is skipped and passes CI Gate. No
label changes what runs (#992). A master push adds the lanes a PR defers;
schedule, workflow_dispatch, tags and the release gate run everything (#436).
Lanes:
- lint:    always (fmt, clippy, Dylint, formatters, unit tests of the crates
           the diff touches, ci/rust_scope.py): the Fast Gate job;
- tier2:   libobs, the crates linked into libobs (derived from
           rust/libobs-rust/Cargo.toml), test/cmocka, deps, the validate
           tooling, and every plugin/frontend/shared source a cmocka test
           compiles (its ${CMAKE_SOURCE_DIR}/... paths, submodule bumps
           included) -> Tier 2 Linux (cmocka against hybrid libobs and
           libobs-c-reference);
- linux_targets: plugins/<x>/ and shared/<x>/ touched -> the Linux CMake
           targets of those modules (one configure, ENABLE_FRONTEND=OFF, no
           Qt); untouched modules are not built, nothing touched skips the job;
- full:    never from a PR path: every OBS app build (Ubuntu, macOS, Windows,
           Flatpak) runs on schedule, workflow_dispatch, tags and the release gate
           (#436); a master push builds none of them. Platform and Qt files
           are costly: macOS and
           Windows start only for a file of that platform or a shared build
           file (cmake/common, cmake/finders, libobs/cmake, root CMake; NOT
           .github/, whose edits run the ci/ self-tests and the changed jobs
           on master); Qt only for frontend/Qt-linked paths and the Qt job's
           own files. A frontend platform file starts no Windows/macOS Rust
           or Tier 2 job (Windows/macOS Qt lanes are a follow-up), except a
           host-named one (below).
- docs/, ci/, .github/ and unported plugins start nothing but lint.
- docs_only: every changed file is documentation (DOCS_ONLY): a PR or
           merge-queue run skips Fast Gate too, since no Rust, C or CMake
           changed. The Format job's ci/ self-tests, which read AGENTS.md,
           SECURITY.md, the skills and other docs, still run (#2079).
Platform and Qt lanes:
- macos:   macOS code (or shared build/CI config) changed -> macOS Rust
           tests and Tier 2;
- windows: Windows code (or shared build/CI config) changed -> Windows Rust
           tests and Tier 2;
- qt:      frontend/Qt code or a Qt-linked cmocka test changed -> the Linux
           Qt frontend build and its unit tests (#992);
- qt_sdk:  rust/obs-build's Qt SDK resolver changed -> the macOS and
           Windows Rust lanes check it against the published Qt SDKs (#1878);
- core:    libobs or the Rust workspace changed (informational).
Platform is decided by path alone (#897, ci/platform_paths.py): a changed
file of a platform (`win-capture/`, `platform-osx.mm`, `threading-posix.c`,
`rust/*/src/platform/windows/`, ...) starts that platform's jobs. Core files
carry no platform defines (ci/platform_ifdef_ratchet.py), so on a PR or a
merge-queue run a core change is Linux only. A push to master is the
exception: core code selects the macOS and Windows Rust lanes too, because
the platform builds read it and a Windows-only break in it otherwise lands
unseen (#2916, #2910). `.github/`, `cmake/common/`, `cmake/finders/`, `libobs/cmake/`
(except `os-<os>.cmake`, which starts its own OS, and the three hidden-symbol
lists when they add or drop the same plain names, which Linux checks: #2288),
the root CMake files, crate `build.rs`, `platform/mod.rs` and the C++ cmocka tests (with their
tests.d registrations) stay shared: every platform consumes them.
Host-named files (docs/rust-port/naming.md, #1294) start their OS lane in any
area, Qt areas included: `foo-host-<os>.c.h` (unity group
`foo.host-<os>+.h.c`), a `hosts/<os>/` directory, or a `foo-<os>.pch.<ext>`
precompiled header, <os> in windows|macos|linux|posix (posix -> Linux and
macOS); platform_paths.host_platforms decides it. A Qt-area path still also
starts qt.
Every OBS app build (Ubuntu, macOS, Windows, Flatpak) runs only in the
release gate (`full`). A job that is skipped here can't hide a break: it
runs at the release gate.
"""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

from platform_paths import LINUX, MACOS, WINDOWS, host_platforms, platforms as name_platforms

CORE_PREFIXES = ("libobs/", "rust/")
CORE_FILES = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
# Build and CI configuration every platform consumes.
SHARED_PREFIXES = ("cmake/common/", "cmake/finders/", "libobs/cmake/")
SHARED_FILES = {"CMakeLists.txt", "CMakePresets.json", "buildspec.json"}
SHARED_PATTERN = re.compile(r"^rust/[^/]+/(?:build\.rs|src/platform/mod\.rs)$")
# libobs/cmake/os-<os>.cmake picks one OS's sources: that OS only (#1051).
OS_CMAKE = re.compile(r"^libobs/cmake/os-[a-z]+\.cmake$")
# Every platform's Tier 2 compiles these with its own compiler and C++
# library; plain C tests stay Linux-only (#610).
SHARED_TEST = re.compile(r"^test/cmocka/(CMakeLists\.txt|[^/]+\.(cpp|hpp))$")
# A tests.d registration is shared when it registers a C++ test.
TESTS_D = re.compile(r"^test/cmocka/tests\.d/([^/]+)\.cmake$")
# One hidden-symbol inventory, written once per linker (#2288).
HIDDEN_LISTS = ("libobs/cmake/rust-hidden.txt", "libobs/cmake/rust-exports.map", "libobs/cmake/rust-unexports-macos.txt")


HIDDEN_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
MAP_NAME = re.compile(r"^\s*([A-Za-z_][A-Za-z0-9_]*);[ \t]*$", re.M)
MAP_COMMENT = re.compile(r"/\*.*?\*/", re.S)


def _split_hidden(text: str, prefix: str) -> tuple[set[str], str]:
    """(plain symbol names, every other line). Wildcards stay in the rest."""
    names, rest = set(), []
    for line in text.splitlines():
        word = line.strip()
        if word.startswith("#") or not word:
            continue
        if word.startswith(prefix) and HIDDEN_NAME.fullmatch(word[len(prefix):]):
            names.add(word[len(prefix):])
        else:
            rest.append(word)
    return names, "\n".join(rest)


def _split_map(text: str) -> tuple[set[str], str]:
    text = MAP_COMMENT.sub("", text)
    names = set(MAP_NAME.findall(text))
    return names, "\n".join(l.strip() for l in MAP_NAME.sub("", text).splitlines() if l.strip())


def hidden_lists_mirrored(old: Callable[[str], str | None], new: Callable[[str], str | None]) -> bool:
    """True when the three lists add or drop the same plain names and nothing
    else: no wildcard, section or `global:` change. Every linker then hides
    exactly what the Linux link hides, and Windows reads none of them."""
    splits = (lambda t: _split_hidden(t, ""), _split_map, lambda t: _split_hidden(t, "_"))
    deltas = set()
    for path, split in zip(HIDDEN_LISTS, splits):
        before, after = split(old(path) or ""), split(new(path) or "")
        if before[1] != after[1]:
            return False
        deltas.add((frozenset(after[0] - before[0]), frozenset(before[0] - after[0])))
    return len(deltas) == 1


# Files no build or Rust lint reads (#2079). ci/test_*.py read some of them,
# and the Format job runs those tests on every PR. docs/ci-evidence/ is retired
# (#2686); it stays here so a branch that still carries a snapshot is read as
# documentation rather than product code.
DOCS_ONLY = re.compile(r"^(?:.*\.md|docs/ci-evidence/.*|LICENSE|COPYING|\.gitignore)\Z")


def is_docs(path: str) -> bool:
    return bool(DOCS_ONLY.match(path))


@dataclass(frozen=True)
class Scope:
    core: bool
    macos: bool
    windows: bool
    qt: bool = False
    tier2: bool = False
    full: bool = False
    linux_targets: str = ""
    qt_sdk: bool = False
    docs_only: bool = False
    cross_targets: str = ""


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


# Cmocka tests that build plugin, frontend or shared sources name them as
# ${CMAKE_SOURCE_DIR}/<path>; a change under <path> runs Tier 2 too.
_SOURCE_REF = re.compile(r"\$\{CMAKE_SOURCE_DIR\}/([^\s\"')$;]+)")
CMOCKA_BUILD_FILES = ("test/cmocka/CMakeLists.txt", "test/cmocka/tests.d")


def cmocka_source_paths(texts: list[str]) -> tuple[str, ...]:
    """Source paths (files or directories) the cmocka build files compile."""
    found = {m.rstrip("/") for text in texts for m in _SOURCE_REF.findall(text)}
    # A top-level directory (frontend, libobs-opengl) is only an include
    # root; matching it would send every change under it to Tier 2.
    return tuple(sorted(p for p in found if "/" in p))


def repo_cmocka_source_paths(root: Path = Path(__file__).resolve().parent.parent) -> tuple[str, ...]:
    files = [root / CMOCKA_BUILD_FILES[0], *sorted((root / CMOCKA_BUILD_FILES[1]).glob("*.cmake"))]
    return cmocka_source_paths([read_text(str(f)) for f in files])


def cmocka_compiles(path: str, refs: tuple[str, ...]) -> bool:
    # path under a referenced source, or a gitlink (submodule root) above one.
    return any(path == r or path.startswith(r + "/") or r.startswith(path + "/") for r in refs)


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
# docs/.../platform-windows.md starts nothing. Core files start Linux only on
# a PR or merge-queue run; a master push adds both platform lanes (#2916).
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
    # Upstream names (win-capture/, mac-*, obs-nix.c, platform-cocoa.m, ...)
    # keep their paths, so this token classifier is permanent: an ambiguous
    # path gets an entry in ci/platform_paths.py, never a rename (#1051).
    return name_platforms(path)


def os_lanes(path: str) -> frozenset[str]:
    if not path.startswith(OS_LANES[LINUX]):
        return frozenset()
    return path_platforms(path) or frozenset({LINUX})


def is_shared(path: str, exists: Callable[[str], bool]) -> bool:
    if OS_CMAKE.match(path):
        return False
    if path.startswith(SHARED_PREFIXES) or path in SHARED_FILES or SHARED_PATTERN.match(path) or SHARED_TEST.match(path):
        return True
    m = TESTS_D.match(path)
    return bool(m) and any(exists(f"test/cmocka/{m.group(1)}.{ext}") for ext in ("cpp", "hpp"))


# The Linux Qt frontend job (#992) runs only when the change can break the Qt
# frontend build or a Qt-linked unit test; master pushes, schedule and release runs always
# run it (build-project.yaml).
QT_PREFIXES = ("frontend/", "shared/qt/", "shared/properties-view/", "plugins/frontend-tools/", "ci/linux_qt_frontend")
QT_FILES = {
    "ci/fetch_libs_asset.py",
    "ci/test_qt_fastpath.py",
    "ci/test_qt_cache_budget.py",
    "ci/test_qt_cache_identity.py",
    "test/cmocka/CMakeLists.txt",
    "cmake/finders/FindFFmpeg.cmake",
}
TEST_SOURCE = re.compile(r"^test/cmocka/([^/]+)\.(?:cpp|hpp|c|h)$")


# rust/obs-build's Qt SDK resolver and native Qt test graph: the Windows and
# macOS lanes fetch the published obs-studio-libs SDKs and run them (#1878).
QT_SDK_PATHS = (
    "rust/obs-build/src/qt.rs",
    "rust/obs-build/src/qt/**",
    "rust/obs-build/src/platform/qt_sdk.rs",
    "rust/obs-build/tests/qt_sdk.rs",
    # The native test graph and package linking the Qt tests build through.
    "rust/obs-build/tests/native.rs",
    "rust/obs-build/src/validation.rs",
    "rust/obs-build/src/validation/**",
    "rust/obs-build/src/platform/shared_library.rs",
)


def is_qt_sdk(path: str) -> bool:
    return any(_glob(g).match(path) for g in QT_SDK_PATHS)


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
    # Gitlink updates name the module directory itself, with no child path.
    # Top-level build files (plugins/CMakeLists.txt etc.) are not targets.
    if len(parts) < 2 or (len(parts) == 2 and "." in parts[1]):
        return None
    plat = path_platforms(path)
    if plat is not None and LINUX not in plat:
        return None  # win-capture/, mac-*/ ... never build on Linux
    return parts[1]


# The cross-compile check (cmake/cross/, #1294) compiles libobs and the
# components its CMakeLists.txt names by `DIR`, for these aliases.
CROSS_LISTS = "cmake/cross/CMakeLists.txt"
CROSS_SELF = ("cmake/cross/", "cmake/common/soldr.cmake")
CROSS_ALIASES = {WINDOWS: "win-x64", MACOS: "mac-arm64"}


def cross_dirs(read: Callable[[str], str]) -> frozenset[str]:
    """libobs plus every literal `DIR` of the cross CMakeLists."""
    return frozenset({"libobs", *re.findall(r"\bDIR ([\w./-]+)", read(CROSS_LISTS))})


def cross_targets(files: list[str], read: Callable[[str], str]) -> str:
    """The cross aliases a change needs: a Windows or macOS file in a component
    the check compiles, or the check itself (both)."""
    dirs = cross_dirs(read)
    oses: set[str] = set()
    for path in files:
        if path.startswith(CROSS_SELF):
            oses |= set(CROSS_ALIASES)
        elif any(path.startswith(d + "/") for d in dirs):
            oses |= set(host_platforms(path) or path_platforms(path) or ()) & set(CROSS_ALIASES)
    return ",".join(sorted(CROSS_ALIASES[os] for os in oses))


def core_master_push_lanes(core: bool, event: str, ref: str) -> bool:
    """Core code on a push to master also needs both platform Rust lanes (#2916).

    A `libobs/` or `rust/` only diff selects no platform lane, so a Windows- or
    macOS-only break in core code reached master having never been compiled
    there (#2910: glibc tolerated the mutex destroy, the Windows pthread layer
    crashed, and the failure surfaced in an unrelated `shared/` PR's lane).
    Master pushes only: a PR and a merge-queue run keep the path selection, so
    the queue pays nothing for it."""
    return core and event == "push" and ref == "master"


def scope(
    files: list[str],
    exists: Callable[[str], bool] = lambda path: False,
    read: Callable[[str], str] = lambda path: "",
    submodules: frozenset[str] = frozenset(),
    cmocka_paths: tuple[str, ...] = (),
    hidden_mirrored: bool = False,
    event: str = "",
    ref: str = "",
) -> Scope:
    """`exists(path)` says whether a file exists in the checkout, `read(path)`
    returns its text ("" when missing). `submodules` is accepted for callers
    but no path escalates to a full run any more. `event`/`ref` are the
    workflow's, and only change core code on a master push (`core_master_push_lanes`)."""
    core = macos = windows = qt = tier2 = qt_sdk = False
    targets: set[str] = set()
    linked = tuple(f"rust/{c}/**" for c in sorted(linked_crates(read)))
    for path in files:
        qt |= is_qt(path, read)
        plat = path_platforms(path)
        # A platform-only file (libobs/**/platform-windows.c) never starts the
        # Linux Tier 2.
        if (lane_hit("tier2", path, linked) or cmocka_compiles(path, cmocka_paths)) and (
            plat is None or LINUX in plat
        ):
            tier2 = True
        if target := linux_target(path):
            targets.add(target)
        if path.startswith(CORE_PREFIXES) or path in CORE_FILES:
            core = True
        if is_qt_sdk(path):
            qt_sdk = macos = windows = True
        if hidden_mirrored and path in HIDDEN_LISTS:
            continue
        if is_shared(path, exists):
            macos = windows = True
            continue
        if path.startswith(QT_AREAS):
            # A host-named file still starts its OS lane (#1294).
            host = host_platforms(path) or frozenset()
            macos |= MACOS in host
            windows |= WINDOWS in host
            continue
        found = os_lanes(path)
        macos |= MACOS in found
        windows |= WINDOWS in found
    if core_master_push_lanes(core, event, ref):
        macos = windows = True
    docs_only = bool(files) and all(is_docs(path) for path in files)
    return Scope(core, macos, windows, qt, tier2, False, ",".join(sorted(targets)), qt_sdk, docs_only, cross_targets(files, read))


def changed_files(base: str) -> list[str]:
    out = subprocess.run(
        (["git", "ls-files"] if base == "ALL" else ["git", "diff", "--name-only", f"{base}...HEAD"]),
        check=True, capture_output=True, text=True
    ).stdout
    return [line for line in out.splitlines() if line]


def read_text(path: str) -> str:
    try:
        return Path(path).read_text()
    except OSError:
        return ""


def base_text(base: str, path: str) -> str:
    out = subprocess.run(["git", "show", f"{base}:{path}"], capture_output=True, text=True)
    return out.stdout if out.returncode == 0 else ""


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0] if __doc__ else None)
    parser.add_argument("base", help="git ref to diff against, or ALL for every tracked file")
    parser.add_argument("--event", default="", help="the workflow's event name (GITHUB_EVENT_NAME)")
    parser.add_argument("--ref", default="", help="the workflow's ref name (GITHUB_REF_NAME)")
    args = parser.parse_args()
    files = changed_files(args.base)
    mirrored = False
    if args.base != "ALL" and any(path in HIDDEN_LISTS for path in files):
        fork = subprocess.run(
            ["git", "merge-base", args.base, "HEAD"], check=True, capture_output=True, text=True
        ).stdout.strip()
        mirrored = hidden_lists_mirrored(lambda path: base_text(fork, path), read_text)
    result = scope(
        files,
        lambda path: Path(path).is_file(),
        read_text,
        cmocka_paths=repo_cmocka_source_paths(Path(".")),
        hidden_mirrored=mirrored,
        event=args.event,
        ref=args.ref,
    )
    lines = [f"{k}={str(v).lower()}" for k, v in vars(result).items()]
    print("\n".join(lines))
    if out := os.environ.get("GITHUB_OUTPUT"):
        with open(out, "a") as f:
            f.write("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
