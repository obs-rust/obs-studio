#!/usr/bin/env -S uv run --script
"""Runner-loss retry: the classifier, the retry decision and the workflow wiring.

    uv run --no-project python -m unittest ci/test_runner_loss_retry.py

Evidence: obs-rust/obs-studio run 46103362249, where a self-hosted runner host
shut down and killed jobs with "The runner has received a shutdown signal".
Every mutation is mocked.
"""
from __future__ import annotations

import importlib.util
import io
import re
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SHUTDOWN = "The runner has received a shutdown signal. This can happen when the runner service is stopped."


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "ci" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


rl = load("runner_loss_retry")


def run(attempt=1, event="pull_request", conclusion="failure", **kw):
    return {"id": 9, "run_attempt": attempt, "event": event, "status": "completed",
            "conclusion": conclusion, "head_sha": "a" * 40, "workflow_id": 7,
            "head_branch": "b", "pull_requests": [{"number": 12}], **kw}


def job(jid, name, conclusion="failure", steps=()):
    return {"id": jid, "name": name, "status": "completed", "conclusion": conclusion,
            "steps": [{"name": n, "conclusion": c} for n, c in steps]}


GATE = job(99, "CI Gate ✅")
LOST = job(1, "Rust Tests 🦀 (linux)")
TEST = job(2, "Tier 2 (linux)", steps=[("Run tests", "failure")])


class ClassifierTests(unittest.TestCase):
    def test_runner_shutdown_and_loss_are_infra(self):
        for text in (SHUTDOWN, "The runner has lost communication with the server",
                     "runner self-hosted-3 disconnected", "Could not resolve host: github.com",
                     "curl: (7) Failed to connect to proxy", "HTTP 503 from registry"):
            with self.subTest(text=text):
                self.assertEqual(rl.job_kind(LOST, text), "infra")

    def test_real_failures_win_over_network_text(self):
        for text in ("test result: FAILED. 1 failed", "error[E0308]: mismatched types",
                     "src/a.c:3:1: error: expected ';'", f"{SHUTDOWN}\nundefined reference to `x'"):
            with self.subTest(text=text):
                self.assertEqual(rl.job_kind(LOST, text), "code")

    def test_empty_or_foreign_evidence_is_unknown(self):
        for text in ("", "Process completed with exit code 1."):
            self.assertEqual(rl.job_kind(LOST, text), "unknown")

    def test_failed_setup_step_is_infra(self):
        setup = job(3, "x", steps=[("Run actions/checkout@abc", "failure")])
        self.assertEqual(rl.job_kind(setup, ""), "infra")
        mixed = job(3, "x", steps=[("Run actions/checkout@abc", "success"), ("Build", "failure")])
        self.assertEqual(rl.job_kind(mixed, ""), "unknown")


class DecisionTests(unittest.TestCase):
    def decide(self, jobs, texts=None, **kw):
        return rl.decide(run(**kw), jobs, texts if texts is not None else {j["id"]: SHUTDOWN for j in jobs})

    def test_lost_runner_reruns_on_first_two_attempts_only(self):
        self.assertTrue(self.decide([LOST, GATE], attempt=1)[0])
        self.assertTrue(self.decide([LOST, GATE], attempt=2)[0])
        ok, why = self.decide([LOST, GATE], attempt=3)
        self.assertFalse(ok)
        self.assertIn("spent", why)

    def test_unknown_attempt_refuses(self):
        for attempt in (None, 0, "1"):
            self.assertFalse(self.decide([LOST, GATE], attempt=attempt)[0])

    def test_one_real_failure_refuses_the_whole_run(self):
        texts = {1: SHUTDOWN, 2: "test result: FAILED"}
        ok, why = self.decide([LOST, TEST, GATE], texts)
        self.assertFalse(ok)
        self.assertIn("code failure: Tier 2", why)

    def test_one_unexplained_failure_refuses(self):
        ok, why = self.decide([LOST, TEST, GATE], {1: SHUTDOWN, 2: ""})
        self.assertFalse(ok)
        self.assertIn("unknown failure", why)

    def test_gate_alone_or_nothing_failed_refuses(self):
        self.assertFalse(self.decide([GATE])[0])
        self.assertFalse(self.decide([job(1, "ok", "success"), GATE])[0])

    def test_cancelled_siblings_are_collateral_of_an_infra_job(self):
        cancelled = job(5, "macOS", "cancelled")
        self.assertTrue(self.decide([LOST, cancelled, GATE])[0])
        self.assertFalse(self.decide([cancelled, GATE])[0])

    def test_only_completed_failed_pr_or_group_runs(self):
        self.assertFalse(self.decide([LOST, GATE], conclusion="cancelled")[0])
        self.assertFalse(self.decide([LOST, GATE], conclusion="success")[0])
        self.assertFalse(self.decide([LOST, GATE], status="in_progress")[0])
        self.assertFalse(self.decide([LOST, GATE], event="push")[0])
        self.assertTrue(self.decide([LOST, GATE], event="merge_group")[0])
        self.assertFalse(rl.decide(run(), [], {})[0])
        self.assertFalse(rl.decide(run(), [dict(LOST, status="in_progress")], {})[0])

    def test_timed_out_job_is_not_infra_without_evidence(self):
        self.assertFalse(self.decide([job(1, "slow", "timed_out"), GATE], {1: ""})[0])


class FlowTests(unittest.TestCase):
    """maintain() with every API call mocked; assert exactly which mutations happen."""

    def go(self, jobs, texts, run_=None, event="pull_request", pr=None, **kw):
        run_ = run_ or run(event=event)
        mutations = []
        pr_api = pr or {"state": "open", "head": {"sha": "a" * 40}}

        def command(argv, **_):
            url = argv[-1]
            if url.endswith("/workflows/ci.yml"):
                return {"id": 7, "path": ".github/workflows/ci.yml"}
            if url.endswith("/actions/runs/9"):
                return run_
            if "/pulls/" in url:
                return pr_api
            raise AssertionError(argv)

        out = io.StringIO()
        with patch.object(rl.retry, "command", command), \
                patch.object(rl.retry, "items", return_value=jobs), \
                patch.object(rl, "job_text", side_effect=lambda r, i, j: texts[j["id"]]), \
                patch.object(rl.retry, "rerun_failed", side_effect=lambda *a, **k: mutations.append("rerun")), \
                patch.object(rl.retry, "recover_group", side_effect=lambda *a, **k: mutations.append("group")), \
                patch.object(rl.retry, "run_unchanged", return_value=True), \
                redirect_stdout(out):
            rc = rl.maintain("o/r", 9, sleep=lambda s: None, **kw)
        return rc, mutations, out.getvalue()

    def test_pr_lost_runner_is_rerun(self):
        rc, mutations, out = self.go([LOST, GATE], {1: SHUTDOWN})
        self.assertEqual((rc, mutations), (0, ["rerun"]))

    def test_pr_real_failure_is_never_retried(self):
        rc, mutations, out = self.go([LOST, TEST, GATE], {1: SHUTDOWN, 2: "assertion `left == right` failed"})
        self.assertEqual((rc, mutations), (0, []))
        self.assertIn("code failure", out)

    def test_third_attempt_is_not_retried(self):
        rc, mutations, _ = self.go([LOST, GATE], {1: SHUTDOWN}, run_=run(attempt=3))
        self.assertEqual(mutations, [])

    def test_pr_with_a_newer_head_is_not_retried(self):
        _, mutations, out = self.go([LOST, GATE], {1: SHUTDOWN}, pr={"state": "open", "head": {"sha": "c" * 40}})
        self.assertEqual(mutations, [])
        self.assertIn("head moved on", out)

    def test_merge_group_recovers_immediately(self):
        group = run(event="merge_group", head_branch="gh-readonly-queue/master/pr-12-" + "a" * 40)
        with patch.object(rl.retry, "pr_snapshot", return_value={"state": "OPEN"}), \
                patch.object(rl.retry, "live_group", return_value=False), \
                patch.object(rl.retry, "ejection", return_value={"id": "r"}):
            rc, mutations, _ = self.go([LOST, GATE], {1: SHUTDOWN}, run_=group)
        self.assertEqual((rc, mutations), (0, ["group"]))

    def test_merge_group_waits_for_the_ejection_event_then_gives_up(self):
        group = run(event="merge_group", head_branch="gh-readonly-queue/master/pr-12-" + "a" * 40)
        with patch.object(rl.retry, "pr_snapshot", return_value={}) as snap, \
                patch.object(rl.retry, "live_group", return_value=False), \
                patch.object(rl.retry, "ejection", return_value=None), \
                patch.object(rl.retry, "held", return_value=False):
            _, mutations, out = self.go([LOST, GATE], {1: SHUTDOWN}, run_=group)
        self.assertEqual(mutations, [])
        self.assertEqual(snap.call_count, rl.EJECTION_POLLS)
        self.assertIn("no live group or latest ejection", out)

    def test_api_error_authorizes_nothing(self):
        with patch.object(rl.retry, "command", side_effect=ValueError("boom")), redirect_stdout(io.StringIO()):
            self.assertEqual(rl.maintain("o/r", 9), 1)


class WorkflowTests(unittest.TestCase):
    def setUp(self):
        self.text = (ROOT / ".github/workflows/ai-rebase.yaml").read_text()
        self.guard = load("test_ai_rebase_token_guard")
        self.jobs = self.guard.jobs(self.text)

    def test_completed_pull_run_triggers_only_the_retry_job(self):
        self.assertRegex(self.text, r"workflow_run:\n    workflows: \[Pull\]\n    types: \[completed\]")
        self.assertIn("github.event_name != 'workflow_run'", self.guard.job_if(self.jobs["capacity-recovery"]))
        self.assertIn("github.event_name != 'workflow_run'", self.guard.job_if(self.jobs["rebase"]))
        retry = self.jobs["runner-loss-retry"]
        condition = self.guard.job_if(retry)
        for needle in ("github.event_name == 'workflow_run'", "workflow_run.conclusion == 'failure'",
                       "workflow_run.run_attempt < 3"):
            self.assertIn(needle, condition)
        self.assertIn("ci/runner_loss_retry.py --run-id", retry)
        self.assertNotIn("ai_rebase.py", retry)

    def test_retry_job_runs_default_branch_code_only(self):
        retry = self.jobs["runner-loss-retry"]
        self.assertIn("ref: master", retry)
        self.assertIn("persist-credentials: false", retry)
        self.assertNotIn("workflow_run.head_sha", retry)
        self.assertIn("steps.app.outputs.token || github.token", retry)
        self.assertRegex(retry, r"concurrency:\n      group: runner-loss-retry-")

    def test_every_ci_artifact_upload_is_idempotent_on_rerun(self):
        """A job whose runner vanished may have finalized an artifact; the rerun must replace it."""
        for name in ("ci.yml", "build-project.yaml"):
            text = (ROOT / ".github/workflows" / name).read_text()
            for match in re.finditer(r"uses: actions/upload-artifact@.*\n((?:(?:          |        ).*\n)*)", text):
                with self.subTest(workflow=name, step=match.group(0).splitlines()[0]):
                    self.assertRegex(match.group(1), r"(?m)^\s+overwrite: true$")


if __name__ == "__main__":
    unittest.main()
