#!/usr/bin/env -S uv run --script
"""Job timeouts bound a lost runner instead of letting it hold the queue (#2593).

    uv run python -m unittest ci/test_ci_job_timeouts.py

When a CI runner dies, the job stays `in_progress` until its
`timeout-minutes` (or the merge queue's response timeout) ends it. A
60/90-minute timeout on an 8-14-minute job turned one lost runner into a
44-60-minute stall on #2514. The maintainer's `Decided:` on #2593 sets the
bounds at about 2x the measured duration: Fast Gate (`rust-tests-linux`) 20;
`rust-tests-windows` 30 / `rust-tests-macos` 25; `rust-tier2-windows` 25 /
`rust-tier2-macos` 25.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
WINDOWS = "windows-2025-vs2026"
MACOS = "macos-26"


def job_block(text: str, name: str) -> str:
    m = re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)
    assert m, f"no {name} job"
    return m.group(1)


def timeout_line(block: str) -> str:
    m = re.search(r"^    timeout-minutes: (.*)$", block, re.M)
    assert m, "job has no timeout-minutes"
    return m.group(1).strip()


class JobTimeoutTest(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")

    def test_fast_gate_times_out_at_20(self):
        """Fast Gate (`rust-tests-linux`) runs 7.7-11.3 min; 2x is 20 (#2593 A)."""
        self.assertEqual(timeout_line(job_block(self.text, "rust-tests-linux")), "20")

    def test_rust_tests_time_out_per_os(self):
        """`rust-tests` runs 9-14.3 min; macos-26 25, windows 30 (#2593 A)."""
        self.assertEqual(timeout_line(job_block(self.text, "rust-tests-macos")), "25")
        self.assertEqual(timeout_line(job_block(self.text, "rust-tests-windows")), "30")

    def test_rust_tier2_times_out_at_25(self):
        """`rust-tier2` runs about 10-15 min; 25 bounds a dead runner (#2593 A)."""
        self.assertEqual(timeout_line(job_block(self.text, "rust-tier2-macos")), "25")
        self.assertEqual(timeout_line(job_block(self.text, "rust-tier2-windows")), "25")


if __name__ == "__main__":
    unittest.main()
