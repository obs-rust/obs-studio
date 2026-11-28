#!/usr/bin/env -S uv run --script
"""nextest enforces the per-test time budget (#2888).

    uv run python -m unittest ci/test_nextest_budget.py

Decided on #2888: a test is SLOW at 10 s and fails at 30 s
(`slow-timeout = { period = "10s", terminate-after = 3 }`); a test that needs
more gets a named override whose comment cites the issue.
"""

from __future__ import annotations

import re
import tomllib
import unittest
from pathlib import Path

CONFIG = Path(__file__).resolve().parent / "nextest.toml"


class NextestBudgetTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = CONFIG.read_text(encoding="utf-8")
        self.cfg = tomllib.loads(self.text)

    def test_default_profile_marks_slow_at_10s_and_fails_at_30s(self):
        slow = self.cfg["profile"]["default"].get("slow-timeout")
        self.assertEqual(slow, {"period": "10s", "terminate-after": 3})

    def test_every_override_is_named_and_cites_its_issue(self):
        overrides = self.cfg["profile"]["default"].get("overrides", [])
        self.assertTrue(overrides, "expected the named exceptions of #2888")
        for o in overrides:
            self.assertIn("test(", o["filter"], o)
            self.assertNotIn("all()", o["filter"], o)
            if "slow-timeout" not in o:
                # Scheduling-only rules cannot alter selection, concurrency,
                # retries or timeouts. Timeout exceptions keep the #2888 check.
                self.assertEqual(set(o), {"filter", "priority"}, o)
                self.assertIs(type(o["priority"]), int, o)
                self.assertTrue(-100 <= o["priority"] <= 100, o)
        for before, override in zip(self.text.split("[[profile.default.overrides]]"), overrides):
            comment = before.rstrip().splitlines()[-1]
            issue = "#2888" in comment if "slow-timeout" in override else re.search(r"#\d+", comment)
            self.assertTrue(comment.startswith("#") and issue, comment)


if __name__ == "__main__":
    unittest.main()
