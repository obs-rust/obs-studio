#!/usr/bin/env -S uv run --script
"""The Windows test lanes check out LF, like the other hosts.

GitHub's Windows runners set core.autocrlf=true, and `* text=auto` then gives
CRLF source files. soldr warns in each of these jobs that CRLF source bytes can
cause avoidable recompiles and cache misses. Only the test lanes change: what
the release builds put in a package stays as it is.

    uv run python -m unittest ci/test_windows_lf_checkout.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"
LANES = ("rust-tests-windows", "rust-tier2-windows")
SETTING = "git config --global core.autocrlf input"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8").split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([a-z0-9-]+):\s*$", text, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))[name]


class WindowsLfCheckoutTest(unittest.TestCase):
    def test_the_setting_comes_before_the_checkout(self):
        for name in LANES:
            with self.subTest(job=name):
                body = job(name)
                self.assertIn(SETTING, body)
                self.assertLess(body.index(SETTING), body.index("actions/checkout@"))

    def test_the_app_build_keeps_its_checkout(self):
        self.assertNotIn(SETTING, job("windows-build"))


if __name__ == "__main__":
    unittest.main()
