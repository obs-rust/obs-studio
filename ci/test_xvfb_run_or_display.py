#!/usr/bin/env -S uv run --script
"""The private Xvfb that cmocka display tests start must not reset.

    uv run python -m unittest ci/test_xvfb_run_or_display.py

test/cmocka/xvfb-run-or-display.sh starts its own Xvfb when a test runs with
no DISPLAY (a local `ctest -R ...`). Xvfb resets when its last client
disconnects, unless it runs with -noreset. A test that calls obs_startup and
obs_shutdown once per case (test_audio_controls) closes libobs's only X
connection at each shutdown, and the next case's XOpenDisplay races that
reset: obs_startup then fails ("XOpenDisplay failed during hotkey
initialization") and cmocka reports "Test setup failed".
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEMPLATE = ROOT / "test" / "cmocka" / "xvfb-run-or-display.sh.in"
DEFAULT = "-screen 0 1280x1024x24 -noreset"


@unittest.skipUnless(shutil.which("sh"), "sh required")
class XvfbRunOrDisplayTest(unittest.TestCase):
    def setUp(self):
        self.directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.directory)
        self.argv = self.directory / "argv"
        stub = self.directory / "xvfb-run"
        # Records the arguments the real xvfb-run would get.
        stub.write_text('#!/bin/sh\nfor a in "$@"; do printf "%s\\n" "$a"; done > "$ARGV_FILE"\n')
        stub.chmod(0o755)
        self.wrapper = self.directory / "xvfb-run-or-display.sh"
        self.wrapper.write_text(TEMPLATE.read_text().replace("@OBS_XVFB_RUN_REAL@", str(stub)))
        self.wrapper.chmod(0o755)

    def run_wrapper(self, *args, display=None):
        env = {k: v for k, v in os.environ.items() if k != "DISPLAY"}
        env["ARGV_FILE"] = str(self.argv)
        if display:
            env["DISPLAY"] = display
        return subprocess.run(["sh", str(self.wrapper), *args], env=env, capture_output=True, text=True, check=True)

    def started_with(self, *args):
        self.run_wrapper(*args)
        return self.argv.read_text().splitlines()

    def test_private_server_gets_noreset_by_default(self):
        self.assertEqual(self.started_with("-a", "/bin/true", "x"), ["-a", "-s", DEFAULT, "/bin/true", "x"])

    def test_given_server_args_get_noreset(self):
        self.assertEqual(
            self.started_with("-a", "-s", "-screen 0 640x480x24", "/bin/true"),
            ["-a", "-s", "-screen 0 640x480x24 -noreset", "/bin/true"],
        )
        self.assertEqual(
            self.started_with("-a", "--server-args=-screen 0 640x480x24", "/bin/true"),
            ["-a", "--server-args=-screen 0 640x480x24 -noreset", "/bin/true"],
        )

    def test_server_args_with_noreset_are_unchanged(self):
        self.assertEqual(self.started_with("-a", "-s", DEFAULT, "/bin/true"), ["-a", "-s", DEFAULT, "/bin/true"])

    def test_command_arguments_are_not_options(self):
        self.assertEqual(
            self.started_with("-a", "--", "/bin/echo", "-s", "x"),
            ["-a", "-s", DEFAULT, "--", "/bin/echo", "-s", "x"],
        )

    def test_existing_display_runs_the_command_directly(self):
        run = self.run_wrapper("-a", "-s", "-screen 0 640x480x24", "/bin/echo", "ran", display=":99")
        self.assertEqual(run.stdout, "ran\n")
        self.assertFalse(self.argv.exists())


if __name__ == "__main__":
    unittest.main()
