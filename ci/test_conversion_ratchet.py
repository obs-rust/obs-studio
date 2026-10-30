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

from conversion_ratchet import (
    groups,
    hit,
    interface_darwin_violations,
    ratcheted_dirs,
    repo_interface_darwin_violations,
    repo_ratcheted_dirs,
)

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
        self.assertEqual(
            groups(ALL_THREE),
            {"-Wconversion", "-Wsign-conversion", "-Wfloat-conversion"},
        )

    def test_a_pin_is_not_a_ratchet_entry(self) -> None:
        # Every target keeps its pins until it is converted; the ratchet is the
        # target that dropped one.
        self.assertEqual(groups(PINNED), set())
        self.assertEqual(ratcheted_dirs({"plugins/foo/CMakeLists.txt": PINNED}), ())

    def test_the_directory_of_the_cmake_lists_is_the_target(self) -> None:
        lists = {
            "frontend/CMakeLists.txt": RATCHETED,
            "plugins/obs-x264/CMakeLists.txt": ALL_THREE,
        }
        self.assertEqual(ratcheted_dirs(lists), ("frontend", "plugins/obs-x264"))

    def test_the_root_cmake_lists_is_the_repository_root(self) -> None:
        self.assertEqual(ratcheted_dirs({"CMakeLists.txt": RATCHETED}), ("",))

    def test_only_compile_inputs_below_a_ratcheted_target_hit(self) -> None:
        dirs = ("frontend", "libobs")
        self.assertTrue(hit("frontend/components/OBSAdvAudioCtrl.cpp.hpp", dirs))
        self.assertTrue(hit("libobs/obs.c.h", dirs))
        self.assertFalse(hit("docs/rust-port/naming.md", dirs))
        # A target's sibling with a shared prefix is a different directory.
        self.assertFalse(hit("frontend-tools/foo.cpp", dirs))

    def test_the_cmake_lists_that_turns_the_ratchet_on_hits(self) -> None:
        # #2956: the PR that opts a target into a group changes only its
        # CMakeLists, and the lane that reports that target's sanitizer-only
        # diagnostics is the one the change needs. A nested CMakeLists is a
        # different target (CMake adds a target's sources itself), and a
        # directory outside the ratchet opts nothing in.
        dirs = ("frontend", "plugins/decklink-captions")
        self.assertTrue(hit("plugins/decklink-captions/CMakeLists.txt", dirs))
        self.assertTrue(hit("frontend/CMakeLists.txt", dirs))
        self.assertFalse(hit("frontend/build/CMakeLists.txt", dirs))
        self.assertFalse(hit("plugins/obs-x264/CMakeLists.txt", dirs))

    def test_the_repository_ratchet_is_read_from_the_tree(self) -> None:
        dirs = repo_ratcheted_dirs(ROOT)
        # #2938: the app target opts into one group, so the frontend is the
        # target whose PRs must run the sanitizer lane until it is converted.
        self.assertIn("frontend", dirs)
        # #2865, #2872: ratcheted plugin targets.
        self.assertIn("plugins/obs-ffmpeg", dirs)
        self.assertIn("plugins/obs-outputs", dirs)
        # Vendored upstream trees never ratchet, and no path is absolute.
        self.assertFalse(
            [directory for directory in dirs if directory.startswith("deps/")]
        )


INTERFACE_BARE = (
    'target_compile_options(\n  properties-view\n  INTERFACE "$<$<COMPILE_LANG_AND_ID:CXX,GNU>'
    ':-Wconversion;-Wsign-conversion;-Wfloat-conversion>"\n)\n'
)
INTERFACE_GENEX = (
    'target_compile_options(\n  properties-view\n  INTERFACE "$<$<COMPILE_LANG_AND_ID:CXX,GNU>:'
    '$<$<NOT:$<PLATFORM_ID:Darwin>>:-Wconversion;-Wsign-conversion>>"\n)\n'
)
INTERFACE_BRANCH = (
    "if(OS_MACOS)\n"
    "  target_compile_options(properties-view INTERFACE -Wno-sign-conversion)\n"
    "else()\n" + INTERFACE_BARE + "endif()\n"
)


class InterfaceDarwinTest(unittest.TestCase):
    """A linked group reaches Apple TUs after the consumer's own pin (#3096)."""

    def test_a_bare_interface_group_is_a_violation(self) -> None:
        # The shape #3102 fixed: the consumer's `-Wno-sign-conversion` is stated
        # before the interface's positive groups, so `-Wconversion` re-enables the
        # sign diagnostics inside Qt 6.11's headers on the macOS app lane.
        self.assertEqual(
            interface_darwin_violations(
                {"shared/properties-view/CMakeLists.txt": INTERFACE_BARE}
            ),
            ("shared/properties-view/CMakeLists.txt: properties-view",),
        )

    def test_either_darwin_exclusion_clears_the_violation(self) -> None:
        self.assertEqual(
            interface_darwin_violations({"x/CMakeLists.txt": INTERFACE_GENEX}), ()
        )
        self.assertEqual(
            interface_darwin_violations({"x/CMakeLists.txt": INTERFACE_BRANCH}), ()
        )

    def test_a_comment_cannot_satisfy_the_check(self) -> None:
        commented = "# $<NOT:$<PLATFORM_ID:Darwin>>\n" + INTERFACE_BARE
        self.assertEqual(
            interface_darwin_violations({"x/CMakeLists.txt": commented}),
            ("x/CMakeLists.txt: properties-view",),
        )

    def test_a_pinned_interface_call_is_not_a_group(self) -> None:
        pinned = (
            "target_compile_options(properties-view INTERFACE -Wno-sign-conversion)\n"
        )
        self.assertEqual(interface_darwin_violations({"x/CMakeLists.txt": pinned}), ())

    def test_the_repository_has_no_reachable_interface_group(self) -> None:
        self.assertEqual(
            repo_interface_darwin_violations(ROOT),
            (),
            "an INTERFACE conversion group reaches every consumer's Apple TUs; "
            "exclude Darwin in the call, or state the group in the else() branch "
            "of an if(OS_MACOS) block (#3096)",
        )


if __name__ == "__main__":
    unittest.main()
