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

    def test_headroom_is_an_additional_cap(self):
        prs = [{"number": n, "headRefOid": HEAD} for n in range(10)]
        self.assertEqual(self.select(prs, headroom=0), [])
        self.assertEqual(len(self.select(prs, headroom=1)), 1)

    def test_model_and_token_errors_after_reservation_are_nonfatal(self):
        candidate = {"number": 1, "headRefOid": HEAD}
        for error in (RuntimeError("model unavailable"), RuntimeError("push token rejected")):
            with self.subTest(error=str(error)), \
                    patch.object(bot.sys, "argv", ["ai_rebase.py", "1"]), \
                    patch.object(bot, "select_rebases", return_value=[candidate]), \
                    patch.object(bot, "gh_json", side_effect=[candidate, {"id": 123}]), \
                    patch.object(bot, "run", return_value=HEAD) as run, \
                    patch.object(bot, "rebase_pr", side_effect=error), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(bot.main(), 0)
                updates = [c for c in run.call_args_list if "PATCH" in c.args]
                self.assertEqual(len(updates), 1)
                self.assertIn("error -->", updates[0].args[-1])

    def test_no_attempt_when_reservation_fails_is_broken_automation(self):
        candidate = {"number": 1, "headRefOid": HEAD}
        with patch.object(bot.sys, "argv", ["ai_rebase.py", "1"]), \
                patch.object(bot, "select_rebases", return_value=[candidate]), \
                patch.object(bot, "gh_json", side_effect=[candidate, RuntimeError("token rejected")]), \
                patch.object(bot, "run", return_value=HEAD), \
                patch.object(bot, "rebase_pr") as rebase, \
                contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(bot.main(), 1)
            rebase.assert_not_called()

    def test_unreadable_backpressure_is_a_closed_queue_not_a_crash(self):
        # #5797: one failed GitHub read during --dirty preparation crashed the
        # scheduled sweep with a traceback and hid gh's stderr.
        error = bot.subprocess.CalledProcessError(1, ["gh", "pr", "list"], stderr="HTTP 502: Bad Gateway\n")
        open_queue = {"state": "open", "headroom": 2}
        for name, state, dirty in (("measure", error, []), ("dirty_prs", open_queue, error)):
            with self.subTest(failing=name), \
                    patch.object(bot.sys, "argv", ["ai_rebase.py", "--dirty"]), \
                    patch.object(bot, "queue_state", side_effect=[state]), \
                    patch.object(bot, "dirty_prs", side_effect=[dirty]), \
                    patch.object(bot, "gh_json", side_effect=AssertionError("unexpected GitHub call")), \
                    patch.object(bot, "rebase_pr") as rebase, \
                    contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(bot.main(), 0)
                rebase.assert_not_called()
                self.assertIn("::warning::", out.getvalue())
                self.assertIn("HTTP 502: Bad Gateway", out.getvalue())

    def test_changed_head_after_selection_is_skipped(self):
        with patch.object(bot, "gh_json", return_value={"headRefOid": "b" * 40}), \
                patch.object(bot, "run") as run:
            self.assertIn("head changed", bot.rebase_pr(1, False, expected_head=HEAD))
            run.assert_not_called()

    def test_workflow_has_no_push_trigger_and_workflow_run_reaches_only_the_retry_job(self):
        workflow = Path(".github/workflows/ai-rebase.yaml").read_text()
        triggers = workflow.split("\non:", 1)[1].split("permissions:", 1)[0]
        self.assertIn("workflow_run:", triggers)  # runner-loss-retry only (#5862)
        self.assertNotIn("push:", triggers)
        self.assertIn("schedule:", triggers)
        self.assertIn("workflow_dispatch:", triggers)
        self.assertIn("ci/mq_acquire_retry.py", workflow)


if __name__ == "__main__":
    unittest.main()
