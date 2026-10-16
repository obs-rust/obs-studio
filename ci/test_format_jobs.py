#!/usr/bin/env -S uv run --script
"""The clang-format and gersemi jobs run the local gate's pinned tools (#427).

    uv run python -m unittest ci/test_format_jobs.py

Each job spent about 75 s per run installing (`brew update`, `brew install
zsh`, then the formatter) to format for 0.3 s. They now run the same uvx-
pinned clang-format 22.1.3 and gersemi 0.25.0, over the same files, as the
local gate's lint lane, so CI and `./lint` cannot disagree.
"""

from __future__ import annotations

import re
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "check-format.yaml"
GATE = ROOT / "ci" / "local_gate.py"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name}"
    return m.group(1)


class FormatJobsTest(unittest.TestCase):
    def test_jobs_use_the_local_gate_not_brew(self):
        j = "\n".join(l for l in job("checks").splitlines() if not l.lstrip().startswith("#"))
        self.assertNotIn("brew", j)
        self.assertNotIn("./.github/actions/run-", j)
        self.assertRegex(j, r"astral-sh/setup-uv@[0-9a-f]{40}")
        for check in ("clang-format", "gersemi"):
            self.assertIn(f"uv run ci/local_gate.py --only {check}", j, check)


class GateOnlyTest(unittest.TestCase):
    def test_only_flag_is_offered(self):
        out = subprocess.run([sys.executable, str(GATE), "--help"], capture_output=True, text=True).stdout
        self.assertIn("--only", out)
        self.assertIn("clang-format", out)
        self.assertIn("gersemi", out)

    def test_unknown_only_value_is_rejected(self):
        r = subprocess.run([sys.executable, str(GATE), "--only", "nope"], capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)


if __name__ == "__main__":
    unittest.main()
