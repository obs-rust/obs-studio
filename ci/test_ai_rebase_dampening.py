#!/usr/bin/env -S uv run --script
"""Persistent rebase budgets, tested without clocks, models or GitHub."""
import contextlib
import datetime as dt
import importlib.util
import io
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("bot", Path(__file__).with_name("ai_rebase.py"))
bot = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bot)
NOW = dt.datetime(2026, 11, 23, 12, tzinfo=dt.timezone.utc)
HEAD = "a" * 40


def comment(age=1, outcome="resolution_refused", head=HEAD):
    when = (NOW - dt.timedelta(hours=age)).isoformat()
    return {"user": {"login": "obs-rust-ai-rebase[bot]", "type": "Bot"},
            "body": f"<!-- ai-rebase {head} {when} {outcome} -->"}


class DampeningTests(unittest.TestCase):
    def select(self, prs, history=None, recent=None, **kw):
        with patch.object(bot, "requeue_comments", return_value=history or []), \
                patch.object(bot, "recent_attempt_comments", return_value=recent or []):
            return bot.select_rebases(prs, now=NOW, **kw)

    def test_same_head_cooldown_includes_refusals(self):
        for outcome in ("pushed", "resolution_refused", "error", "started"):
            with self.subTest(outcome=outcome):
                self.assertEqual(self.select([{ "number": 1, "headRefOid": HEAD}],
                                             [comment(outcome=outcome)]), [])

    def test_new_head_and_expired_cooldown(self):
        for history in ([comment(head="b" * 40)], [comment(age=6)], [comment(age=7)]):
            self.assertEqual(len(self.select([{"number": 1, "headRefOid": HEAD}], history)), 1)

    def test_run_cap_selects_three_of_ten_and_names_reason(self):
        prs = [{"number": n, "headRefOid": HEAD} for n in range(10)]
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(len(self.select(prs)), 3)
        self.assertIn("per-run cap", out.getvalue())

    def test_hour_cap_counts_attempts_across_heads_and_outcomes(self):
        prs = [{"number": n, "headRefOid": HEAD} for n in range(10)]
        with contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(len(self.select(prs, recent=[comment(age=0.5)] * 5)), 1)
        self.assertIn("per-hour cap", out.getvalue())
        self.assertEqual(len(self.select(prs, recent=[comment(age=1)] * 6)), 3)

    def test_untrusted_comment_cannot_consume_budget(self):
        forged = comment()
        forged["user"] = {"login": "someone", "type": "User"}
        self.assertEqual(len(self.select([{"number": 1, "headRefOid": HEAD}], [forged])), 1)

    def test_workflow_has_only_schedule_and_manual_triggers(self):
        workflow = Path(".github/workflows/ai-rebase.yaml").read_text()
        triggers = workflow.split("on:", 1)[1].split("permissions:", 1)[0]
        self.assertNotIn("workflow_run:", triggers)
        self.assertNotIn("push:", triggers)
        self.assertIn("schedule:", triggers)
        self.assertIn("workflow_dispatch:", triggers)
        self.assertIn("ci/mq_acquire_retry.py", workflow)


if __name__ == "__main__":
    unittest.main()
