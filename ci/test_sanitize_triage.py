#!/usr/bin/env -S uv run --script
"""rust/tools/sanitize/triage.py: LeakSanitizer reports.

    uv run python -m unittest ci/test_sanitize_triage.py

A leak allocated inside a library's one-time initialization that was unloaded
before LeakSanitizer reported (every frame above OBS's own is
`(<unknown module>)` or pthread_once; the OpenGL driver under gs_create) is
not an OBS finding: it is listed as external, not counted. Each leak in a
report is triaged on its own, so a real OBS leak next to it still counts.

Exit 77 is ctest's skip code, not a failure. A test in KNOWN is listed, not
counted: its exit and its sanitizer reports. The list keeps itself honest
(#3238): every bug or flaky entry names its issue, a known bug that ran
clean is a finding (the fix must drop its entry), a flaky entry that ran
clean is listed, and the known list comes first in the summary.
"""

import importlib.util
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TRIAGE = ROOT / "rust/tools/sanitize/triage.py"

DRIVER_LEAK = """\
Direct leak of 128 byte(s) in 1 object(s) allocated from:
    #0 0x78f44f328e9f in realloc ../../../../src/libsanitizer/asan/asan_malloc_linux.cpp:81
    #1 0x74f439a65216  (<unknown module>)
    #2 0x78f44d8b14bb in __pthread_once_slow nptl/pthread_once.c:116
    #3 0x78f44d8b1538 in ___pthread_once nptl/pthread_once.c:143
    #4 0x74f43c3335aa  (<unknown module>)
    #5 0x78f44e97c5f2 in gs_create /home/runner/work/obs-studio/obs-studio/libobs/graphics/graphics.c.h:232
    #6 0x78f44e804248 in obs_init_graphics /home/runner/work/obs-studio/obs-studio/libobs/obs.c.h:234

"""
OBS_LEAK = """\
Direct leak of 64 byte(s) in 1 object(s) allocated from:
    #0 0x7b7254b29ddf in malloc ../../../../src/libsanitizer/asan/asan_malloc_linux.cpp:67
    #1 0x7b725428f712 in bmalloc (/build/libobs/libobs.so.30+0x68f712)
    #2 0x7b72540e7b16 in obs_canvas_insert_source /home/runner/work/obs-studio/obs-studio/libobs/obs-canvas.c.h:330

"""
HEADER = "=================================================================\n==1==ERROR: LeakSanitizer: detected memory leaks\n\n"


GLIB_RACE = """\
==================
WARNING: ThreadSanitizer: data race (pid=1)
  Write of size 8 at 0x72080000b280 by thread T4:
    #0 free ../../../../src/libsanitizer/tsan/tsan_interceptors_posix.cpp:735 (libtsan.so.2+0x8ccc3)
    #1 <null> <null> (libglib-2.0.so.0+0x5e9c2)

  Previous write of size 8 at 0x72080000b280 by thread T1:
    #0 malloc ../../../../src/libsanitizer/tsan/tsan_interceptors_posix.cpp:670 (libtsan.so.2+0x8d41c)
    #1 g_malloc <null> (libglib-2.0.so.0+0x679a9)

SUMMARY: ThreadSanitizer: data race (/usr/lib/x86_64-linux-gnu/libglib-2.0.so.0+0x5e9c2)
==================
"""
OBS_RACE = """\
==================
WARNING: ThreadSanitizer: data race (pid=1)
  Read of size 8 at 0x7208000a0000 by thread T2:
    #0 obs_source_video_tick /home/runner/work/obs-studio/obs-studio/libobs/obs-source.c.h:1409 (libobs.so.30+0x1)

  Previous write of size 8 at 0x7208000a0000 by thread T1:
    #0 free ../../../../src/libsanitizer/tsan/tsan_interceptors_posix.cpp:735 (libtsan.so.2+0x8ccc3)
    #1 <null> <null> (libglib-2.0.so.0+0x5e9c2)

SUMMARY: ThreadSanitizer: data race libobs/obs-source.c.h:1409 in obs_source_video_tick
==================
"""


def triage(*leaks, tool="asan", text=None, test="test_x", summary=None):
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / tool
        (out / "tests").mkdir(parents=True)
        (out / "tests" / "summary.txt").write_text(summary if summary is not None else f"{test} exit=0\n")
        if leaks or text is not None:
            (out / "tests" / f"{test}.{tool}.1").write_text(text if text is not None else HEADER + "".join(leaks))
        run = subprocess.run(
            ["uv", "run", "--no-project", str(TRIAGE), str(out), "--fail-on-findings"],
            capture_output=True, text=True,
        )
        return run.returncode, run.stdout


class LeakTriageTest(unittest.TestCase):
    def test_unloaded_library_init_leak_is_external(self):
        code, md = triage(DRIVER_LEAK)
        self.assertEqual(code, 0, md)
        self.assertIn("External", md)
        self.assertIn("libobs/graphics/graphics.c.h:232", md)

    def test_obs_leak_next_to_it_still_counts(self):
        code, md = triage(DRIVER_LEAK, OBS_LEAK)
        self.assertEqual(code, 1, md)
        self.assertIn("libobs/obs-canvas.c.h:330", md)


class RaceTriageTest(unittest.TestCase):
    """A race with no OBS frame on either side (uninstrumented glib, Mesa) is
    listed as external; one OBS side is enough to count it."""

    def test_race_entirely_outside_obs_is_external(self):
        code, md = triage(tool="tsan", text=GLIB_RACE)
        self.assertEqual(code, 0, md)
        self.assertIn("External", md)

    def test_race_with_one_obs_side_counts(self):
        code, md = triage(tool="tsan", text=GLIB_RACE + OBS_RACE)
        self.assertEqual(code, 1, md)
        self.assertIn("libobs/obs-source.c.h:1409", md)


class SkipAndKnownTest(unittest.TestCase):
    """test_media_remux and test_dock_proportions are KNOWN under asan."""

    def test_skip_exit_is_not_a_finding(self):
        code, md = triage(summary="test_pipewire_camera_nv12 exit=77\n")
        self.assertEqual(code, 0, md)
        self.assertIn("skipped: test_pipewire_camera_nv12", md)
        self.assertIn("0/0 clean", md)

    def test_known_test_failure_is_listed_not_counted(self):
        code, md = triage(summary="test_dock_proportions exit=6\ntest_y exit=0\n")
        self.assertEqual(code, 0, md)
        self.assertIn("1/1 clean", md)
        self.assertIn("**test_dock_proportions**", md)

    def test_known_test_reports_are_listed_not_counted(self):
        code, md = triage(OBS_LEAK, test="test_media_remux")
        self.assertEqual(code, 0, md)
        self.assertNotIn("libobs/obs-canvas.c.h:330", md)
        self.assertIn("**test_media_remux**", md)

    def test_known_test_that_ran_clean_is_listed(self):
        code, md = triage(summary="test_dock_proportions exit=0\n")
        self.assertEqual(code, 0, md)
        self.assertIn("Known entries that ran clean", md)
        self.assertIn("test_dock_proportions", md)

    def test_known_bug_that_ran_clean_is_a_finding(self):
        """test_media_remux is a known bug: clean means fixed, so the entry
        has to go, and the job says so instead of passing."""
        code, md = triage(summary="test_media_remux exit=0\n")
        self.assertEqual(code, 1, md)
        self.assertIn("ran clean: drop the entry", md)
        self.assertIn("test_media_remux", md)

    def test_known_list_comes_first(self):
        code, md = triage(summary="test_dock_proportions exit=6\ntest_y exit=1\n")
        self.assertEqual(code, 1, md)
        self.assertLess(md.index("#### Known"), md.index("#### Unit tests with a non-zero exit"), md)

    def test_unknown_test_failure_still_counts(self):
        code, md = triage(summary="test_y exit=1\n")
        self.assertEqual(code, 1, md)


class KnownListTest(unittest.TestCase):
    """Every entry has a kind; bug and flaky entries name an open issue."""

    def test_every_entry_has_a_kind_and_an_issue(self):
        spec = importlib.util.spec_from_file_location("triage", TRIAGE)
        triage_mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(triage_mod)
        for (tool, test), (kind, why) in triage_mod.KNOWN.items():
            self.assertIn(kind, {"bug", "flaky", "tool"}, (tool, test))
            if kind != "tool":
                self.assertRegex(why, r"#\d+", (tool, test))


if __name__ == "__main__":
    unittest.main()
