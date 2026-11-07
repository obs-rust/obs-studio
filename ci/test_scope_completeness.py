"""Completeness ratchet for the source -> lane map (ci_scope.py).

A change that selects no lane is silent under-invalidation: CI stays green on a
tree it never rebuilt. This test walks every tracked build input and requires
each one to select at least one lane, with an explicit, shrink-only exemption
list for files that genuinely drive no lane (packaging scripts with their own
workflow, cosmetic dotfiles).
"""

from fnmatch import fnmatch
from pathlib import Path
import subprocess
import sys

import pytest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "ci"))

import ci_scope  # noqa: E402

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


def _exemptions() -> list[str]:
    out: list[str] = []
    for raw in EXEMPTIONS_PATH.read_text(encoding="utf-8").splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            out.append(line)
    return out


def _tracked() -> list[str]:
    return subprocess.run(
        ["git", "ls-files"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.splitlines()


def _lanes(path: str) -> set[str]:
    scope = ci_scope.scope(
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
    for name, value in (
        ("tier2", scope.tier2),
        ("core", scope.core),
        ("macos", scope.macos),
        ("windows", scope.windows),
        ("qt", scope.qt or scope.qt_sdk),
        ("linux", bool(scope.linux_targets)),
        ("asan", scope.asan),
        ("cross", bool(scope.cross_targets)),
    ):
        if value:
            hit.add(name)
    return hit


def test_every_build_input_selects_a_lane() -> None:
    patterns = _exemptions()
    exempt = {path for path in _tracked() if any(fnmatch(path, pat) for pat in patterns)}
    uncovered = [
        path
        for path in _tracked()
        if path.startswith(BUILD_INPUT_ROOTS)
        and path not in exempt
        and not _lanes(path)
    ]
    assert not uncovered, (
        "these build inputs select no CI lane (silent under-invalidation); "
        "add a rule to ci_scope.py or an explicit line to ci/scope_exemptions.txt:\n  "
        + "\n  ".join(uncovered[:40])
    )


def test_exemptions_still_exist() -> None:
    """A stale exemption hides a rule that was deleted - keep the list honest."""
    tracked = set(_tracked())
    stale = [pat for pat in _exemptions() if not any(fnmatch(p, pat) for p in tracked)]
    assert not stale, f"scope_exemptions.txt lists files that no longer exist: {stale}"


def test_exemptions_are_still_uncovered() -> None:
    """If a rule now covers an exempted file, drop the exemption."""
    covered = [pat for pat in _exemptions() if any(fnmatch(p, pat) and _lanes(p) for p in _tracked())]
    assert not covered, (
        "these are exempted but now select a lane - remove them from "
        f"scope_exemptions.txt: {covered}"
    )


@pytest.mark.parametrize(
    "path,expected",
    [
        # A CMake module every target includes: all platforms and Linux Tier 2.
        ("cmake/Modules/ObsHelpers.cmake", {"macos", "windows", "tier2"}),
        # OpenGL is compiled on Linux and Windows; the Linux Tier 2 build links it.
        ("libobs-opengl/CMakeLists.txt", {"tier2", "windows"}),
        ("libobs-metal/CMakeLists.txt", {"macos"}),
        ("libobs-winrt/CMakeLists.txt", {"windows"}),
        ("plugins/CMakeLists.txt", {"macos", "windows"}),
    ],
)
def test_tree_rules_select_the_compiling_lanes(path: str, expected: set[str]) -> None:
    """Regression pin for the trees whose names carry no OS token."""
    assert expected <= _lanes(path), f"{path} selects {_lanes(path)}, want {expected}"
