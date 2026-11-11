#!/usr/bin/env -S uv run --script
"""Completeness ratchet for the source -> lane map (ci_scope.py).

A change that selects no lane is silent under-invalidation: CI stays green on a
tree it never rebuilt. This walks every tracked build input and requires each
one to select at least one lane, with an explicit, shrink-only exemption list
for files that genuinely drive no lane (packaging scripts with their own
workflow, cosmetic dotfiles).

    cd ci && uv run python -m unittest test_scope_completeness.py
"""

from __future__ import annotations

import subprocess
import unittest
from fnmatch import fnmatch
from pathlib import Path

from ci_scope import scope

ROOT = Path(__file__).resolve().parent.parent
EXEMPTIONS_PATH = ROOT / "ci" / "scope_exemptions.txt"

# Trees whose files are build inputs. Everything here must select a lane.
BUILD_INPUT_ROOTS = (
    "cmake/",
    "libobs/",
    "libobs-opengl/",
    "libobs-metal/",
    "libobs-d3d11/",
    "libobs-winrt/",
    "deps/",
    "plugins/",
    "shared/",
    "frontend/",
    "UI/",
    "build-aux/",
)

LANE_FIELDS = (
    "tier2",
    "core",
    "macos",
    "windows",
    "qt",
    "qt_sdk",
    "linux_targets",
    "cross_targets",
)


def tracked() -> list[str]:
    return subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.splitlines()


def patterns() -> list[str]:
    out = []
    for raw in EXEMPTIONS_PATH.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            out.append(line)
    return out


def lanes_for(path: str) -> set[str]:
    result = scope(
        [path],
        exists=lambda p: (ROOT / p).exists(),
        read=lambda p: (ROOT / p).read_text(encoding="utf-8", errors="ignore")
        if (ROOT / p).exists()
        else "",
        cmocka_paths=(),
        event="pull_request",
        ref="refs/pull/1/head",
    )
    hit = set()
    for field in LANE_FIELDS:
        value = getattr(result, field)
        if value:
            hit.add(field)
    return hit


class ScopeCompletenessTest(unittest.TestCase):
    def test_every_build_input_selects_a_lane(self) -> None:
        exempt_patterns = patterns()
        uncovered = [
            path
            for path in tracked()
            if path.startswith(BUILD_INPUT_ROOTS)
            and not any(fnmatch(path, pattern) for pattern in exempt_patterns)
            and not lanes_for(path)
        ]
        self.assertEqual(
            uncovered[:40],
            [],
            "these build inputs select no CI lane (silent under-invalidation); "
            "add a rule in ci_scope.py or an explicit line in ci/scope_exemptions.txt",
        )

    def test_exemptions_still_exist(self) -> None:
        files = tracked()
        stale = [
            pattern
            for pattern in patterns()
            if not any(fnmatch(path, pattern) for path in files)
        ]
        self.assertEqual(stale, [], "scope_exemptions.txt has patterns matching nothing")

    def test_exemptions_are_still_uncovered(self) -> None:
        files = tracked()
        covered = [
            pattern
            for pattern in patterns()
            if any(fnmatch(path, pattern) and lanes_for(path) for path in files)
        ]
        self.assertEqual(
            covered,
            [],
            "these are exempted but now select a lane - remove them from "
            "ci/scope_exemptions.txt",
        )


class TreeRuleTest(unittest.TestCase):
    """Regression pin for trees whose names carry no OS token."""

    CASES = {
        # A CMake module every target includes: all platforms and Linux Tier 2.
        "cmake/Modules/ObsHelpers.cmake": {"macos", "windows", "tier2"},
        # OpenGL is compiled on Linux and Windows; Linux Tier 2 links it.
        "libobs-opengl/CMakeLists.txt": {"tier2", "windows"},
        "libobs-metal/CMakeLists.txt": {"macos"},
        "libobs-winrt/CMakeLists.txt": {"windows"},
        "plugins/CMakeLists.txt": {"macos", "windows"},
    }

    def test_tree_rules_select_the_compiling_lanes(self) -> None:
        for path, expected in self.CASES.items():
            with self.subTest(path=path):
                self.assertTrue(
                    expected <= lanes_for(path),
                    f"{path} selects {sorted(lanes_for(path))}, want {sorted(expected)}",
                )


if __name__ == "__main__":
    unittest.main()
