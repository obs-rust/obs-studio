#!/usr/bin/env -S uv run --script
"""The conversion ratchet, read from the targets' own CMakeLists (#2625, #2923).

    uv run python -m unittest ci/test_conversion_ratchet.py

A target joins the ratchet by naming a conversion group in its own
`target_compile_options()`. Only the positive spelling counts: `-Wno-...` keeps
the global pin and is the shape every target has before it is converted. The
directories the ratchet hands `ci/ci_scope.py` decide which PRs run the asan lane.
"""

from __future__ import annotations

import unittest
from pathlib import Path

from conversion_ratchet import groups, hit, ratcheted_dirs, repo_ratcheted_dirs

ROOT = Path(__file__).resolve().parent.parent
RATCHETED = 'target_compile_options(foo PRIVATE "$<$<COMPILE_LANG_AND_ID:CXX,GNU>:-Wsign-conversion>")\n'
PINNED = 'target_compile_options(foo PRIVATE "$<$<COMPILE_LANG_AND_ID:CXX,GNU>:-Wno-sign-conversion>")\n'
ALL_THREE = (
    'target_compile_options(\n  foo\n  PRIVATE "$<$<COMPILE_LANG_AND_ID:C,GNU>'
    ':-Wconversion;-Wsign-conversion;-Wfloat-conversion>"\n)\n'
)


class ConversionRatchetTest(unittest.TestCase):
    def test_a_positive_group_is_a_ratchet_entry(self) -> None:
        self.assertEqual(groups(RATCHETED), {"-Wsign-conversion"})
        self.assertEqual(groups(ALL_THREE), {"-Wconversion", "-Wsign-conversion", "-Wfloat-conversion"})

    def test_a_pin_is_not_a_ratchet_entry(self) -> None:
        # Every target keeps its pins until it is converted; the ratchet is the
        # target that dropped one.
        self.assertEqual(groups(PINNED), set())
        self.assertEqual(ratcheted_dirs({"plugins/foo/CMakeLists.txt": PINNED}), ())

    def test_the_directory_of_the_cmake_lists_is_the_target(self) -> None:
        lists = {"frontend/CMakeLists.txt": RATCHETED, "plugins/obs-x264/CMakeLists.txt": ALL_THREE}
        self.assertEqual(ratcheted_dirs(lists), ("frontend", "plugins/obs-x264"))

    def test_the_root_cmake_lists_is_the_repository_root(self) -> None:
        self.assertEqual(ratcheted_dirs({"CMakeLists.txt": RATCHETED}), ("",))

    def test_only_compile_inputs_below_a_ratcheted_target_hit(self) -> None:
        dirs = ("frontend", "libobs")
        self.assertTrue(hit("frontend/components/OBSAdvAudioCtrl.cpp.hpp", dirs))
        self.assertTrue(hit("libobs/obs.c.h", dirs))
        self.assertFalse(hit("frontend/CMakeLists.txt", dirs))
        self.assertFalse(hit("docs/rust-port/naming.md", dirs))
        # A target's sibling with a shared prefix is a different directory.
        self.assertFalse(hit("frontend-tools/foo.cpp", dirs))

    def test_the_repository_ratchet_is_read_from_the_tree(self) -> None:
        dirs = repo_ratcheted_dirs(ROOT)
        # #2938: the app target opts into one group, so the frontend is the
        # target whose PRs must run the sanitizer lane until it is converted.
        self.assertIn("frontend", dirs)
        # #2865, #2872: ratcheted plugin targets.
        self.assertIn("plugins/obs-ffmpeg", dirs)
        self.assertIn("plugins/obs-outputs", dirs)
        # Vendored upstream trees never ratchet, and no path is absolute.
        self.assertFalse([directory for directory in dirs if directory.startswith("deps/")])


if __name__ == "__main__":
    unittest.main()
