#!/usr/bin/env -S uv run --script
"""Superseded merge-queue runs are cancelled when master moves (#986).

    uv run python -m unittest ci/test_cancel_superseded.py

A queue ref carries the base commit (gh-readonly-queue/master/pr-<N>-<sha>),
so the workflow's concurrency group never matched a PR's rebuilt queue run
to its old one: after master moved, 2-3 runs per PR waited on the scarce
ubuntu-26.04 runners and were cancelled by hand (10, then 11 runs). The
first job of every merge_group run now cancels the older in-flight queue
runs for the same PR.
"""

from __future__ import annotations

import importlib.util
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci" / "cancel_superseded.py"
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"


def load():
    spec = importlib.util.spec_from_file_location("cancel_superseded", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod
    spec.loader.exec_module(mod)
    return mod


def run(rid, ref, status="queued", created="2026-10-16T04:00:00Z"):
    return {"id": rid, "head_branch": ref, "status": status, "created_at": created}


Q = "gh-readonly-queue/master/pr-"


class PrNumberTest(unittest.TestCase):
    def test_reads_the_pr_from_a_queue_ref(self):
        cs = load()
        self.assertEqual(cs.queue_pr(f"{Q}970-a03f8d36dee7599fc690ebdc40f2563071d7e2ce"), 970)
        self.assertEqual(cs.queue_pr(f"refs/heads/{Q}12-abc"), 12)
        self.assertIsNone(cs.queue_pr("ci/966-tier2-native"))


class SupersededTest(unittest.TestCase):
    def setUp(self):
        self.cs = load()
        self.current = run(10, f"{Q}970-a1b2c3", "in_progress", "2026-10-16T04:05:00Z")

    def test_cancels_older_runs_for_the_same_pr(self):
        runs = [
            self.current,
            run(9, f"{Q}970-b2c3d4", "queued", "2026-10-16T04:01:00Z"),
            run(8, f"{Q}970-c3d4e5", "in_progress", "2026-10-16T03:40:00Z"),
        ]
        self.assertEqual(sorted(self.cs.superseded(runs, self.current)), [8, 9])

    def test_leaves_other_prs_alone(self):
        runs = [self.current, run(7, f"{Q}936-d4e5f6", "queued", "2026-10-16T04:01:00Z")]
        self.assertEqual(self.cs.superseded(runs, self.current), [])

    def test_never_cancels_a_newer_run(self):
        runs = [self.current, run(11, f"{Q}970-e5f6a7", "queued", "2026-10-16T04:09:00Z")]
        self.assertEqual(self.cs.superseded(runs, self.current), [])

    def test_ignores_finished_runs_and_pr_branches(self):
        runs = [
            self.current,
            run(6, f"{Q}970-f6a7b8", "completed", "2026-10-16T04:00:00Z"),
            run(5, "ci/970-branch", "queued", "2026-10-16T04:00:00Z"),
        ]
        self.assertEqual(self.cs.superseded(runs, self.current), [])


class WorkflowTest(unittest.TestCase):
    def setUp(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        m = re.search(r"^  cancel-superseded:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
        self.assertIsNotNone(m, "no cancel-superseded job")
        self.job = m.group(1)

    def test_runs_only_for_the_merge_queue(self):
        self.assertIn("if: ${{ !cancelled() && github.event_name == 'merge_group' }}", self.job)

    def test_only_this_job_can_cancel_runs(self):
        self.assertIn("actions: write", self.job)
        top = WORKFLOW.read_text(encoding="utf-8").split("\njobs:")[0]
        self.assertNotIn("actions: write", top)

    def test_calls_the_script(self):
        self.assertIn("uv run --no-project ci/cancel_superseded.py", self.job)


if __name__ == "__main__":
    unittest.main()
