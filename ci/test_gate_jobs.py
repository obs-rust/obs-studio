#!/usr/bin/env -S uv run --script
"""CI Gate fails on a cancelled job nested in a reusable workflow (#1793).

    uv run python -m unittest ci/test_gate_jobs.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
from gate_jobs import blocking

PULL = (ROOT / ".github" / "workflows" / "ci.yml").read_text()


def job(name: str, conclusion: str | None, status: str = "completed") -> dict:
    return {"name": name, "status": status, "conclusion": conclusion}


# Run 39677084695 attempt 1 (2026-10-21): both macOS jobs were never acquired
# by a hosted runner and ended cancelled, yet CI Gate succeeded.
RUN_39677084695 = [
    job("Validate Data 🕵️", "success"),
    job("Format 🔍 / checks", "success"),
    job("Build 🧱 / Event Data 🔎", "success"),
    job("Build 🧱 / CI Mode 🛂", "success"),
    job("Build 🧱 / Fast Gate ⚡", "success"),
    job("Build 🧱 / Rust Tests 🦀 (windows-2025-vs2026)", "success"),
    job("Build 🧱 / Rust Tier 2 windows-2025-vs2026", "success"),
    job("Build 🧱 / Rust Tier 2 Linux 🐧 (build)", "success"),
    job("Build 🧱 / Rust Cross Lint 🦀 (Windows, macOS)", "success"),
    job("Build 🧱 / Rust Tests 🦀 (macos-26)", "cancelled"),
    job("Build 🧱 / Rust Tier 2 macos-26", "cancelled"),
    job("Build 🧱 / Build macOS", "skipped"),
    job("CI Gate ✅", None, "in_progress"),
]


class GateJobs(unittest.TestCase):
    def test_historical_cancelled_macos_jobs_block(self):
        self.assertEqual(blocking(RUN_39677084695), [
            "Build 🧱 / Rust Tests 🦀 (macos-26)",
            "Build 🧱 / Rust Tier 2 macos-26",
        ])

    def test_scope_skips_and_successes_pass(self):
        self.assertEqual(blocking([j for j in RUN_39677084695 if j["conclusion"] != "cancelled"]), [])

    def test_every_unsuccessful_conclusion_blocks(self):
        for conclusion in ("failure", "cancelled", "timed_out", "startup_failure", "action_required"):
            self.assertEqual(blocking([job("Build 🧱 / x", conclusion)]), ["Build 🧱 / x"], conclusion)

    def test_maintenance_jobs_never_block(self):
        # ci-pre's sweep and the end-of-queue janitor sit outside the gate.
        self.assertEqual(blocking([
            job("CI Pre 🧹 / Cache Janitor", "failure"),
            job("Cache Janitor 🧹", "cancelled"),
            job("CI Gate ✅", "failure"),
        ]), [])

    def test_unfinished_jobs_are_ignored(self):
        self.assertEqual(blocking([job("Cache Janitor 🧹", None, "queued"), job("Build 🧱 / y", None, "in_progress")]), [])

    def test_gate_runs_the_check_on_every_event(self):
        gate = re.search(r"^  ci-gate:\n((?:    .*\n|\s*\n)*)", PULL, re.M).group(1)
        step = re.search(r"- name: Check Run Jobs ✅\n((?:        .*\n)*)", gate)
        self.assertIsNotNone(step, "CI Gate must list the run's jobs (ci/gate_jobs.py)")
        self.assertIn("uv run --no-project ci/gate_jobs.py", step.group(1))
        self.assertNotIn("if:", step.group(1))


if __name__ == "__main__":
    unittest.main()
