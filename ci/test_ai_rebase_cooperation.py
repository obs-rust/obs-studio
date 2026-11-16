#!/usr/bin/env -S uv run --script
"""Rebase-bot cooperation contracts; no network, pushes or model calls."""
import contextlib
import datetime as dt
import importlib.util
import io
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
                                        "afterCommit": {"oid": HEAD}}], True)]
        for timeline, allowed in cases:
            data = {"data": {"repository": {"pullRequest": {
                "headRefOid": HEAD, "timelineItems": {"nodes": timeline}}}}}
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


if __name__ == "__main__":
    unittest.main()
