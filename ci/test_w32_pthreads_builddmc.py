#!/usr/bin/env -S uv run --script
"""w32-pthreads' Digital Mars build helper is Python (obs-rust/obs-studio#962).

The helper lives in `deps/w32-pthreads/builddmc.py` since the port off the
batch file: the same two `dmc` command lines, release then debug, token for
token. It has no caller -- it is the vendored w32-pthreads build helper for the
Digital Mars Compiler (its ChangeLog is the only reference to it) -- so this
suite drives the module directly and pins that its wiring is complete: the
`.bat` gone, its GEN-005 exception gone.

    uv run python -m unittest ci/test_w32_pthreads_builddmc.py
"""

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parent.parent
W32 = ROOT / "deps" / "w32-pthreads"
MODULE = W32 / "builddmc.py"
SHELL = W32 / "builddmc.bat"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "deps/w32-pthreads/builddmc.bat"'

# The batch file's two `dmc` command lines, token for token.
COMMON = [
    "-D_WIN32_WINNT",
    "-D_MT",
    "-DHAVE_PTW32_CONFIG_H",
    "-I.;c:\\dm\\include",
    "-o+all",
    "-WD",
    "pthread.c",
    "user32.lib+kernel32.lib+wsock32.lib",
    "-L/impl",
    "-L/NODEBUG",
    "-L/SU:WINDOWS",
]
RELEASE = ["c:\\dm\\bin\\dmc", *COMMON]
DEBUG = ["c:\\dm\\bin\\dmc", "-g", *COMMON]


def load_module():
    """Import builddmc.py by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("w32_pthreads_builddmc", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ArgvTest(unittest.TestCase):
    def test_release_and_debug_commands_are_unchanged(self):
        module = load_module()
        self.assertEqual(module.argv(debug=False), RELEASE)
        self.assertEqual(module.argv(debug=True), DEBUG)


class MainTest(unittest.TestCase):
    def test_main_runs_release_then_debug(self):
        module = load_module()
        with patch.object(module.subprocess, "run", return_value=Mock(returncode=0)) as run:
            status = module.main()
        self.assertEqual(status, 0)
        run.assert_called()
        self.assertEqual(
            [call.args[0] for call in run.call_args_list],
            [RELEASE, DEBUG],
        )

    def test_main_reports_the_last_command_status(self):
        # Batch leaves %errorlevel% at the last command's status.
        module = load_module()
        with patch.object(module.subprocess, "run", return_value=Mock(returncode=7)):
            self.assertEqual(module.main(), 7)


class WiringTest(unittest.TestCase):
    def test_wiring_is_complete(self):
        self.assertFalse(SHELL.exists(), "builddmc.bat should be gone")
        self.assertTrue(MODULE.is_file(), "builddmc.py should exist")
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(encoding="utf-8"),
            "the GEN-005 exception for builddmc.bat should be removed",
        )


if __name__ == "__main__":
    unittest.main()
