#!/usr/bin/env -S uv run --script
"""Ratchet on the build files every platform consumes (#3237, tracker #3232).

ci/ci_scope.py starts the macOS and Windows lanes for any edit to a shared
build file (cmake/common, cmake/finders, libobs/cmake, the root CMake files,
crate build.rs, ...). Each such file makes every edit to it cost every
platform, and a central list that ports append to makes parallel PRs conflict
and re-run CI. #3233-#3236 split the hot ones into per-unit and per-OS files.

This test keeps the set from growing back: every tracked file ci_scope treats
as shared must be listed in ci/shared-build-files.txt, and every listed file
must still be shared. A new shared file fails until it is either moved into a
per-OS file (cmake/<os>/, os-<os>.cmake, *-<os> names) or a per-unit fragment,
or listed with a reason in the same change. A removed one must leave the list,
so the set only shrinks.

    cd ci && uv run python -m unittest test_shared_build_files
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import unittest
from pathlib import Path

from ci_scope import HIDDEN_FRAGMENT, is_docs, is_shared

ROOT = Path(__file__).resolve().parent.parent
LIST = ROOT / "ci" / "shared-build-files.txt"

# Shared, but one file per unit by design: a port or test adds its own file
# and edits no one else's, so a new one is not a new central list. Every
# platform still builds them (Windows reads the export fragments; each
# platform's Tier 2 compiles the C++ tests).
PER_UNIT = re.compile(
    r"^(?:libobs/cmake/rust-exports\.d/[^/]+\.txt"
    r"|test/cmocka/[^/]+\.(?:cpp|hpp)"
    r"|test/cmocka/tests\.d/[^/]+\.cmake)$"
)


def tracked_files() -> list[str]:
    """Tracked files plus new ones not yet added: `./lint` runs before the
    commit, and a new crate's src/platform/mod.rs passed it untracked, then
    failed in CI (#3420)."""
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return [line for line in out.splitlines() if line]


def shared_now(files: list[str]) -> set[str]:
    present = set(files)
    return {
        f
        for f in files
        if not HIDDEN_FRAGMENT.match(f) and not PER_UNIT.match(f) and not is_docs(f) and is_shared(f, present.__contains__)
    }


def listed() -> set[str]:
    names = set()
    for line in LIST.read_text(encoding="utf-8").splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            names.add(line)
    return names


class SharedBuildFilesTest(unittest.TestCase):
    def test_list_exists(self):
        self.assertTrue(LIST.is_file(), f"{LIST.relative_to(ROOT)} is missing")

    def test_no_new_shared_build_file(self):
        new = sorted(shared_now(tracked_files()) - listed())
        self.assertEqual(
            new,
            [],
            "These files start every platform lane (ci/ci_scope.py is_shared). Move each into a per-OS "
            "file or a per-unit fragment (#3232), or add it to ci/shared-build-files.txt with a reason.",
        )

    def test_untracked_files_are_listed(self):
        probe = ROOT / "rust" / f"obs-zz-probe-{os.getpid()}" / "src" / "platform" / "mod.rs"
        probe.parent.mkdir(parents=True)
        try:
            probe.touch()
            rel = probe.relative_to(ROOT).as_posix()
            self.assertIn(rel, tracked_files())
            self.assertIn(rel, shared_now(tracked_files()))
        finally:
            shutil.rmtree(probe.parents[2])

    def test_no_stale_entry(self):
        stale = sorted(listed() - shared_now(tracked_files()))
        self.assertEqual(stale, [], "No longer shared (moved, split or deleted): remove from ci/shared-build-files.txt.")

    def test_list_is_sorted_and_unique(self):
        names = [l.split("#", 1)[0].strip() for l in LIST.read_text(encoding="utf-8").splitlines()]
        names = [n for n in names if n]
        self.assertEqual(names, sorted(set(names)))


if __name__ == "__main__":
    unittest.main()
