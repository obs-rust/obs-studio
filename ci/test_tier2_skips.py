#!/usr/bin/env -S uv run --script
"""Tests for rust/tools/linux-validate/check-skips.sh (obs-rust/obs-studio#776).

    uv run python -m unittest ci/test_tier2_skips.py
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHECK = ROOT / "rust" / "tools" / "linux-validate" / "check-skips.sh"

SKIPPED_LOG = """\
129/200 Test #129: test_pipewire_camera_registry_race ...***Skipped   0.01 sec
130/200 Test #130: test_core_module_load ................   Passed    0.40 sec
131/200 Test #131: test_pipewire_camera_unload ..........***Skipped   0.01 sec
"""
INHIBIT_SKIPPED_LOG = """\
140/200 Test #140: test_inhibit_sleep_dbus ..............***Skipped   0.01 sec
141/200 Test #141: test_inhibit_sleep ...................   Passed    0.02 sec
"""
PASSED_LOG = """\
129/200 Test #129: test_pipewire_camera_registry_race ...   Passed    0.39 sec
130/200 Test #130: test_svt_av1 .........................***Skipped   0.01 sec
"""


class CheckTier2SkipsTest(unittest.TestCase):
    def run_check(self, log: str):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "ctest.log"
            p.write_text(log)
            return subprocess.run(["bash", str(CHECK), str(p)], capture_output=True, text=True)

    def test_skipped_pipewire_test_fails(self):
        r = self.run_check(SKIPPED_LOG)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("test_pipewire_camera_registry_race", r.stdout)
        self.assertIn("test_pipewire_camera_unload", r.stdout)
        self.assertIn("#776", r.stdout)

    def test_skipped_inhibit_sleep_dbus_fails(self):
        # Sleep inhibition through the portal and session D-Bus was once
        # lost without a failing test (#453).
        r = self.run_check(INHIBIT_SKIPPED_LOG)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("test_inhibit_sleep_dbus", r.stdout)

    def test_passed_pipewire_and_other_skips_pass(self):
        r = self.run_check(PASSED_LOG)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)


if __name__ == "__main__":
    unittest.main()
