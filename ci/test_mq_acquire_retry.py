#!/usr/bin/env -S uv run --script
"""Recorded capacity incidents; every mutation is mocked.

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
    def __init__(self, event="merge_group"):
        self.fixture = json.loads((ROOT / "ci/fixtures/mq-acquire-retry.json").read_text())[event]
        self.run = self.fixture["run"]
        self.mutations = []
        self.reads = []
        self.comments = []
        self.run_reads = 0
        self.changed_run = False
        self.advance_on_live = False
        self.changed_pr = False
        self.pr_reads = 0
        self.fail_annotations = False
        self.pr = {
            "number": 5109, "headRefOid": "b" * 40, "state": "OPEN", "isDraft": False,
            "mergeable": "MERGEABLE", "mergeStateStatus": "CLEAN", "reviewDecision": "APPROVED",
            "autoMergeRequest": None, "labels": [], "body": "", "id": "PR_test",
            "mergeQueueEntry": None,
        }
        # Synthetic queue transitions bound to the recorded synthetic run SHA.
        self.events = [
            {"__typename": "PullRequestCommit", "commit": {"oid": self.pr["headRefOid"]}},
            {"__typename": "AddedToMergeQueueEvent", "createdAt": self.run["created_at"]},
            {"__typename": "RemovedFromMergeQueueEvent", "id": "removal-1", "reason": "failed_checks",
             "createdAt": self.run["updated_at"], "beforeCommit": {"oid": self.run["head_sha"]}},
        ]

    def command(self, argv, **kwargs):
        self.reads.append(argv)
        if argv[:3] == ["gh", "pr", "merge"] or "POST" in argv:
            self.mutations.append(argv)
            if "POST" in argv and any("/comments" in a for a in argv):
                self.comments.append({"body": next(a[5:] for a in argv if a.startswith("body="))})
            value = None
        elif argv[0] == "uv":
            assert any(a.endswith("gh_graphql.py") for a in argv), argv
            self.pr_reads += 1
            pr = copy.deepcopy(self.pr)
            if self.changed_pr and self.pr_reads > 1:
                pr["headRefOid"] = "c" * 40
            pr["labels"] = {"nodes": pr["labels"], "pageInfo": {"hasNextPage": False}}
            pr["timelineItems"] = {"nodes": self.events, "pageInfo": {"hasPreviousPage": False, "startCursor": None}}
            value = {"data": {"repository": {"pullRequest": pr}}}
        else:
            value = self.rest(argv)
        return subprocess.CompletedProcess(argv, 0, json.dumps(value), "")

    def rest(self, argv):
        endpoint = next(a for a in argv if a.startswith(f"repos/{REPO}/"))
        if "/check-runs/" in endpoint:
            if self.fail_annotations:
                raise subprocess.CalledProcessError(1, argv)
            job = endpoint.split("/check-runs/")[1].split("/")[0]
            value = [self.fixture["annotations"][job]]
        elif "/comments" in endpoint:
            value = [self.comments]
        elif "/jobs?" in endpoint:
            value = [self.fixture["jobs"]]
        elif endpoint.endswith("workflows/ci.yml"):
            value = {"id": 386287070, "path": ".github/workflows/ci.yml"}
        elif "/runs?" in endpoint:
            value = [{"total_count": 1, "workflow_runs": [self.run]}]
        else:
            value = dict(self.run)
            self.run_reads += 1
            if ((self.changed_run and self.run_reads > 1)
                    or (self.advance_on_live and self.run_reads > 2)):
                value["run_attempt"] += 1
        return value


class RetryTest(unittest.TestCase):
    def setUp(self):
        self.api = RecordedApi()

    def run_bot(self, dry_run=False, sweep=False, token=True):
        output = io.StringIO()
        with patch.object(subprocess, "run", side_effect=self.api.command), redirect_stdout(output):
            result = retry.maintain(REPO, run_id=None if sweep else self.api.run["id"],
                                    dry_run=dry_run, can_requeue=token)
        return result, output.getvalue()

    def assert_no_mutations(self):
        self.run_bot()
        self.assertEqual(self.api.mutations, [])

    def test_ejected_group_rearms_same_head_instead_of_rerunning_dead_run(self):
        self.assertEqual(self.run_bot()[0], 0)
        self.assertEqual(len(self.api.mutations), 2)
        self.assertIn("/comments", " ".join(self.api.mutations[0]))
        self.assertEqual(self.api.mutations[1][-3:], ["--auto", "--match-head-commit", self.api.pr["headRefOid"]])
        self.assertFalse(any("rerun" in " ".join(a) for a in self.api.mutations))

    def test_pr_capacity_reruns_only_failed_jobs_once(self):
        self.api = RecordedApi("pull_request")
        self.run_bot()
        self.assertEqual(len(self.api.mutations), 1)
        self.assertIn(f"repos/{REPO}/actions/runs/44318137015/rerun-failed-jobs", self.api.mutations[0])
        self.api.run["run_attempt"] = 2
        self.api.mutations.clear()
        self.assert_no_mutations()

    def test_schedule_finds_departed_groups_without_live_queue_inventory(self):
        self.run_bot(sweep=True)
        self.assertEqual(len(self.api.mutations), 2)
        self.assertFalse(any("mergeQueue{" in " ".join(a) for a in self.api.reads))

    def test_same_head_retry_budget_survives_new_run_id(self):
        self.run_bot()
        self.api.mutations.clear()
        self.api.run["id"] += 1
        self.assert_no_mutations()

    def test_capacity_requeue_requires_triggering_token(self):
        self.run_bot(token=False)
        self.assertEqual(self.api.mutations, [])

    def test_already_armed_ejected_pr_can_recover(self):
        self.api.pr["autoMergeRequest"] = {"enabledAt": self.api.run["created_at"]}
        self.run_bot()
        self.assertEqual(len(self.api.mutations), 2)

    def test_negative_queue_evidence(self):
        for change in ("manual", "wrong_group", "changed_head", "no_prior_enqueue", "disarmed", "already_requeued"):
            with self.subTest(change=change):
                self.api = RecordedApi()
                if change == "manual":
                    self.api.events[-1]["reason"] = "manual"
                elif change == "wrong_group":
                    self.api.events[-1]["beforeCommit"]["oid"] = "a" * 40
                elif change == "changed_head":
                    self.api.pr["headRefOid"] = "a" * 40
                elif change == "no_prior_enqueue":
                    self.api.events.pop(1)
                elif change == "disarmed":
                    self.api.events.append({"__typename": "AutoMergeDisabledEvent"})
                else:
                    self.api.events.append({"__typename": "AddedToMergeQueueEvent"})
                self.assert_no_mutations()

    def test_pr_holds_and_unknown_mergeability(self):
        for values in ({"state": "CLOSED"}, {"isDraft": True}, {"mergeable": "CONFLICTING"},
                       {"mergeable": "UNKNOWN"}, {"reviewDecision": "CHANGES_REQUESTED"},
                       {"labels": [{"name": "queue-failed"}]}, {"body": "Coordinator hold: do not enqueue"}):
            with self.subTest(values=values):
                self.api = RecordedApi()
                self.api.pr.update(values)
                self.assert_no_mutations()

    def test_concurrent_change_prevents_mutation(self):
        for attr in ("changed_run", "changed_pr"):
            with self.subTest(attr=attr):
                self.api = RecordedApi()
                setattr(self.api, attr, True)
                self.assert_no_mutations()

    def test_real_failure_and_uncertain_annotations_never_retry(self):
        self.api.fixture["jobs"]["jobs"][0]["conclusion"] = "failure"
        self.assert_no_mutations()
        self.api = RecordedApi()
        self.api.fail_annotations = True
        self.assertEqual(self.run_bot()[0], 1)
        self.assertEqual(self.api.mutations, [])

    def test_incomplete_jobs_inventory_never_retries(self):
        self.api.fixture["jobs"]["total_count"] += 1
        self.assertEqual(self.run_bot()[0], 1)
        self.assertEqual(self.api.mutations, [])

    def test_capacity_and_gate_evidence_must_be_exact(self):
        for change in ("started", "steps", "missing", "inexact", "extra", "gate_setup", "gate_scope", "gate_missing", "lone_gate"):
            with self.subTest(change=change):
                self.api = RecordedApi()
                jobs = self.api.fixture["jobs"]["jobs"]
                cancelled = next(j for j in jobs if j["conclusion"] == "cancelled")
                gate = next(j for j in jobs if j["name"] == "CI Gate ✅")
                annotations = self.api.fixture["annotations"]
                cap = annotations[str(cancelled["id"])]
                if change == "started":
                    cancelled["runner_id"] = 123
                elif change == "steps":
                    cancelled["steps"] = [{"name": "Set up job", "conclusion": "success"}]
                elif change == "missing":
                    annotations[str(cancelled["id"])] = []
                elif change == "inexact":
                    cap[0]["message"] += " other"
                elif change == "extra":
                    cap.append({"annotation_level": "failure", "message": "assertion failed"})
                elif change == "gate_setup":
                    gate["steps"][0]["conclusion"] = "failure"
                elif change == "gate_scope":
                    annotations[str(gate["id"])].append({"annotation_level": "failure", "message": "scope unresolved"})
                elif change == "gate_missing":
                    annotations[str(gate["id"])] = []
                else:
                    cancelled["conclusion"] = "success"
                self.assert_no_mutations()

    def test_unfinished_success_or_unrelated_workflow_never_retries(self):
        for values in ({"status": "in_progress"}, {"conclusion": "success"}, {"event": "push"},
                       {"workflow_id": 7}, {"head_branch": "untrusted"}):
            with self.subTest(values=values):
                self.api = RecordedApi()
                self.api.run.update(values)
                self.assert_no_mutations()

    def live_group(self, state="UNMERGEABLE"):
        self.api = RecordedApi("live_group")
        self.api.events.pop()  # the capacity-failed group is still present
        self.api.pr["mergeQueueEntry"] = {"state": state, "headCommit": {"oid": self.api.run["head_sha"]}}

    def test_capacity_group_retries_while_present_regardless_of_queue_state(self):
        for state in ("UNMERGEABLE", "AWAITING_CHECKS"):
            with self.subTest(state=state):
                self.live_group(state)
                self.assertEqual(self.run_bot(token=False)[0], 0)
                self.assertEqual(len(self.api.mutations), 1)
                self.assertIn("44351072142/rerun-failed-jobs", " ".join(self.api.mutations[0]))
                self.api.mutations.clear()
                self.api.run["run_attempt"] = 2
                self.assert_no_mutations()

    def test_live_capacity_retry_never_follows_a_changed_group_or_real_conflict(self):
        for change in ("group", "conflict", "gate_error", "fresh_head", "disarmed"):
            with self.subTest(change=change):
                self.live_group()
                if change == "group":
                    self.api.pr["mergeQueueEntry"]["headCommit"]["oid"] = "a" * 40
                elif change == "conflict":
                    self.api.pr["mergeable"] = "CONFLICTING"
                elif change == "fresh_head":
                    self.api.changed_pr = True
                elif change == "disarmed":
                    self.api.events.append({"__typename": "AutoMergeDisabledEvent"})
                else:
                    self.api.fixture["annotations"]["134992644660"].append({
                        "annotation_level": "failure", "message": "scope is unknown"})
                self.assert_no_mutations()

    def test_live_manual_retry_during_membership_reads_prevents_duplicate(self):
        self.live_group()
        self.api.advance_on_live = True
        self.assert_no_mutations()

    def test_live_capacity_dry_run_never_mutates(self):
        self.live_group()
        self.assertIn("dry-run", self.run_bot(dry_run=True, token=False)[1])
        self.assertEqual(self.api.mutations, [])

    def test_real_failed_checks_timeline_and_uncached_authorization_read(self):
        fixture = json.loads((ROOT / "ci/fixtures/mq-acquire-retry.json").read_text())["observed_ejection"]
        pr = dict(self.api.pr, headRefOid=fixture["headRefOid"], events=fixture["events"])
        run = dict(self.api.run, head_sha=fixture["events"][-1]["beforeCommit"]["oid"])
        self.assertEqual(retry.ejection(pr, run), fixture["events"][-1])
        self.run_bot()
        queries = [next(a for a in argv if a.startswith("query=")) for argv in self.api.reads if argv[0] == "uv"]
        self.assertEqual(len(queries), 2)
        self.assertNotEqual(queries[0], queries[1])
        self.assertIn("query CapacityRecheck", queries[1])

    def test_missing_gate_and_exhausted_attempt_never_retry(self):
        for scenario in ("missing_gate", "attempt_three", "unknown_attempt"):
            with self.subTest(scenario=scenario):
                self.api = RecordedApi()
                if scenario == "missing_gate":
                    jobs = self.api.fixture["jobs"]["jobs"]
                    self.api.fixture["jobs"]["jobs"] = [j for j in jobs if j["name"] != "CI Gate ✅"]
                    self.api.fixture["jobs"]["total_count"] -= 1
                else:
                    self.api.run["run_attempt"] = 3 if scenario == "attempt_three" else None
                self.assert_no_mutations()

    def test_legacy_handler_cannot_bypass_strict_runnerless_evidence(self):
        bot = load_ci("ai_rebase")
        with patch.object(bot, "requeue_api_items", return_value=[self.api.run]), \
                patch.object(bot, "run_jobs", return_value=self.api.fixture["jobs"]["jobs"]):
            self.assertEqual(bot.queue_failure_kind(self.api.events[-1]), "unknown")

    def test_dry_run_has_no_mutations(self):
        self.assertIn("dry-run", self.run_bot(dry_run=True)[1])
        self.assertEqual(self.api.mutations, [])


class WorkflowTest(unittest.TestCase):
    def test_scheduled_recovery_is_isolated_from_rebase_and_untrusted_checkout(self):
        guard = load_ci("test_ai_rebase_token_guard")
        workflow = (ROOT / ".github/workflows/ai-rebase.yaml").read_text()
        self.assertEqual(workflow.count("workflow_run:"), 1)  # the runner-loss-retry trigger only
        jobs = guard.jobs(workflow)
        recovery = jobs["capacity-recovery"]
        self.assertIn("ci/mq_acquire_retry.py $DRY", recovery)
        self.assertNotIn("RUN_ID", recovery)
        self.assertIn("ci/ai_rebase.py --requeue-ejected --limit 10", recovery)
        self.assertIn("ref: master", recovery)
        self.assertIn("persist-credentials: false", recovery)
        self.assertIn("steps.app.outputs.token || github.token", recovery)
        self.assertIn("github.event_name != 'schedule'", jobs["rebase"])
        self.assertIn('17,47 * * * *', workflow)
        self.assertNotIn("ci/mq_acquire_retry.py", jobs["rebase"])


class GhColorTests(unittest.TestCase):
    """setup-soldr exports CLICOLOR_FORCE=1 and FORCE_COLOR=1 to the job, and gh
    then colors its --json output even into a pipe, so json.loads fails (ai-rebase
    run 45002524272). gh JSON is always read with color off."""

    def test_gh_json_runs_gh_with_color_off(self):
        import os
        import subprocess as sp
        from unittest.mock import patch
        mod = retry
        seen = []

        def fake(*args, **kwargs):
            seen.append(kwargs.get("env"))
            return sp.CompletedProcess(args[0] if args else [], 0, stdout='[{"number": 1}]', stderr="")

        forced = {"CLICOLOR_FORCE": "1", "FORCE_COLOR": "1", "GH_FORCE_TTY": "1", "PATH": os.environ.get("PATH", "")}
        with patch.dict("os.environ", forced, clear=True), patch.object(mod.subprocess, "run", side_effect=fake):
            self.assertEqual(mod.command(["gh", "pr", "list", "--json", "number"]), [{"number": 1}])
        self.assertTrue(seen)
        for env in seen:
            self.assertIsNotNone(env)
            self.assertEqual(env["CLICOLOR_FORCE"], "0")
            self.assertEqual(env["NO_COLOR"], "1")
            self.assertNotIn("GH_FORCE_TTY", env)
            self.assertNotIn("FORCE_COLOR", env)


if __name__ == "__main__":
    unittest.main()
