#!/usr/bin/env -S uv run --script
"""The reusable PR claim check must receive this repository's PR events (#26)."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class ClaimPullRequestEventsTest(unittest.TestCase):
    def test_claim_bot_subscribes_to_all_claim_recheck_events(self):
        text = (ROOT / ".github/workflows/claim-bot.yaml").read_text()
        match = re.search(r"^  pull_request:\n    types: \[([^]]+)\]", text, re.M)
        self.assertIsNotNone(match, "PR claim checks must be routed to the reusable claim bot")
        self.assertEqual(
            {x.strip() for x in match.group(1).split(",")},
            {"opened", "edited", "synchronize", "ready_for_review", "reopened"},
        )

    def test_pr_runs_are_serialized_with_comments_on_the_same_number(self):
        text = (ROOT / ".github/workflows/claim-bot.yaml").read_text()
        self.assertIn("github.event.issue.number || github.event.pull_request.number", text)
        self.assertIn("cancel-in-progress: false", text)


# claims.handles() from obs-rust/.github workflow/claims.py: a comment is acted
# on when its stripped first line matches a command, or its body asks for a
# decision, or the issue is claim-blocked. Keep in sync with that file.
_FIRST_LINE = [
    r"^\s*claiming\b",
    r"^\s*/ready\b",
    r"^\s*/investigate\b",
    r"^\s*releasing\b",
    r"^\s*taking over stale claim from @([A-Za-z0-9-]+)",
    r"^\s*/reassign\s+@([A-Za-z0-9-]+)",
    r"^\s*(?:/?handoff\b|hand(?:ed|ing)\b|pass(?:ed|ing)\b|giv(?:e|ing)\b"
    r"|transfer(?:red|ring)\b|mov(?:e|ing)\b|it'?s\s+yours\b|\byours\b)",
    r"^\s*(?:blocked\s*:|/blocked\b)",
    r"^\s*[*_]{0,3}\s*(?:decided|decision)\s*[*_]{0,3}\s*:",
]
_DECISION = r"^\s*(?:#{1,6}\s+)?[*_]{0,3}\s*decision\s+needed\s*[*_]{0,3}\s*:"


def handles(body: str, labels: set[str]) -> bool:
    line = body.strip().splitlines()[0] if body.strip() else ""
    return (
        any(re.match(p, line, re.I) for p in _FIRST_LINE)
        or bool(re.search(_DECISION, body, re.I | re.M))
        or "claim-blocked" in labels
    )


def admitted(body: str, labels: set[str]) -> bool:
    """The claim job's `if:` for an issue_comment, as GitHub evaluates it."""
    text = (ROOT / ".github/workflows/claim-bot.yaml").read_text()
    condition = text.split("    if: >-\n", 1)[1].split("\n    # The claim bot", 1)[0]
    contains = re.findall(r"contains\(github\.event\.comment\.body, '([^']+)'\)", condition)
    starts = re.findall(r"startsWith\(github\.event\.comment\.body, '([^']+)'\)", condition)
    starts += [{"t": "\t", "r": "\r", "n": "\n"}[c] for c in re.findall(r"fromJSON\('\"\\(\w)\"'\)", condition)]
    low = body.lower()
    return (
        ("claim-blocked" in labels and "labels.*.name, 'claim-blocked'" in condition)
        or any(w.lower() in low for w in contains)
        or any(low.startswith(w.lower()) for w in starts)
    )


class ClaimCommentPrefilterTest(unittest.TestCase):
    """The job-level `if:` must never drop a comment claim_bot.py would act on."""

    HANDLED = [
        "claiming",
        "Claiming: investigation",
        "  claiming this",
        "\nclaiming",
        "/ready",
        "/investigate why",
        "Releasing, out of time",
        "Taking over stale claim from @someone",
        "/reassign @other",
        "/handoff to @other",
        "Handing this to @other",
        "Passing to @other",
        "giving it to @x",
        "Transferring to @x",
        "moving to @x",
        "It's yours @x",
        "yours",
        "Blocked: waiting on #12",
        "/blocked",
        "**Decision**: ship it",
        "decided: no",
        "Progress\n\n## Decision needed: which runner?",
    ]
    IGNORED = [
        "PROGRESS #1945: lint 8/8 passed, Python lane running.",
        "GREEN: 600/600 runs passed under contention.",
        "PUSHED: https://github.com/obs-rust/obs-studio/pull/1",
        "Thanks, merged.",
        "CI is green; waiting for review.",
    ]

    def test_every_handled_comment_is_admitted(self):
        for body in self.HANDLED:
            with self.subTest(body=body):
                self.assertTrue(handles(body, set()), "fixture is not a command: fix the fixture")
                self.assertTrue(admitted(body, set()))
        self.assertTrue(admitted("PROGRESS: still going", {"claim-blocked"}))

    def test_progress_comments_do_not_start_a_runner(self):
        for body in self.IGNORED:
            with self.subTest(body=body):
                self.assertFalse(handles(body, set()))
                self.assertFalse(admitted(body, set()))


if __name__ == "__main__":
    unittest.main()
