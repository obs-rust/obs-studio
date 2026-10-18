#!/usr/bin/env -S uv run --script
"""The claim bot labels claimants and expires stale claims (#909), quietly.

    uv run python -m unittest ci/test_claim_bot.py

Only a rejection notifies anyone: it replies to and mentions the person
refused. Accepted commands get the `claimed` label and a reaction on the
command comment; expiry just removes the label. Bot comments, assignments
and @-mentions each sent the maintainer a phone notification for every
claim their agents made. GitHub I/O is not run.
"""

from __future__ import annotations

import importlib.util
import re
import sys
import unittest
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "tools" / "claim-bot" / "claim_bot.py"
WORKFLOW = ROOT / ".github" / "workflows" / "claim-bot.yaml"


def load():
    spec = importlib.util.spec_from_file_location("claim_bot", SCRIPT)
    mod = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = mod  # dataclasses look the module up by name
    spec.loader.exec_module(mod)
    return mod


cb = load()
T0 = datetime(2026, 10, 15, 12, 0, tzinfo=timezone.utc)
_ids = iter(range(1000, 10**6))


def h(n: float) -> datetime:
    return T0 + timedelta(hours=n)


def human(author, body, at, assoc="CONTRIBUTOR", granted=False):
    return cb.Comment(author, assoc, body, at, id=next(_ids), granted=granted)


def bot(body, at):
    return cb.Comment(cb.BOT, "NONE", f"{cb.BOT_PREFIX} {body}", at)


def issue(comments=(), labels=(), prs=()):
    return cb.Issue(1, set(labels), [], list(comments), list(prs))


def kinds(actions):
    return [a[0] for a in actions]


def applied(i, c, acts):
    """The issue after the bot's actions, as GitHub would record them."""
    for kind, arg in acts:
        if kind == "label_add":
            i.labels.add(arg)
        elif kind == "label_remove":
            i.labels.discard(arg)
        elif kind == "react":
            assert arg == c.id
            c.granted = True
    i.comments.append(c)
    return i


def claimed_by_alice(labels=(cb.CLAIMED,), prs=()):
    return issue([human("alice", "Claiming: x", h(0), granted=True)], labels, prs)


def accepted(acts, c):
    return acts == [("label_add", cb.CLAIMED), ("react", c.id)]


def rejected(acts, who):
    return kinds(acts) == ["comment"] and f"@{who}," in acts[0][1]


class ClaimTest(unittest.TestCase):
    def test_outside_contributor_claim_is_labeled_and_reacted(self):
        c = human("vpacher", "Claiming: audio-io\n1. plan", h(0))
        i = issue()
        acts = cb.decide_comment(i, c, h(0))
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "vpacher")

    def test_one_account_may_hold_any_number_of_claims(self):
        """Several agents on one maintainer's machine share one account."""
        c = human("zackees", "Claiming: x", h(0), "MEMBER")
        self.assertTrue(accepted(cb.decide_comment(issue(), c, h(0)), c))

    def test_claim_is_case_insensitive_and_first_line_only(self):
        self.assertTrue(cb.decide_comment(issue(), human("a", "claiming: x", h(0)), h(0)))
        self.assertEqual(cb.decide_comment(issue(), human("a", "plan\nClaiming: x", h(0)), h(0)), [])

    def test_fresh_claim_by_someone_else_is_rejected_with_a_mention(self):
        acts = cb.decide_comment(claimed_by_alice(), human("bob", "Claiming: x", h(1)), h(1))
        self.assertTrue(rejected(acts, "bob"), acts)
        self.assertIn("already claimed by `alice`", acts[0][1])

    def test_rejected_claim_comment_does_not_become_the_claim(self):
        i = claimed_by_alice()
        c = human("bob", "Claiming: x", h(1))
        i = applied(i, c, cb.decide_comment(i, c, h(1)))
        self.assertEqual(cb.current_claim(i).user, "alice")

    def test_reclaim_by_holder_is_a_no_op(self):
        self.assertEqual(cb.decide_comment(claimed_by_alice(), human("alice", "Claiming: x", h(1)), h(1)), [])

    def test_claim_on_a_stale_claim_moves_it(self):
        i = claimed_by_alice()
        c = human("bob", "Claiming: x", h(49))
        acts = cb.decide_comment(i, c, h(49))
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")

    def test_locked_issue_is_ignored(self):
        self.assertEqual(cb.decide_comment(issue(labels=[cb.LOCKED]), human("a", "Claiming: x", h(0)), h(0)), [])

    def test_bot_comments_are_ignored(self):
        self.assertEqual(cb.decide_comment(issue(), cb.Comment("x[bot]", "NONE", "Claiming: x", h(0)), h(0)), [])


class StateTest(unittest.TestCase):
    def test_no_label_means_no_claim(self):
        self.assertIsNone(cb.current_claim(claimed_by_alice(labels=())))

    def test_legacy_at_records_still_read(self):
        i = issue([human("alice", "Claiming: x", h(0)), bot("claimed by @alice. ...", h(0))], [cb.CLAIMED])
        self.assertEqual(cb.current_claim(i).user, "alice")

    def test_pre_bot_claim_is_read_from_the_comment(self):
        i = issue([human("nathan-v", "Claiming: x", h(0))], [cb.CLAIMED])
        self.assertEqual(cb.current_claim(i).user, "nathan-v")


class ReleaseTest(unittest.TestCase):
    def test_holder_releases_silently(self):
        i = claimed_by_alice()
        c = human("alice", "Releasing x, branch foo", h(2))
        acts = cb.decide_comment(i, c, h(2))
        self.assertEqual(acts, [("label_remove", cb.CLAIMED), ("react", c.id)])
        self.assertIsNone(cb.current_claim(applied(i, c, acts)))

    def test_others_cannot_release(self):
        self.assertEqual(cb.decide_comment(claimed_by_alice(), human("bob", "Releasing x", h(2)), h(2)), [])

    def test_claim_after_release_is_granted(self):
        i = claimed_by_alice()
        r = human("alice", "Releasing x", h(2))
        i = applied(i, r, cb.decide_comment(i, r, h(2)))
        c = human("bob", "Claiming: x", h(3))
        acts = cb.decide_comment(i, c, h(3))
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")


class HandoffTest(unittest.TestCase):
    """Handoff language in the claimant's own comment moves the claim in one
    step instead of needing a separate `/reassign`."""

    def hands(self, body):
        i = claimed_by_alice()
        c = human("alice", body, h(2))
        acts = cb.decide_comment(i, c, h(2))
        return i, c, acts

    def test_release_naming_a_handoff_target_moves_the_claim(self):
        """The real wording: `Releasing #1301 — handing it to @vpacher, ...`."""
        i, c, acts = self.hands("Releasing x — handing it to @bob, who has the bench driver")
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")

    def test_bare_handoff_line_moves_the_claim(self):
        i, c, acts = self.hands("Handing x over to @bob")
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")

    def test_handoff_command_moves_the_claim(self):
        i, c, acts = self.hands("/handoff @bob")
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")

    def test_informal_yours_moves_the_claim(self):
        i, c, acts = self.hands("It's yours, @bob 🫡")
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")

    def test_gratitude_is_not_a_handoff(self):
        """`thanks to @bob` in a release keeps the release a release."""
        i, c, acts = self.hands("Releasing x — thanks to @bob for the review")
        self.assertEqual(acts, [("label_remove", cb.CLAIMED), ("react", c.id)], acts)
        self.assertIsNone(cb.current_claim(applied(i, c, acts)))

    def test_handoff_to_self_still_releases(self):
        i, c, acts = self.hands("Releasing x — handing it to @alice")
        self.assertEqual(acts, [("label_remove", cb.CLAIMED), ("react", c.id)], acts)
        self.assertIsNone(cb.current_claim(applied(i, c, acts)))

    def test_handoff_without_a_target_still_releases(self):
        i, c, acts = self.hands("Releasing x, handing it over to nobody in particular")
        self.assertEqual(acts, [("label_remove", cb.CLAIMED), ("react", c.id)], acts)

    def test_only_the_holder_can_hand_off(self):
        i = claimed_by_alice()
        c = human("mallory", "Handing x over to @mallory", h(2))
        self.assertEqual(cb.decide_comment(i, c, h(2)), [])

    def test_handoff_starts_the_new_holders_clock(self):
        i, c, acts = self.hands("Handing x over to @bob")
        i = applied(i, c, acts)
        claim = cb.current_claim(i)
        self.assertFalse(cb.is_stale(i, claim, h(49.9)))
        self.assertTrue(cb.is_stale(i, claim, h(50)))

    def test_handoff_blocks_another_claimant(self):
        i, c, acts = self.hands("Handing x over to @bob")
        i = applied(i, c, acts)
        other = human("carol", "Claiming: x", h(3))
        acts = cb.decide_comment(i, other, h(3))
        self.assertTrue(rejected(acts, "carol"), acts)
        self.assertIn("already claimed by `bob`", acts[0][1])

    def test_handoff_lines_are_recognized_commands(self):
        """The event gate must route handoff comments to the bot at all."""
        for line in ("Handing x over to @bob", "/handoff @bob", "It's yours, @bob",
                     "Releasing x — handing it to @bob"):
            self.assertTrue(any(r.match(line) for r in cb.COMMANDS), line)


class TakeoverTest(unittest.TestCase):
    def test_takeover_of_fresh_claim_is_rejected(self):
        acts = cb.decide_comment(claimed_by_alice(),
                                 human("bob", "Taking over stale claim from @alice", h(10)), h(10))
        self.assertTrue(rejected(acts, "bob"), acts)
        self.assertIn("still active", acts[0][1])

    def test_takeover_of_stale_claim_moves_it_silently(self):
        i = claimed_by_alice()
        c = human("bob", "Taking over stale claim from @alice", h(48))
        acts = cb.decide_comment(i, c, h(48))
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")

    def test_takeover_naming_the_wrong_holder_is_rejected(self):
        acts = cb.decide_comment(claimed_by_alice(),
                                 human("bob", "Taking over stale claim from @carol", h(48)), h(48))
        self.assertTrue(rejected(acts, "bob"), acts)

    def test_critical_claim_with_idle_draft_pr_is_stealable_after_8h(self):
        pr = cb.LinkedPR(5, "alice", [h(1)])
        i = claimed_by_alice(labels=(cb.CLAIMED, cb.CRITICAL), prs=[pr])
        take = human("bob", "Taking over stale claim from @alice", h(8.5))
        self.assertTrue(accepted(cb.decide_comment(i, take, h(9)), take))
        self.assertTrue(rejected(cb.decide_comment(i, take, h(8.5)), "bob"))

    def test_pushes_to_the_pr_keep_a_critical_claim(self):
        pr = cb.LinkedPR(5, "alice", [h(1), h(7)])
        i = claimed_by_alice(labels=(cb.CLAIMED, cb.CRITICAL), prs=[pr])
        take = human("bob", "Taking over stale claim from @alice", h(10))
        self.assertTrue(rejected(cb.decide_comment(i, take, h(10)), "bob"))

    def test_someone_elses_pr_does_not_count_as_progress(self):
        i = claimed_by_alice(prs=[cb.LinkedPR(5, "mallory", [h(40)])])
        self.assertTrue(cb.is_stale(i, cb.current_claim(i), h(48)))


class ReassignTest(unittest.TestCase):
    def test_maintainer_reassigns_a_fresh_claim_silently(self):
        i = claimed_by_alice()
        c = human("zackees", "/reassign @bob", h(1), "MEMBER")
        acts = cb.decide_comment(i, c, h(1))
        self.assertTrue(accepted(acts, c), acts)
        self.assertEqual(cb.current_claim(applied(i, c, acts)).user, "bob")

    def test_contributor_cannot_reassign(self):
        acts = cb.decide_comment(claimed_by_alice(), human("bob", "/reassign @bob", h(1)), h(1))
        self.assertTrue(rejected(acts, "bob"), acts)


class SweepTest(unittest.TestCase):
    def test_fresh_claim_is_left_alone(self):
        self.assertEqual(cb.decide_sweep(claimed_by_alice(), h(47.9)), [])

    def test_expires_after_48h_silently(self):
        self.assertEqual(cb.decide_sweep(claimed_by_alice(), h(48)), [("label_remove", cb.CLAIMED)])

    def test_claimant_comment_resets_the_clock(self):
        i = claimed_by_alice()
        i.comments.append(human("alice", "progress: RED test pushed", h(40)))
        self.assertEqual(cb.decide_sweep(i, h(60)), [])

    def test_critical_expires_after_8h(self):
        self.assertEqual(cb.decide_sweep(claimed_by_alice((cb.CLAIMED, cb.CRITICAL)), h(8)),
                         [("label_remove", cb.CLAIMED)])

    def test_locked_is_never_expired(self):
        self.assertEqual(cb.decide_sweep(claimed_by_alice((cb.CLAIMED, cb.LOCKED)), h(100)), [])

    def test_pre_bot_claim_expires(self):
        i = issue([human("nathan-v", "Claiming: x", h(0))], [cb.CLAIMED])
        self.assertEqual(cb.decide_sweep(i, h(48)), [("label_remove", cb.CLAIMED)])


class QuietTest(unittest.TestCase):
    """Only rejections comment; nothing assigns."""

    def scenarios(self):
        alice = claimed_by_alice
        yield False, cb.decide_comment(issue(), human("bob", "Claiming: x", h(0)), h(0))
        yield True, cb.decide_comment(alice(), human("bob", "Claiming: x", h(1)), h(1))
        yield False, cb.decide_comment(alice(), human("bob", "Claiming: x", h(49)), h(49))
        yield False, cb.decide_comment(alice(), human("alice", "Releasing x", h(2)), h(2))
        yield False, cb.decide_comment(alice(), human("alice", "Releasing x — handing it to @bob", h(2)), h(2))
        yield True, cb.decide_comment(alice(), human("bob", "Taking over stale claim from @alice", h(10)), h(10))
        yield False, cb.decide_comment(alice(), human("bob", "Taking over stale claim from @alice", h(48)), h(48))
        yield True, cb.decide_comment(alice(), human("bob", "/reassign @bob", h(1)), h(1))
        yield False, cb.decide_comment(alice(), human("zackees", "/reassign @bob", h(1), "MEMBER"), h(1))
        yield False, cb.decide_sweep(alice(), h(46.5))
        yield False, cb.decide_sweep(alice(), h(48))

    def test_only_rejections_comment(self):
        for is_rejection, acts in self.scenarios():
            self.assertEqual("comment" in kinds(acts), is_rejection, acts)

    def test_a_rejection_mentions_only_the_person_refused(self):
        for is_rejection, acts in self.scenarios():
            for kind, arg in acts:
                if kind == "comment":
                    prose = re.sub(r"`[^`]*`", "", arg)  # code spans never mention
                    self.assertEqual(len(re.findall(r"(?<!\w)@[A-Za-z0-9-]", prose)), 1, arg)

    def test_no_assignment(self):
        for _, acts in self.scenarios():
            self.assertFalse({"assign", "unassign"} & set(kinds(acts)), acts)


class WorkflowTest(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")

    def test_triggers_on_comments_and_hourly(self):
        self.assertIn("issue_comment:", self.text)
        self.assertRegex(self.text, r'cron: "\d+ \* \* \* \*"')

    def test_token_can_write_issues_only(self):
        self.assertRegex(self.text, r"issues: write")
        self.assertNotIn("contents: write", self.text)
        self.assertNotIn("pull_request_target", self.text)

    def test_comment_body_never_reaches_a_shell(self):
        self.assertNotRegex(self.text, r"\$\{\{\s*github\.event\.comment")


if __name__ == "__main__":
    unittest.main()
