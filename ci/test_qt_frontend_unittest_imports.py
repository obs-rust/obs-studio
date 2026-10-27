#!/usr/bin/env -S uv run --script
"""linux_qt_frontend.py's real-Qt unittest command imports its modules
(#2311 removed the sys.path edits that used to put ci/ on the path). Run the
driver's own command with no Qt prefix: every test must load, then skip.

    uv run --no-project ci/test_qt_frontend_unittest_imports.py
"""

from __future__ import annotations

import os
import subprocess
import unittest
from pathlib import Path

from linux_qt_frontend import REAL_QT_TESTS, UNITTEST

ROOT = Path(__file__).resolve().parent.parent


class QtFrontendUnittestImportsTest(unittest.TestCase):
    def test_every_module_imports(self) -> None:
        env = {k: v for k, v in os.environ.items() if k not in ("PYTHONPATH", "OBS_QT_TEST_PREFIX")}
        env.update(OBS_QT_TEST_PREFIX="", PYTHONPATH="ci")
        out = subprocess.run(
            [*UNITTEST.split(), *REAL_QT_TESTS], cwd=ROOT, env=env, capture_output=True, text=True
        )
        self.assertNotIn("ModuleNotFoundError", out.stderr)
        self.assertEqual(out.returncode, 0, out.stderr[-2000:])


if __name__ == "__main__":
    unittest.main()
