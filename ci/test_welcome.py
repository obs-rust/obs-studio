#!/usr/bin/env -S uv run --script
"""The first-contribution welcome, once per person (#2506).

    uv run python -m unittest ci/test_welcome.py

Every rule the workflow depends on is a decision function over the event
payload plus an injected API: who is welcomed, who is skipped, the once-only
marker, and the fail-closed behaviour when the API cannot answer. The
templates and the workflow text are checked here too, so the copy that a
contributor reads is reviewed like code.

RED first: this file was committed before ci/welcome.py, the templates and
the workflow existed, and failed with "no module named ci.welcome" (8 errors)
and the missing-template assertions.
"""

from __future__ import annotations

import contextlib
import io
import re
import unittest
from collections.abc import Collection
from pathlib import Path

import welcome

ROOT = Path(__file__).resolve().parent.parent
WELCOME_DIR = ROOT / ".github" / "welcome"
WORKFLOW = ROOT / ".github" / "workflows" / "welcome.yaml"

DISCORD = "https://discord.gg/6MWWX4x9pf"
DISCUSSIONS = "https://github.com/obs-rust/obs-studio/discussions"


def pr_payload(
    action,
    *,
    association="FIRST_TIME_CONTRIBUTOR",
    author="newcomer",
    number=42,
    merged=False,
    repo="obs-rust/obs-studio",
):
    return {
        "action": action,
        "repository": {"full_name": repo},
        "pull_request": {
            "number": number,
            "merged": merged,
            "user": {"login": author, "type": "User"},
            "author_association": association,
        },
    }


def issue_payload(
    action="opened",
    *,
    association="FIRST_TIMER",
    author="newcomer",
    number=42,
    repo="obs-rust/obs-studio",
):
    return {
        "action": action,
        "repository": {"full_name": repo},
        "issue": {
            "number": number,
            "user": {"login": author, "type": "User"},
            "author_association": association,
        },
    }


class WelcomeDecisionTest(unittest.TestCase):
    """Who gets a welcome, and who does not."""

    def decide(
        self,
        payload: dict,
        *,
        posted: Collection[str] = (),
        merged_count: int | Exception = 1,
    ):
        def already_posted(_number: int, marker: str) -> bool:
            return marker in posted

        def merged_pr_count(_author: str) -> int:
            if isinstance(merged_count, Exception):
                raise merged_count
            return merged_count

        return welcome.decide(
            payload, already_posted=already_posted, merged_pr_count=merged_pr_count
        )

    # --- welcomed ---------------------------------------------------------

    def test_first_time_contributor_opening_a_pr_gets_the_first_pr_welcome(self):
        post = self.decide(pr_payload("opened"))
        self.assertIsNotNone(post)
        self.assertEqual(post.number, 42)
        self.assertEqual(post.marker, "first-pr")
        self.assertEqual(post.template.name, "first-pr.md")

    def test_first_timer_opening_a_pr_gets_the_first_pr_welcome(self):
        # FIRST_TIMER is "never contributed anywhere"; FIRST_TIME_CONTRIBUTOR
        # is "never contributed here". A first PR of either kind is a first
        # contribution to this repository (#2506).
        self.assertEqual(
            self.decide(pr_payload("opened", association="FIRST_TIMER")).marker,
            "first-pr",
        )

    def test_a_first_issue_gets_the_same_welcome(self):
        post = self.decide(issue_payload())
        self.assertEqual(post.marker, "first-pr")

    def test_a_first_merged_pr_gets_the_first_merge_welcome(self):
        post = self.decide(pr_payload("closed", merged=True))
        self.assertIsNotNone(post)
        self.assertEqual(post.marker, "first-merge")
        self.assertEqual(post.template.name, "first-merge.md")

    # --- skipped ----------------------------------------------------------

    def test_a_contributor_whose_first_pr_merges_is_still_welcomed(self):
        # The author of the very PR being merged can already read CONTRIBUTOR,
        # so the merge message cannot key off a first-time association.
        post = self.decide(pr_payload("closed", merged=True, association="CONTRIBUTOR"))
        assert post is not None
        self.assertEqual(post.marker, "first-merge")

    def test_a_member_is_never_welcomed(self):
        for association in ("MEMBER", "OWNER", "COLLABORATOR"):
            self.assertIsNone(
                self.decide(pr_payload("opened", association=association)), association
            )
            self.assertIsNone(
                self.decide(pr_payload("closed", association=association, merged=True)),
                association,
            )

    def test_a_returning_contributor_is_not_welcomed_again(self):
        self.assertIsNone(self.decide(pr_payload("opened", association="CONTRIBUTOR")))

    def test_a_bot_is_never_welcomed(self):
        self.assertIsNone(self.decide(pr_payload("opened", author="dependabot[bot]")))
        self.assertIsNone(
            self.decide(pr_payload("closed", author="github-actions[bot]", merged=True))
        )

    def test_the_marker_keeps_it_to_once_per_person(self):
        self.assertIsNone(self.decide(pr_payload("opened"), posted={"first-pr"}))
        self.assertIsNone(
            self.decide(pr_payload("closed", merged=True), posted={"first-merge"})
        )

    def test_the_first_pr_marker_does_not_suppress_the_first_merge_welcome(self):
        # A contributor welcomed for their first PR is due the second message
        # when that PR merges; the two markers are independent.
        post = self.decide(pr_payload("closed", merged=True), posted={"first-pr"})
        self.assertEqual(post.marker, "first-merge")

    def test_a_second_merged_pr_is_not_welcomed(self):
        self.assertIsNone(
            self.decide(pr_payload("closed", merged=True), merged_count=2)
        )

    def test_a_closed_but_unmerged_pr_is_not_welcomed(self):
        self.assertIsNone(self.decide(pr_payload("closed", merged=False)))

    def test_an_event_with_nothing_to_do_is_ignored(self):
        self.assertIsNone(self.decide(pr_payload("synchronize")))
        self.assertIsNone(self.decide(pr_payload("opened", association="NONE")))
        self.assertIsNone(
            self.decide(
                {"action": "opened", "repository": {"full_name": "obs-rust/obs-studio"}}
            )
        )

    def test_a_fork_of_the_repository_is_left_alone(self):
        self.assertIsNone(self.decide(pr_payload("opened", repo="someone/obs-studio")))

    # --- fail closed ------------------------------------------------------

    def test_an_api_error_posts_nothing(self):
        # decide() logs why it stayed quiet; keep that out of the gate log.
        with contextlib.redirect_stderr(io.StringIO()):
            self.assertIsNone(
                self.decide(
                    pr_payload("closed", merged=True), merged_count=RuntimeError("502")
                )
            )


class TemplateTest(unittest.TestCase):
    """The text a contributor reads."""

    def text(self, name):
        path = WELCOME_DIR / name
        self.assertTrue(path.is_file(), f"missing template {path}")
        return path.read_text(encoding="utf-8")

    def test_both_templates_exist_and_are_not_empty(self):
        for name in ("first-pr.md", "first-merge.md"):
            self.assertGreater(len(self.text(name).strip()), 100, name)

    def test_templates_point_at_discord_and_discussions(self):
        for name in ("first-pr.md", "first-merge.md"):
            text = self.text(name)
            self.assertIn(DISCORD, text, name)
            self.assertIn(DISCUSSIONS, text, name)
            self.assertIn("No question is too small", text, name)

    def test_the_first_pr_template_explains_the_fork_ci_approval(self):
        # The question a first-time PR author actually has (#2483).
        self.assertIn("maintainer", self.text("first-pr.md").lower())

    def test_the_first_merge_template_can_carry_a_line_about_the_fix(self):
        self.assertIn("{{", self.text("first-merge.md"))

    def test_a_template_never_carries_a_marker_itself(self):
        # The script appends the marker; a marker in the prose would let a
        # template edit silently disable the once-only check.
        for name in ("first-pr.md", "first-merge.md"):
            self.assertNotIn("<!-- welcome:", self.text(name), name)


class RenderTest(unittest.TestCase):
    """What actually gets posted."""

    def test_the_author_is_mentioned_first(self):
        self.assertTrue(welcome.render("first-pr", "newcomer").startswith("@newcomer "))

    def test_the_marker_is_appended(self):
        for marker in ("first-pr", "first-merge"):
            self.assertIn(
                f"<!-- welcome:{marker} -->", welcome.render(marker, "newcomer"), marker
            )

    def test_no_placeholder_survives_into_the_comment(self):
        # C's optional "one line on what the fix does" is dropped, not posted
        # raw: a comment with {{...}} in it is worse than one without the line.
        for marker in ("first-pr", "first-merge"):
            self.assertNotIn("{{", welcome.render(marker, "newcomer"), marker)


class WorkflowTest(unittest.TestCase):
    """The workflow that runs the decision."""

    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")

    def test_it_runs_on_issues_and_pull_request_target(self):
        self.assertRegex(
            self.text, r"(?m)^on:\n(?:.*\n)*?  issues:\n(?:.*\n)*?    types: \[opened\]"
        )
        self.assertRegex(
            self.text, r"pull_request_target:\n(?:.*\n)*?    types: \[opened, closed\]"
        )

    def test_it_asks_only_for_the_permissions_it_needs(self):
        self.assertIn("issues: write", self.text)
        self.assertIn("pull-requests: write", self.text)
        self.assertNotIn("contents: write", self.text)

    def test_it_never_checks_out_the_fork_head(self):
        # pull_request_target runs with a write token; checking out the pull
        # request's head would run the contributor's code against it.
        self.assertNotIn("head.sha", self.text)
        self.assertNotIn("head_ref", self.text)

    def test_it_runs_the_script_on_the_base_tree(self):
        # --no-project: a workflow's `uv run` never depends on project
        # discovery (ci/test_uv_run.py, TOOL-003).
        self.assertIn("uv run --no-project ci/welcome.py", self.text)
        self.assertIn("GITHUB_EVENT_PATH", self.text)

    def test_actions_are_pinned_to_a_commit(self):
        for use in re.findall(r"uses: (\S+)", self.text):
            self.assertRegex(use, r"@[0-9a-f]{40}$", use)

    def test_it_does_not_run_in_forks(self):
        self.assertIn("github.repository == 'obs-rust/obs-studio'", self.text)


if __name__ == "__main__":
    unittest.main()
