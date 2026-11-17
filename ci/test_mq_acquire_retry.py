#!/usr/bin/env -S uv run --script
"""Recorded runner-acquisition incident, with negative API-response variants.

    uv run --no-project python -m unittest ci/test_mq_acquire_retry.py
"""

from __future__ import annotations

import copy
import importlib.util
import io
import json
import subprocess
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

REPO = "obs-rust/obs-studio"
ROOT = Path(__file__).resolve().parent.parent


def load_ci(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "ci" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


retry = load_ci("mq_acquire_retry")


class RecordedApi:
    """Serve the recorded REST responses; mutations only record their argv."""

    def __init__(self):
        self.fixture = json.loads((ROOT / "ci/fixtures/mq-acquire-retry.json").read_text())
        self.run = self.fixture["run"]
        self.heads = [self.run["head_sha"]]
        self.mutations = []
        self.reads = []
        self.queue_reads = 0
        self.leave_before_post = False
        self.advance_before_post = False
        self.fail_annotations = False
        self.queue_total_offset = 0

    def command(self, argv, **kwargs):
        self.reads.append(argv)
        if argv[0] == "uv":
            self.queue_reads += 1
            heads = [] if self.leave_before_post and self.queue_reads > 1 else self.heads
            value = {"data": {"repository": {"mergeQueue": {"entries": {
                "totalCount": len(heads) + self.queue_total_offset,
                "nodes": [{"headCommit": {"oid": head}} for head in heads],
                "pageInfo": {"hasNextPage": False, "endCursor": None},
            }}}}}
            self.assert_graphql(argv)
        elif "POST" in argv:
            self.mutations.append(argv)
            value = None
        else:
            endpoint = next(arg for arg in argv if arg.startswith(f"repos/{REPO}/"))
            if endpoint.endswith("rerun-failed-jobs"):
                raise AssertionError("mutation must explicitly use POST")
            if "/check-runs/" in endpoint:
                if self.fail_annotations:
                    raise subprocess.CalledProcessError(1, argv, stderr="HTTP 403")
                job_id = endpoint.split("/check-runs/")[1].split("/")[0]
                value = [self.fixture["annotations"][job_id]]
            elif "/jobs?" in endpoint:
                value = [self.fixture["jobs"]]
            elif "/runs?" in endpoint:
                value = [{"total_count": 1, "workflow_runs": [self.run]}]
            else:
                value = dict(self.run)
                if self.advance_before_post:
                    value["run_attempt"] += 1
        return subprocess.CompletedProcess(argv, 0, json.dumps(value), "")

    @staticmethod
    def assert_graphql(argv):
        assert any(arg.endswith("gh_graphql.py") for arg in argv), argv
        assert "--no-project" in argv and "--script" in argv, argv


class RetryTest(unittest.TestCase):
    def setUp(self):
        self.api = RecordedApi()

    def run_bot(self, dry_run=False):
        output = io.StringIO()
        with patch.object(subprocess, "run", side_effect=self.api.command), redirect_stdout(output):
            result = retry.maintain(REPO, dry_run=dry_run)
        self.assertIn("queue", output.getvalue().lower())
        return result, output.getvalue()

    def test_acquisition_only_reruns_failed_jobs(self):
        result, output = self.run_bot()
        self.assertEqual(result, 0)
        self.assertEqual(len(self.api.mutations), 1)
        self.assertIn(f"repos/{REPO}/actions/runs/44150038419/rerun-failed-jobs", self.api.mutations[0])
        self.assertIn("rerun", output)

    def test_real_test_failure_prevents_rerun(self):
        job = next(j for j in self.api.fixture["jobs"]["jobs"] if j["conclusion"] == "success")
        job["conclusion"] = "failure"
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_attempt_three_is_never_retried(self):
        self.api.run["run_attempt"] = 3
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_attempt_two_can_retry_to_attempt_three(self):
        self.api.run["run_attempt"] = 2
        self.run_bot()
        self.assertEqual(len(self.api.mutations), 1)

    def test_incomplete_queue_inventory_never_retries(self):
        self.api.queue_total_offset = 1
        self.assertEqual(self.run_bot()[0], 1)
        self.assertEqual(self.api.mutations, [])

    def test_incomplete_jobs_inventory_never_retries(self):
        self.api.fixture["jobs"]["total_count"] += 1
        self.assertEqual(self.run_bot()[0], 1)
        self.assertEqual(self.api.mutations, [])

    def test_gate_failure_without_capacity_diagnostics_never_retries(self):
        gate = next(j for j in self.api.fixture["jobs"]["jobs"] if j["name"] == "CI Gate ✅")
        self.api.fixture["annotations"][str(gate["id"])] = []
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_cancellation_after_start_is_not_runner_acquisition(self):
        cancelled = next(j for j in self.api.fixture["jobs"]["jobs"] if j["conclusion"] == "cancelled")
        cancelled["runner_name"] = "GitHub Actions 123"
        cancelled["runner_id"] = 123
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_lone_gate_failure_does_not_retry(self):
        for job in self.api.fixture["jobs"]["jobs"]:
            if job["conclusion"] == "cancelled":
                job["conclusion"] = "success"
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_departed_entry_is_ignored_without_listing_runs(self):
        self.api.heads = []
        self.run_bot()
        self.assertEqual(self.api.mutations, [])
        self.assertFalse(any(argv[0] == "gh" for argv in self.api.reads))

    def test_departure_during_inspection_prevents_rerun(self):
        self.api.leave_before_post = True
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_concurrent_manual_retry_prevents_duplicate(self):
        self.api.advance_before_post = True
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_dry_run_has_no_mutations(self):
        _, output = self.run_bot(dry_run=True)
        self.assertIn("dry-run", output)
        self.assertEqual(self.api.mutations, [])

    def test_running_work_is_deferred(self):
        self.api.run["status"] = "in_progress"
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_non_merge_group_run_is_ignored(self):
        self.api.run["event"] = "pull_request"
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_wrong_sha_is_ignored(self):
        self.api.heads = ["a" * 40]
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_uncertain_annotations_never_retry(self):
        self.api.fail_annotations = True
        self.assertEqual(self.run_bot()[0], 1)
        self.assertEqual(self.api.mutations, [])

    def test_gate_setup_or_scope_failure_is_not_an_aggregate(self):
        for scenario in ("scope", "setup"):
            with self.subTest(scenario=scenario):
                self.api = RecordedApi()
                gate = next(j for j in self.api.fixture["jobs"]["jobs"] if j["name"] == "CI Gate ✅")
                if scenario == "scope":
                    self.api.fixture["annotations"][str(gate["id"])].append({
                        "annotation_level": "failure", "message": "CI Gate could not resolve merge-group scope",
                    })
                else:
                    gate["steps"][0]["conclusion"] = "failure"
                self.run_bot()
                self.assertEqual(self.api.mutations, [])

    def test_missing_or_inexact_acquisition_annotation_is_not_retried(self):
        cancelled = next(j for j in self.api.fixture["jobs"]["jobs"] if j["conclusion"] == "cancelled")
        annotations = self.api.fixture["annotations"][str(cancelled["id"])]
        original = copy.deepcopy(annotations)
        for evidence in ([], [dict(original[0], message=original[0]["message"] + " (other)")],
                         [*original, {"annotation_level": "failure", "message": "assertion failed"}]):
            with self.subTest(evidence=evidence):
                self.api.fixture["annotations"][str(cancelled["id"])] = evidence
                self.run_bot()
                self.assertEqual(self.api.mutations, [])


class WorkflowTest(unittest.TestCase):
    def test_retry_uses_github_token_before_ejection_handler(self):
        guard = load_ci("test_ai_rebase_token_guard")
        workflow = (ROOT / ".github/workflows/ai-rebase.yaml").read_text()
        shared = guard.steps(guard.jobs(workflow)["rebase"])
        step = next(s for s in shared if "ci/mq_acquire_retry.py" in s)
        self.assertEqual(guard.step_if(step), "")
        self.assertIn("GH_TOKEN: ${{ github.token }}", step)
        self.assertIn("inputs.dry_run", step)
        self.assertIn("actions: write", workflow.split("\njobs:")[0])
        self.assertIn("checks: read", workflow.split("\njobs:")[0])
        self.assertLess(shared.index(step), next(i for i, s in enumerate(shared) if "--requeue-ejected" in s))


if __name__ == "__main__":
    unittest.main()
