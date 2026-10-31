#!/usr/bin/env -S uv run --script
"""The clang replay of the conversion ratchet (#3118).

    uv run python -m unittest ci/test_conversion_clang_replay.py

The replay reads a configured tree, so these tests pin the parts that decide
what it checks rather than its numbers: the flags it swaps (the pins out, the
conversion group in, nothing written), which groups it replays (unity groups,
addressed by the directory the ratchet names), which diagnostics count (the
conversion family only, deduped, inside the tree), and that a ratcheted
directory the configuration did not compile is reported instead of passing.
"""

from __future__ import annotations

import unittest
from pathlib import Path

from conversion_clang_replay import (
    clang_command,
    conversion_diagnostics,
    exit_status,
    first_error,
    group_directory,
    Replay,
    repo_compdb,
    uncovered,
    unexplained,
    unity_group,
)
from conversion_ratchet import repo_ratcheted_dirs

ROOT = Path(__file__).resolve().parent.parent

ENTRY = {
    "directory": "/tree/build",
    "command": (
        "/usr/bin/cc -DLIBOBS -I/tree/build/libobs -I/tree/libobs "
        '-DOBS_INSTALL_PREFIX="/usr/local" -Werror -Wno-conversion '
        "-Wno-sign-conversion -Wno-float-conversion -Wno-implicit-fallthrough "
        "-MD -MT CMakeFiles/libobs.dir/build/libobs.core+.h.c.o "
        "-MF CMakeFiles/libobs.dir/build/libobs.core+.h.c.o.d "
        "-o CMakeFiles/libobs.dir/build/libobs.core+.h.c.o "
        "-c /tree/libobs/build/libobs.core+.h.c"
    ),
    "file": "/tree/libobs/build/libobs.core+.h.c",
    "output": "/tree/build/CMakeFiles/libobs.dir/build/libobs.core+.h.c.o",
}


class ClangCommandTest(unittest.TestCase):
    def test_the_pins_come_out_and_the_group_goes_in(self) -> None:
        command = clang_command(ENTRY)
        self.assertEqual(command[0], "clang")
        for pin in ("-Wno-conversion", "-Wno-sign-conversion", "-Wno-float-conversion"):
            self.assertNotIn(pin, command)
        for warning in ("-Wconversion", "-Wsign-conversion", "-Wfloat-conversion"):
            self.assertIn(warning, command)

    def test_a_compiler_launcher_is_not_the_compiler(self) -> None:
        # The sanitizer tree configures zccache as the launcher, so the entry
        # reads "<launcher> <compiler> ...": swapping only the launcher would
        # hand clang the real compiler path as an input file.
        launched = {**ENTRY, "command": ENTRY["command"].replace("/usr/bin/cc", "zccache /usr/bin/cc", 1)}
        command = clang_command(launched)
        self.assertEqual(command[0], "clang")
        self.assertNotIn("/usr/bin/cc", command[:2])
        self.assertNotIn("zccache", command)

    def test_werror_is_dropped_so_every_site_is_reported(self) -> None:
        self.assertNotIn("-Werror", clang_command(ENTRY))

    def test_nothing_is_written(self) -> None:
        command = clang_command(ENTRY)
        self.assertIn("-fsyntax-only", command)
        self.assertNotIn("-c", command)
        self.assertNotIn("-o", command)
        self.assertNotIn(ENTRY["output"], command)
        self.assertNotIn("-MF", command)
        self.assertNotIn("-MD", command)

    def test_the_targets_own_flags_and_includes_stay(self) -> None:
        command = clang_command(ENTRY)
        self.assertIn("-DLIBOBS", command)
        self.assertIn("-I/tree/libobs", command)
        self.assertIn("-Wno-implicit-fallthrough", command)
        self.assertEqual(command[-1], ENTRY["file"])

    def test_the_groups_policy_is_the_group_not_the_compiler(self) -> None:
        # The same command replayed by gcc is the control the cost proof used.
        self.assertEqual(clang_command(ENTRY, "gcc")[0], "gcc")


class GroupSelectionTest(unittest.TestCase):
    def test_only_a_unity_group_is_replayed(self) -> None:
        self.assertTrue(unity_group(ENTRY))
        self.assertFalse(unity_group({**ENTRY, "file": "/tree/libobs/obs.c"}))

    def test_the_directory_is_the_one_the_ratchet_names(self) -> None:
        root = Path("/tree")
        self.assertEqual(group_directory(ENTRY, root), "libobs")
        plugin = {
            **ENTRY,
            "file": "/tree/plugins/obs-ffmpeg/build/obs-ffmpeg.core+.h.c",
        }
        self.assertEqual(group_directory(plugin, root), "plugins/obs-ffmpeg")

    def test_a_ratcheted_directory_without_a_group_is_uncovered(self) -> None:
        self.assertEqual(
            uncovered(("libobs", "plugins/obs-ffmpeg"), {"libobs"}),
            ("plugins/obs-ffmpeg",),
        )
        self.assertEqual(uncovered(("libobs",), {"libobs"}), ())

    def test_the_repo_ratchet_is_what_the_replay_is_measured_against(self) -> None:
        # The replay's coverage set is the ratchet's own list, never a copy:
        # the PR that opts a target in changes that CMakeLists and nothing here.
        ratcheted = repo_ratcheted_dirs(ROOT)
        self.assertIn("libobs", ratcheted)
        self.assertIn("plugins/obs-ffmpeg", ratcheted)


class DiagnosticsTest(unittest.TestCase):
    ROOT = Path("/tree")

    def test_a_conversion_warning_counts_with_its_source_path(self) -> None:
        stderr = (
            "/tree/libobs/obs-source.c:12:9: warning: implicit conversion loses "
            "integer precision: 'long long' to 'int' [-Wshorten-64-to-32]\n"
        )
        self.assertEqual(
            conversion_diagnostics(stderr, self.ROOT),
            ["libobs/obs-source.c:12:9: implicit conversion loses integer "
             "precision: 'long long' to 'int'"],
        )

    def test_a_replay_artifact_does_not_count(self) -> None:
        stderr = (
            "clang: warning: argument unused during compilation: '-L/tree/lib' "
            "[-Wunused-command-line-argument]\n"
            "/tree/libobs/obs-source.c:12:9: warning: unused variable 'x' "
            "[-Wunused-variable]\n"
        )
        self.assertEqual(conversion_diagnostics(stderr, self.ROOT), [])

    def test_a_finding_in_a_dependency_does_not_count(self) -> None:
        stderr = (
            "/deps/simde/x86/sse2.h:40:2: warning: implicit conversion changes "
            "signedness: 'int' to 'unsigned int' [-Wsign-conversion]\n"
        )
        self.assertEqual(conversion_diagnostics(stderr, self.ROOT), [])

    def test_a_unity_group_reporting_one_site_twice_reports_it_once(self) -> None:
        line = (
            "/tree/libobs/obs-source.c:12:9: warning: implicit conversion changes "
            "signedness: 'int' to 'uint32_t' [-Wsign-conversion]\n"
        )
        self.assertEqual(len(conversion_diagnostics(line + line, self.ROOT)), 1)


class ReplayFailureTest(unittest.TestCase):
    def test_a_group_that_did_not_parse_is_not_a_clean_group(self) -> None:
        stderr = "/tree/libobs/obs.c:3:10: error: 'simde/simd.h' file not found\n"
        self.assertIn("'simde/simd.h' file not found", first_error(stderr))

    def test_a_failed_replay_is_reported(self) -> None:
        broken: Replay = {
            "group": "libobs.core+.h.c",
            "directory": "libobs",
            "seconds": 0.1,
            "returncode": 1,
            "diagnostics": [],
            "failure": "clang: error: no such file or directory",
        }
        found: Replay = {**broken, "failure": "", "diagnostics": ["libobs/a.c:1:2: x"]}
        self.assertEqual(unexplained([broken, found]), [broken])


class ExitStatusTest(unittest.TestCase):
    FINDING: Replay = {
        "group": "libobs.core+.h.c",
        "directory": "libobs",
        "seconds": 0.1,
        "returncode": 1,
        "diagnostics": ["libobs/a.c.h:1:2: implicit conversion"],
        "failure": "",
    }
    BROKEN: Replay = {
        "group": "obs-filters.core+.h.c",
        "directory": "plugins/obs-filters",
        "seconds": 0.1,
        "returncode": 1,
        "diagnostics": [],
        "failure": "clang: error: 'X11/X.h' file not found",
    }

    def test_a_clean_replay_passes(self) -> None:
        self.assertEqual(exit_status([], (), []), 0)

    def test_a_finding_is_red(self) -> None:
        self.assertEqual(exit_status([self.FINDING], (), []), 1)

    def test_a_group_that_did_not_parse_is_red(self) -> None:
        self.assertEqual(exit_status([], (), [self.BROKEN]), 1)

    def test_an_uncovered_directory_is_red_by_default(self) -> None:
        # A full app tree must compile every ratcheted target.
        self.assertEqual(exit_status([], ("frontend",), []), 1)

    def test_a_lane_tree_may_leave_ratcheted_targets_unbuilt(self) -> None:
        # The sanitizer tree turns vlc, vst, webrtc and decklink off, so the
        # lane reports those as unbuilt rather than red (#3118).
        self.assertEqual(exit_status([], ("plugins/vlc-video",), [], allow_uncovered=True), 0)
        # It still fails on a real finding.
        self.assertEqual(exit_status([self.FINDING], ("plugins/vlc-video",), [], allow_uncovered=True), 1)


class CompileDatabaseTest(unittest.TestCase):
    def test_an_unreadable_database_is_an_error_not_a_clean_run(self) -> None:
        with self.assertRaises(OSError):
            repo_compdb(Path("/nonexistent/compile_commands.json"))


if __name__ == "__main__":
    unittest.main()
