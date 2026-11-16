#!/usr/bin/env -S uv run --script
"""Keep MSVC's /O2 off test_video_stop_requested.c (obs-rust/obs-studio#4815).

MSVC 19.51 at /O2 rotates stop_all_combinations' mask loop: it reads the
bound before it computes `1U << count`, so `count = 0` uses an uninitialized
stack slot as the bound and every later count uses the previous one. The test
then fails with `1 != 0` at line 51 whenever that slot holds 2 or more, and
where it passes it checks only half the masks. /O1 compiles the loop
correctly (a standalone repro: /O2, /O2 /Ob1, /O2 /Ob2 wrong; /O1, /Od right).
The C test stays unchanged; both of its targets get /O1 on MSVC.
"""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ("test_video_stop_requested", "test_video_stop_requested_rust")


class VideoStopRequestedMsvc(unittest.TestCase):
    def test_both_targets_build_with_o1_on_msvc(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        for name in TARGETS:
            with self.subTest(target=name):
                options = graph["targets"][name].get("compile_options", [])
                self.assertIn({"name": "/O1", "requires": "MSVC"}, options)

    def test_the_c_test_keeps_its_loop(self):
        # The workaround is in the build, not in the unchanged C test.
        source = (ROOT / "test/cmocka/test_video_stop_requested.c").read_text()
        self.assertIn("for (unsigned mask = 0; mask < (1U << count); mask++)", source)


if __name__ == "__main__":
    unittest.main()
