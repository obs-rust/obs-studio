#!/usr/bin/env -S uv run --script
"""Each merge-queue run reports its wall time and rebuilds (#930 E1).

    uv run python -m unittest ci/test_queue_report.py

Queue wall times came from `gh run list` by hand, so a regression or a
bypass that rebuilt every queued PR went unseen. CI Gate now writes a job
summary on merge_group runs: wall time, runner minutes, the slowest jobs,
and which attempt this is for the PR (attempt 2+ means the PR was rebuilt,
because master moved under it: a merge ahead of it failed, or a bypass).
"""

from __future__ import annotations

import importlib.util
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("queue_report", ROOT / "ci" / "queue_report.py")
queue_report = importlib.util.module_from_spec(spec)
sys.modules["queue_report"] = queue_report
spec.loader.exec_module(queue_report)
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"


def run(i: int, pr: int, created: str, sha: str = "a" * 40) -> dict:
    return {"id": i, "head_branch": f"gh-readonly-queue/master/pr-{pr}-{sha}", "created_at": created}


class AttemptTest(unittest.TestCase):
    def test_first_run_for_a_pr_is_attempt_one(self):
        cur = run(5, 7, "2026-10-16T10:00:00Z")
        self.assertEqual(queue_report.attempt([cur, run(4, 8, "2026-10-16T09:00:00Z")], cur), (1, []))

    def test_earlier_runs_for_the_same_pr_make_it_a_rebuild(self):
        cur = run(9, 7, "2026-10-16T10:00:00Z", "b" * 40)
        older = [run(3, 7, "2026-10-16T08:00:00Z"), run(6, 7, "2026-10-16T09:00:00Z", "c" * 40)]
        later = run(12, 7, "2026-10-16T11:00:00Z")
        self.assertEqual(queue_report.attempt(older + [later, cur], cur), (3, [3, 6]))


class SummaryTest(unittest.TestCase):
    def setUp(self):
        self.cur = run(9, 7, "2026-10-16T10:00:00Z")
        self.jobs = [
            {"name": "Build / Rust Tier 2 macos-26", "started_at": "2026-10-16T10:01:00Z", "completed_at": "2026-10-16T10:11:00Z", "conclusion": "success"},
            {"name": "Build / Fast Gate", "started_at": "2026-10-16T10:01:00Z", "completed_at": "2026-10-16T10:03:00Z", "conclusion": "success"},
            {"name": "CI Gate", "started_at": "2026-10-16T10:12:00Z", "completed_at": None, "conclusion": None},
        ]

    def test_reports_wall_time_runner_minutes_and_slowest_jobs(self):
        md = queue_report.summary(self.cur, self.jobs, (1, []), now="2026-10-16T10:12:30Z")
        self.assertIn("PR #7", md)
        self.assertRegex(md, r"[Ww]all time[^\n]*12\.5 min")
        self.assertRegex(md, r"[Rr]unner time[^\n]*12\.0 min")
        self.assertLess(md.index("Rust Tier 2 macos-26"), md.index("Fast Gate"))
        self.assertIn("attempt 1", md)

    def test_names_a_rebuild(self):
        md = queue_report.summary(self.cur, self.jobs, (3, [3, 6]), now="2026-10-16T10:12:30Z")
        self.assertIn("attempt 3", md)
        self.assertIn("rebuilt", md)
        self.assertIn("3, 6", md)

    def test_reports_dispatch_wait_without_calling_it_dependency_wait(self):
        self.jobs[-1]["created_at"] = "2026-10-16T10:11:01Z"
        md = queue_report.summary(self.cur, self.jobs, (1, []), now="2026-10-16T10:12:30Z")
        self.assertIn("Job creation-to-start elapsed time", md)
        self.assertIn("| CI Gate | 59.0 |", md)
        self.assertIn("job creation to runner start", md)
        self.assertIn("may include dependency waiting", md)
        self.assertIn("cannot isolate runner dispatch latency", md)

    def test_setup_steps_report_elapsed_time_without_counting_post_or_incomplete_steps(self):
        self.jobs[1]["steps"] = [
            {"name": "Set Up soldr", "started_at": "2026-10-16T10:01:00Z", "completed_at": "2026-10-16T10:01:30Z", "conclusion": "success"},
            {"name": "Restore ccache", "started_at": "2026-10-16T10:01:30Z", "completed_at": "2026-10-16T10:01:36Z", "conclusion": "success"},
            {"name": "Post Set Up soldr", "started_at": "2026-10-16T10:02:58Z", "completed_at": "2026-10-16T10:03:00Z", "conclusion": "success"},
            {"name": "Unfinished cache", "started_at": "2026-10-16T10:02:58Z", "completed_at": None, "conclusion": None},
        ]
        md = queue_report.summary(self.cur, self.jobs, (1, []), now="2026-10-16T10:12:30Z")
        self.assertIn("| Build / Fast Gate | Set Up soldr | 30.0 |", md)
        self.assertIn("| Build / Fast Gate | Restore ccache | 6.0 |", md)
        self.assertNotIn("Post Set Up soldr", md)
        self.assertNotIn("Unfinished cache", md)
        self.assertIn("cache restores do not prove compiler reuse", md)


class WorkflowTest(unittest.TestCase):
    def test_ci_gate_reports_on_queue_runs_without_failing(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        gate = text[text.index("  ci-gate:"):]
        m = re.search(r"- name: Report Queue Run[^\n]*\n(?:        [^\n]*\n|\n)+", gate)
        self.assertIsNotNone(m, "no Report Queue Run step in ci-gate")
        step = m.group(0)
        self.assertIn("github.event_name == 'merge_group'", step)
        self.assertIn("continue-on-error: true", step)
        self.assertIn("uv run --no-project ci/queue_report.py", step)
        self.assertIn("actions: read", gate)


if __name__ == "__main__":
    unittest.main()
