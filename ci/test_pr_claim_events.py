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


if __name__ == "__main__":
    unittest.main()
