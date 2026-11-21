#!/usr/bin/env -S uv run --script
"""Job timeouts bound a lost runner instead of letting it hold the queue (#2593).

    uv run python -m unittest ci/test_ci_job_timeouts.py

When a CI runner dies, the job stays `in_progress` until its
`timeout-minutes` (or the merge queue's response timeout) ends it. A
60/90-minute timeout on an 8-14-minute job turned one lost runner into a
44-60-minute stall on #2514. The maintainer's `Decided:` on #2593 sets the
bounds at about 2x the measured duration: Fast Gate (`rust-tests-linux`) 20;
`rust-tests-windows` 30 / `rust-tests-macos` 25; `rust-tier2-windows` 25 /
`rust-tier2-macos` 25. The macOS Tier 2 lane grew to 17-24 min and timed out
on master at 25, so the same 2x rule now sets it at 45 (#5415).
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

    def test_rust_tier2_windows_times_out_at_25(self):
        """`rust-tier2-windows` runs about 10-15 min; 25 bounds a dead runner (#2593 A)."""
        self.assertEqual(timeout_line(job_block(self.text, "rust-tier2-windows")), "25")

    def test_rust_tier2_macos_times_out_at_45(self):
        """`rust-tier2-macos` runs 17-24 min and timed out at 25 on master; 2x is 45 (#5415)."""
        self.assertEqual(timeout_line(job_block(self.text, "rust-tier2-macos")), "45")


class Tier2AptTimeoutTest(unittest.TestCase):
    def test_all_tier2_apt_calls_have_command_and_step_bounds(self):
        """A mirror stall must not consume the build's 45-minute budget (#5213)."""
        calls = 0
        for workflow in (WORKFLOW, WORKFLOW.with_name("ci.yml")):
            text = workflow.read_text(encoding="utf-8")
            for job in re.finditer(r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)", text, re.M | re.S):
                name, body = job.groups()
                if "tier2" not in name:
                    continue
                for step in re.split(r"^      - ", body, flags=re.M)[1:]:
                    apt_calls = list(re.finditer(r"^([^#\n]*?)\bapt-get\b", step, re.M))
                    if not apt_calls:
                        continue
                    with self.subTest(workflow=workflow.name, job=name, step=step.splitlines()[0]):
                        bound = re.search(r"^        timeout-minutes: (\d+)$", step, re.M)
                        self.assertIsNotNone(bound, "apt step has no timeout-minutes")
                        self.assertTrue(0 < int(bound[1]) <= 10, "apt step must finish within 10 minutes")
                    for call in apt_calls:
                        calls += 1
                        with self.subTest(workflow=workflow.name, job=name, command=call[0].strip()):
                            bound = re.search(r"\btimeout (?:--kill-after=\d+s? )?([1-9]\d*)\b", call[1])
                            self.assertIsNotNone(bound, "apt-get has no command timeout")
                            self.assertLessEqual(int(bound[1]), 300, "apt command exceeds five minutes")
        self.assertGreater(calls, 0, "Tier 2 apt calls disappeared from the workflow scan")

    def test_acquire_bounds_are_set_before_the_prebaked_fast_path(self):
        bootstrap = job_block(WORKFLOW.read_text(encoding="utf-8"), "rust-linux-tier2-build")
        bootstrap = bootstrap.split("name: Install Git", 1)[1].split("\n      - ", 1)[0]
        config = bootstrap.split('if [ "$TIER2_PREBAKED" = true ]; then', 1)[0]
        for option in ('Acquire::http::Timeout "20";', 'Acquire::https::Timeout "20";',
                       'Acquire::Retries "3";'):
            self.assertIn(option, config)
        self.assertIn("> /etc/apt/apt.conf.d/99-ci-acquire-bounds", config)


if __name__ == "__main__":
    unittest.main()
