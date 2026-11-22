#!/usr/bin/env -S uv run --script
"""Every dock kind is set up through one path (#5471).

Usage:
    uv run python -m unittest ci/test_dock_setup.py

The main window had a different setup for each dock kind: a `SETUP_DOCK`
macro for the main docks, `AddDockWidget()` for plugin docks and
`AddCustomDockWidget()` for custom docks. The feature set
`lock ? NoDockWidgetFeatures : Closable | Movable | Floatable` was written out
five times. Plugin and custom docks were kept in two pairs of parallel lists
(names and docks, kept in step by index), and `IsDockObjectNameUsed()` typed
the main dock names out again.

Since #5471, `DockFeatures()` (frontend/utility/ExtraDocks) is the only
feature set, `ExtraDocks` is the only list of plugin and custom docks, and the
main docks are listed once in OBSBasic. The behavior is tested in
`test_dock_state_restart`; this pins the structure.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRONTEND = ROOT / "frontend"
EXTRA_DOCKS = FRONTEND / "utility" / "ExtraDocks.cpp.hpp"
OBS_BASIC_HPP = FRONTEND / "widgets" / "OBSBasic.hpp"
OBS_BASIC = FRONTEND / "widgets" / "OBSBasic.cpp.hpp"
OBS_BASIC_DOCKS = FRONTEND / "widgets" / "OBSBasic_Docks.cpp.hpp"

# A dock feature set written out (the features DockFeatures() returns).
FEATURE_SET = re.compile(r"\bDockWidget(Movable|Floatable)\b|\bNoDockWidgetFeatures\b")
# The parallel lists that ExtraDocks replaces.
PARALLEL_LISTS = re.compile(r"\b(extraDockNames|extraCustomDockNames|extraCustomDocks)\b")
# A main dock's object name typed out as a string.
MAIN_DOCK_NAME = re.compile(r"\"(scenes|sources|mixer|transitions|controls|stats)Dock\"")


def frontend_sources():
    for path in sorted(FRONTEND.rglob("*")):
        if path.suffix in (".hpp", ".cpp", ".h"):
            yield path


def matches(pattern, paths):
    return [
        f"{path.relative_to(ROOT)}:{number}: {line.strip()}"
        for path in paths
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1)
        if pattern.search(line)
    ]


class DockSetupTest(unittest.TestCase):
    def test_one_feature_set(self):
        """Only DockFeatures() writes out a dock feature set."""
        others = [path for path in frontend_sources() if path != EXTRA_DOCKS]
        self.assertEqual(matches(FEATURE_SET, others), [], "dock features written outside DockFeatures()")
        self.assertTrue(EXTRA_DOCKS.exists(), "frontend/utility/ExtraDocks.cpp.hpp is missing")
        self.assertIn("DockFeatures(", OBS_BASIC_DOCKS.read_text(encoding="utf-8"))

    def test_no_setup_dock_macro(self):
        """The main docks are set up from one list, not a macro."""
        self.assertEqual(matches(re.compile(r"\bSETUP_DOCK\b"), [OBS_BASIC]), [])

    def test_one_list_of_extra_docks(self):
        """Plugin and custom docks are kept in ExtraDocks only."""
        self.assertEqual(matches(PARALLEL_LISTS, list(frontend_sources())), [])
        self.assertIn("ExtraDocks extraDocks;", OBS_BASIC_HPP.read_text(encoding="utf-8"))

    def test_main_dock_names_not_typed_again(self):
        """IsDockObjectNameUsed() takes the main dock names from the docks."""
        self.assertEqual(matches(MAIN_DOCK_NAME, [OBS_BASIC_DOCKS]), [])


if __name__ == "__main__":
    unittest.main()
