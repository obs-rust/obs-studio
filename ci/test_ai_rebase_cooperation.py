#!/usr/bin/env -S uv run --script
"""Rebase-bot cooperation contracts; no network, pushes or model calls."""
import contextlib
import datetime as dt
import importlib.util
import io
import json
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("ai_rebase", Path(__file__).with_name("ai_rebase.py"))
bot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bot)
HEAD = "a" * 40
NOW = dt.datetime(2026, 11, 16, 23, tzinfo=dt.timezone.utc)


def pr(number=1, **changes):
    return {"number": number, "headRefName": "fix/probe", "headRefOid": HEAD,
            "state": "OPEN", "isCrossRepository": False, "isDraft": False,
            "labels": [], "body": "", "mergeStateStatus": "DIRTY",
            "createdAt": "2026-11-15T12:00:00Z", "updatedAt": "2026-11-16T20:00:00Z",
            **changes}


def commit(sha=HEAD):
    return {"__typename": "PullRequestCommit", "commit": {"oid": sha}}


def removal(reason="failed_checks"):
    return {"__typename": "RemovedFromMergeQueueEvent", "reason": reason,
            "beforeCommit": {"oid": "b" * 40}}


def history(nodes, *, head=HEAD, previous=False, cursor=None):
    return {"data": {"repository": {"pullRequest": {
        "headRefOid": head, "timelineItems": {"nodes": nodes,
            "pageInfo": {"hasPreviousPage": previous, "startCursor": cursor}}}}}}


class CooperationTests(unittest.TestCase):
    def test_arm_uses_queue_strategy_and_exact_head(self):
        with patch.object(bot, "run", return_value="") as run, \
                patch.object(bot, "should_enqueue", return_value=(True, "")), \
                patch.object(bot, "preflight", return_value=(True, "")):
            self.assertEqual(bot.publish(1, pr(), None, "new"), "auto-merge armed")
        arm = next(c for c in run.call_args_list if c.args[:3] == ("gh", "pr", "merge"))
        self.assertNotIn("--squash", arm.args)
        self.assertIn("new", arm.args)
        self.assertTrue(arm.kwargs.get("check", True))
        self.assertIn(f"--force-with-lease=refs/heads/fix/probe:{HEAD}", run.call_args_list[0].args)

    def test_failed_arm_is_reported_after_successful_push(self):
        def run(*args, **kwargs):
            if args[:3] == ("gh", "pr", "merge") and kwargs.get("check", True):
                raise RuntimeError("queue rejected arm")
            return ""
        with patch.object(bot, "run", side_effect=run), \
                patch.object(bot, "should_enqueue", return_value=(True, "")), \
                patch.object(bot, "preflight", return_value=(True, "")):
            outcome = bot.publish(1, pr(), None, "new")
        self.assertIn("failed to arm", outcome)
        self.assertIn("queue rejected arm", outcome)

    def test_single_pr_skips_worker_before_fetch_or_checkout(self):
        for active in [pr(labels=[{"name": "pool-active"}]),
                       pr(updatedAt="2999-01-01T00:00:00Z")]:
            with self.subTest(active=active), patch.object(bot, "gh_json", return_value=active), \
                    patch.object(bot, "run") as run:
                self.assertEqual(bot.rebase_pr(1, False), "skipped: worker active")
                run.assert_not_called()

    def test_bulk_skips_workers_before_spending_fifo_headroom(self):
        candidates = [pr(1, labels=[{"name": "pool-active"}]),
                      pr(2, updatedAt="2999-01-01T00:00:00Z"),
                      pr(4, createdAt="2026-11-16T11:00:00Z"), pr(3)]
        with patch.object(bot, "gh_json", return_value=candidates), contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(bot.dirty_prs(1), [3])
        self.assertIn("#1 skipped: worker active", out.getvalue())
        self.assertIn("#2 skipped: worker active", out.getvalue())

    def test_worker_quiet_period_boundary(self):
        for minutes, expected in [(29, True), (30, False), (31, False)]:
            with self.subTest(minutes=minutes):
                stamp = (NOW - dt.timedelta(minutes=minutes)).isoformat()
                self.assertEqual(bot.worker_active(pr(updatedAt=stamp), now=NOW), expected)

    def test_queue_failure_label_never_arms(self):
        with patch.object(bot, "gh_json") as api:
            self.assertEqual(bot.should_enqueue(pr(labels=[{"name": "queue-failed"}])),
                             (False, "failed in the merge queue"))
            api.assert_not_called()

    def test_latest_removal_is_bound_to_current_head(self):
        cases = [([commit(), removal()], False),
                 ([commit(), removal("conflicts")], True),
                 ([commit(), removal(), removal("manual")], True),
                 ([commit("c" * 40), removal(), commit()], True),
                 ([commit(), removal(), {"__typename": "HeadRefForcePushedEvent",
                                        "afterCommit": {"oid": HEAD}}], False),
                 ([commit(), commit("c" * 40), removal(), {"__typename": "HeadRefForcePushedEvent",
                                        "afterCommit": {"oid": HEAD}}], True)]
        for timeline, allowed in cases:
            data = history(timeline)
            with self.subTest(timeline=timeline), patch.object(bot, "gh_json", return_value=data):
                go, why = bot.should_enqueue(pr())
            self.assertEqual(go, allowed)
            if not allowed:
                self.assertEqual(why, "failed in the merge queue")

    def test_publish_remembers_failed_queue_before_rebase_push(self):
        with patch.object(bot, "run", return_value="") as run, \
                patch.object(bot, "should_enqueue", return_value=(False, "failed in the merge queue")):
            self.assertEqual(bot.publish(1, pr(), None, "new"), "left out: failed in the merge queue")
        self.assertFalse(any(c.args[:3] == ("gh", "pr", "merge") for c in run.call_args_list))

    def test_queue_history_pages_back_to_head(self):
        pages = [history([removal()], previous=True, cursor="older"), history([commit()])]
        with patch.object(bot, "gh_json", side_effect=pages) as api:
            self.assertEqual(bot.should_enqueue(pr()), (False, "failed in the merge queue"))
        self.assertIn("before=older", api.call_args.args)

    def test_unknown_failure_history_refuses_before_mutation(self):
        bad = [history([removal()]), history([commit(), removal()], head="new"),
               {"errors": [{"message": "API unavailable"}]}]
        for data in bad:
            with self.subTest(data=data), patch.object(bot, "gh_json", side_effect=[pr(), data]), \
                    patch.object(bot, "run") as run:
                with self.assertRaises(RuntimeError):
                    bot.rebase_pr(1, False)
                run.assert_not_called()

    def test_failure_snapshot_survives_rebase(self):
        commands = []
        def run(*args, **kwargs):
            commands.append(args)
            if args[:2] == ("git", "rev-parse"):
                return HEAD if len([c for c in commands if c[:2] == args[:2]]) == 1 else "new"
            return ""
        with patch.object(bot, "gh_json", side_effect=[pr(), history([commit(), removal()])]) as api, \
                patch.object(bot, "run", side_effect=run), patch.object(bot, "replay", return_value=0):
            self.assertIn("left out: failed in the merge queue", bot.rebase_pr(1, False))
        self.assertEqual(api.call_count, 2)
        self.assertFalse(any(c[:3] == ("gh", "pr", "merge") for c in commands))

    def test_graphql_uses_shared_wrapper(self):
        with patch.object(bot, "run", return_value=json.dumps(history([]))) as run:
            bot.failed_in_queue(pr())
        self.assertEqual(run.call_args.args[:4], ("uv", "run", "--no-project", "--script"))
        self.assertTrue(run.call_args.args[4].endswith("gh_graphql.py"))

    def test_failed_arm_outcome_sets_nonzero_exit(self):
        with patch.object(bot.sys, "argv", ["ai_rebase.py", "1"]), \
                patch.object(bot, "run", return_value=HEAD), \
                patch.object(bot, "rebase_pr", return_value="pushed: old -> new; failed to arm auto-merge: rejected"), \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(bot.main(), 1)

    def test_stacked_draft_still_becomes_ready_and_arms(self):
        stack = (42, "c" * 40, "MERGED")
        stacked = pr(isDraft=True, body=f"Stacked on #42: review only the commits after {stack[1]}.")
        with patch.object(bot, "run", return_value="") as run, \
                patch.object(bot, "should_enqueue", return_value=(True, "")), \
                patch.object(bot, "preflight", return_value=(True, "")):
            self.assertEqual(bot.publish(1, stacked, stack, "new"), "auto-merge armed")
        self.assertFalse(stacked["isDraft"])
        self.assertNotIn("Stacked on", stacked["body"])
        self.assertTrue(any(c.args[:3] == ("gh", "pr", "ready") for c in run.call_args_list))


def ejection(reason="failed_checks", **changes):
    return {**removal(reason), "createdAt": "2026-11-16T22:00:00Z", **changes}


def queue_job(step="Run tests", **changes):
    return {"id": 101, "name": "Fast Gate", "conclusion": "failure",
            "runner_name": "GitHub Actions 1", "labels": ["ubuntu-24.04"],
            "started_at": "2026-11-16T21:30:00Z",
            "steps": [{"name": step, "conclusion": "failure"}], **changes}


class RequeueTests(unittest.TestCase):
    def candidate(self, **changes):
        return pr(autoMergeRequest=None, mergeStateStatus="CLEAN", **changes)

    def invoke(self, *, candidate=None, events=None, verdict="infra", comments=None,
               refreshed=None, dry_run=False):
        candidate = self.candidate() if candidate is None else candidate
        events = [ejection(), commit()] if events is None else events
        with patch.object(bot, "queue_events", return_value=iter(events)), \
                patch.object(bot, "queue_failure_kind", return_value=verdict), \
                patch.object(bot, "requeue_comments", return_value=comments or []), \
                patch.object(bot, "gh_json", return_value=refreshed or candidate), \
                patch.object(bot, "run", return_value="") as run:
            out = bot.requeue_pr(candidate, dry_run, now=NOW)
        return out, run.call_args_list

    def test_infra_rearms_with_marker_and_exact_head_without_strategy(self):
        out, calls = self.invoke()
        self.assertIn("re-armed", out)
        self.assertEqual(len(calls), 2)
        self.assertIn("comment", calls[0].args)
        self.assertIn(bot.requeue_marker(HEAD), calls[0].args[-1])
        self.assertEqual(calls[1].args, ("gh", "pr", "merge", "1", "--repo", bot.REPO,
                                      "--auto", "--match-head-commit", HEAD))

    def test_timeout_needs_no_run_evidence(self):
        with patch.object(bot, "queue_failure_kind") as classify:
            with patch.object(bot, "queue_events", return_value=iter([ejection("checks_timed_out"), commit()])), \
                    patch.object(bot, "requeue_comments", return_value=[]), \
                    patch.object(bot, "gh_json", return_value=self.candidate()), \
                    patch.object(bot, "run", return_value=""):
                self.assertIn("re-armed", bot.requeue_pr(self.candidate(), False, now=NOW))
            classify.assert_not_called()

    def test_real_failure_labels_without_arming(self):
        out, calls = self.invoke(verdict="code")
        self.assertIn("queue-failed", out)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0].args[-2:], ("--add-label", "queue-failed"))

    def test_unknown_evidence_does_nothing(self):
        out, calls = self.invoke(verdict="unknown")
        self.assertIn("skipped", out)
        self.assertEqual(calls, [])

    def test_cap_is_per_head_and_survives_later_ejections(self):
        for sha, allowed in [(HEAD, False), ("c" * 40, True)]:
            out, calls = self.invoke(comments=[{"body": bot.requeue_marker(sha)}])
            self.assertEqual("re-armed" in out, allowed)
            self.assertEqual(bool(calls), allowed)

    def test_holds_drafts_workers_auto_merge_and_conflicts(self):
        cases = [{"isDraft": True}, {"body": "Hold until review"},
                 {"labels": [{"name": "do not enqueue"}]},
                 {"labels": [{"name": "queue-failed"}]},
                 {"labels": [{"name": "pool-active"}]},
                 {"updatedAt": "2999-01-01T00:00:00Z"},
                 {"reviewDecision": "CHANGES_REQUESTED"},
                 {"autoMergeRequest": {"enabledAt": "now"}},
                 {"mergeStateStatus": "DIRTY"}, {"state": "CLOSED"}]
        for changes in cases:
            with self.subTest(changes=changes):
                candidate = {**self.candidate(), **changes}
                out, calls = self.invoke(candidate=candidate)
                self.assertIn("skipped", out)
                self.assertEqual(calls, [])

    def test_latest_transition_freshness_and_unchanged_head(self):
        cases = [[ejection("conflicts"), commit()], [ejection("manual"), commit()],
                 [{"__typename": "AddedToMergeQueueEvent"}, ejection(), commit()],
                 [commit(), ejection(), commit()],
                 [ejection(), commit("c" * 40)], [ejection()],
                 [ejection(createdAt="2026-11-16T19:59:59Z"), commit()],
                 [ejection(createdAt="2026-11-17T00:00:00Z"), commit()]]
        for events in cases:
            with self.subTest(events=events):
                out, calls = self.invoke(events=events)
                self.assertIn("skipped", out)
                self.assertEqual(calls, [])

    def test_refresh_rechecks_ownership_holds_head_and_new_ejection(self):
        for changes in [{"headRefOid": "new"}, {"body": "hold"},
                        {"autoMergeRequest": {"enabledAt": "now"}},
                        {"labels": [{"name": "pool-active"}]}]:
            with self.subTest(changes=changes):
                out, calls = self.invoke(refreshed={**self.candidate(), **changes})
                self.assertIn("skipped", out)
                self.assertEqual(calls, [])

    def test_dry_run_has_no_mutations(self):
        for verdict in ["infra", "code"]:
            out, calls = self.invoke(verdict=verdict, dry_run=True)
            self.assertIn("dry-run", out)
            self.assertEqual(calls, [])

    def test_marker_failure_prevents_arm_and_arm_failure_consumes_cap(self):
        for failed_command in ["comment", "merge"]:
            commands = []
            def fail(*args, **kwargs):
                commands.append(args)
                if args[2] == failed_command:
                    raise RuntimeError("API failed")
                return ""
            with self.subTest(command=failed_command), \
                    patch.object(bot, "queue_events", return_value=iter([ejection(), commit()])), \
                    patch.object(bot, "queue_failure_kind", return_value="infra"), \
                    patch.object(bot, "requeue_comments", return_value=[]), \
                    patch.object(bot, "gh_json", return_value=self.candidate()), \
                    patch.object(bot, "run", side_effect=fail):
                with self.assertRaises(RuntimeError):
                    bot.requeue_pr(self.candidate(), False, now=NOW)
            self.assertEqual(len(commands), 1 if failed_command == "comment" else 2)

    def test_failed_job_classification(self):
        cases = [("Run actions/checkout@pin", "", "infra"),
                 ("Set Up soldr 🦀", "", "infra"),
                 ("Restore compiler cache", "", "infra"),
                 ("Run tests", "The runner has lost communication with the server.", "infra"),
                 ("Build", "fatal: Could not resolve host: github.com", "infra"),
                 ("Run tests", "assertion failed: wrong value", "code"),
                 ("Build", "error[E0425]: cannot find value", "code"),
                 ("Run tests", "assertion failed: network is unreachable", "code"),
                 ("Something else", "unrecognized failure", "unknown")]
        for step, log, expected in cases:
            with self.subTest(step=step, log=log), patch.object(bot, "run", return_value=log), \
                    patch.object(bot, "passed_elsewhere", return_value=False):
                self.assertEqual(bot.failed_job_kind({"id": 1}, queue_job(step)), expected)

    def test_self_hosted_control_is_same_job_other_runner_within_hour(self):
        failed = queue_job(labels=["self-hosted"], runner_name="dell-4")
        candidate_run = {"id": 2, "workflow_id": 7}
        original_run = {"id": 1, "workflow_id": 7}
        cases = [({}, True), ({"runner_name": "dell-4"}, False),
                 ({"name": "Other tests"}, False), ({"conclusion": "failure"}, False),
                 ({"started_at": "2026-11-16T20:29:59Z"}, False)]
        for changes, expected in cases:
            success = queue_job(conclusion="success", runner_name="dell-1", **changes) if not changes else {
                **queue_job(conclusion="success", runner_name="dell-1"), **changes}
            with self.subTest(changes=changes), patch.object(bot, "requeue_api_items", return_value=[candidate_run]), \
                    patch.object(bot, "run_jobs", return_value=[success]):
                self.assertEqual(bot.passed_elsewhere(original_run, failed), expected)

    def test_group_classification_checks_all_failures_and_ignores_only_dependent_gate(self):
        group = {"id": 1, "head_sha": "b" * 40, "event": "merge_group", "conclusion": "failure"}
        for conclusions, kinds, expected in [(["cancelled"], [], "infra"),
                (["failure"], ["infra"], "infra"),
                (["failure", "failure"], ["infra", "code"], "code"),
                (["failure"], ["unknown"], "unknown")]:
            jobs = [queue_job(id=i, name=f"job {i}", conclusion=c) for i, c in enumerate(conclusions)]
            jobs.append(queue_job(name="CI Gate ✅"))
            with self.subTest(kinds=kinds), patch.object(bot, "requeue_api_items", return_value=[group]), \
                    patch.object(bot, "run_jobs", return_value=jobs), \
                    patch.object(bot, "failed_job_kind", side_effect=kinds):
                self.assertEqual(bot.queue_failure_kind(ejection()), expected)

    def test_empty_or_wrong_group_evidence_is_unknown(self):
        for runs in [[], [{"id": 1, "head_sha": "wrong", "event": "merge_group", "conclusion": "cancelled"}]]:
            with self.subTest(runs=runs), patch.object(bot, "requeue_api_items", return_value=runs):
                self.assertEqual(bot.queue_failure_kind(ejection()), "unknown")

    def test_mode_does_not_checkout_fetch_push_or_call_model(self):
        with patch.object(bot.sys, "argv", ["ai_rebase.py", "--requeue-ejected"]), \
                patch.object(bot, "requeue_ejected", return_value=0) as requeue, \
                patch.object(bot, "run") as run, patch.object(bot, "ask_model") as model:
            self.assertEqual(bot.main(), 0)
            requeue.assert_called_once()
            run.assert_not_called()
            model.assert_not_called()


if __name__ == "__main__":
    unittest.main()
