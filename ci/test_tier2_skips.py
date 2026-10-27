#!/usr/bin/env -S uv run --script
"""check-skips fails when a Tier 2 test that must run was Skipped (#776), in Python.

The rule lives in rust/tools/linux-validate/check_skips.py since the port off the
shell script, and validate.sh runs it through `uv run --no-project`. This drives
the port's verdicts over canned ctest logs.

    uv run python -m unittest ci/test_tier2_skips.py
"""

from __future__ import annotations

import importlib.util
import io
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "rust" / "tools" / "linux-validate"
MODULE = TOOLS / "check_skips.py"
SHELL_SCRIPT = TOOLS / "check-skips.sh"

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


def load_module():
    """rust/tools/linux-validate/check_skips.py by path: `uv run python -m unittest
    ci/test_tier2_skips.py` does not put that directory on sys.path, and no file in
    this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("check_skips", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class CheckTier2SkipsTest(unittest.TestCase):
    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def invoke(self, log: str) -> tuple[int, str, str]:
        path = Path(self.tmp.name) / "ctest.log"
        path.write_text(log, encoding="utf-8")
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = self.module.main(["check_skips.py", str(path)])
        return code, out.getvalue(), err.getvalue()

    def test_skipped_pipewire_test_fails(self):
        code, out, err = self.invoke(SKIPPED_LOG)
        self.assertEqual(code, 1, out + err)
        self.assertIn("test_pipewire_camera_registry_race", out)
        self.assertIn("test_pipewire_camera_unload", out)
        self.assertIn("#776", out)
        self.assertIn("The harness image must provide pipewire", out)

    def test_skipped_inhibit_sleep_dbus_fails(self):
        # Sleep inhibition through the portal and session D-Bus was once
        # lost without a failing test (#453).
        code, out, err = self.invoke(INHIBIT_SKIPPED_LOG)
        self.assertEqual(code, 1, out + err)
        self.assertIn("test_inhibit_sleep_dbus", out)

    def test_passed_pipewire_and_other_skips_pass(self):
        code, out, err = self.invoke(PASSED_LOG)
        self.assertEqual(code, 0, out + err)
        self.assertEqual(out, "")

    def test_names_are_sorted_and_deduplicated(self):
        log = (
            "1/2 Test #1: test_vlc_player ............***Skipped   0.01 sec\n"
            "2/2 Test #2: test_pipewire_camera_registry_race ...***Skipped   0.01 sec\n"
            "2/2 Test #2: test_pipewire_camera_registry_race ...***Skipped   0.01 sec\n"
        )
        code, out, err = self.invoke(log)
        self.assertEqual(code, 1, out + err)
        self.assertEqual(
            out.splitlines()[1:3],
            [
                "  test_pipewire_camera_registry_race",
                "  test_vlc_player",
            ],
        )

    def test_empty_log_passes(self):
        code, out, err = self.invoke("")
        self.assertEqual(code, 0, out + err)

    def test_usage_error(self):
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = self.module.main(["check_skips.py"])
        self.assertEqual(code, 2)
        self.assertIn("usage: check_skips.py CTEST_LOG", err.getvalue())

    def test_shell_script_removed(self):
        self.assertFalse(SHELL_SCRIPT.exists(), f"{SHELL_SCRIPT} should have been deleted")


if __name__ == "__main__":
    unittest.main()
