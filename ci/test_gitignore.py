#!/usr/bin/env -S uv run --script
""".gitignore must only ignore generated and local files, never source.

    uv run python -m unittest ci/test_gitignore.py

The upstream file started with `/*` and re-included a list of known
top-level paths, so any new top-level file or directory was silently
ignored until someone added it to the list (#379). Even tracked files such
as SECURITY.md and .gitattributes matched it.
"""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# New source a contributor could add at the top level or in a new directory.
NEW_SOURCE = (
    "NEW_FILE.md",
    ".new-config.toml",
    "newdir/main.rs",
    "rust/new-crate/src/lib.rs",
    "tools/new_tool.py",
)

# Build output and local files that must stay ignored.
GENERATED = (
    "ci-evidence-1694/setup-observations.json",
    "docs/ci-evidence/2026-10-22-ubuntu-cache-allocation.json",
    "build/CMakeCache.txt",
    "build_x64/obs.sln",
    "target/debug/obs",
    "dylints/ban_platform_cfg_outside_boundary/target/debug/x",
    "ci/__pycache__/x.cpython-312.pyc",
    "docs/sphinx/_build/html/index.html",
    "cmake/.CMakeBuildNumber",
    ".DS_Store",
    "CMakeUserPresets.json",
)


def ignored(paths: tuple[str, ...]) -> list[str]:
    out = subprocess.run(
        ["git", "check-ignore", "--no-index", "--verbose", "--stdin"],
        cwd=ROOT,
        input="\n".join(paths) + "\n",
        capture_output=True,
        text=True,
        check=False,
    )
    # "<source>:<line>:<pattern>\t<path>"; a path whose last match is a
    # negated pattern (!foo) is re-included, not ignored.
    result = []
    for line in out.stdout.splitlines():
        match, path = line.split("\t", 1)
        if not match.split(":", 2)[2].startswith("!"):
            result.append(path)
    return result


class GitignoreTest(unittest.TestCase):
    def test_ci_evidence_dumps_are_retired_from_the_tree(self):
        """#2686: the run-metadata snapshots were 185 tracked files (11.6 MiB).

        They are measurement output, not source, and the CI attestations live
        in commit-message trailers. The directory may come back as untracked
        scratch from a local run, so it stays ignored -- but nothing under it
        may be tracked again.
        """
        tracked = subprocess.run(
            ["git", "ls-files", "docs/ci-evidence"],
            cwd=ROOT, capture_output=True, text=True, check=True,
        ).stdout.split()
        self.assertEqual(tracked, [])

    def test_tracked_files_are_not_ignored(self):
        tracked = subprocess.run(
            ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
        ).stdout.split("\n")
        tracked = tuple(p for p in tracked if p)
        self.assertEqual(ignored(tracked), [])

    def test_new_source_is_not_ignored(self):
        self.assertEqual(ignored(NEW_SOURCE), [])

    def test_generated_files_are_ignored(self):
        self.assertEqual(sorted(ignored(GENERATED)), sorted(GENERATED))


if __name__ == "__main__":
    unittest.main()
