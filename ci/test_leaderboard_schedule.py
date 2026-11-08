#!/usr/bin/env -S uv run --script
"""The contributor leaderboard refreshes every 4 hours.

    uv run python -m unittest ci/test_leaderboard_schedule.py
"""

from __future__ import annotations

import importlib.util
import re
import unittest
from pathlib import Path
from threading import Barrier, Lock
from unittest.mock import patch

WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "leaderboard.yml"
GENERATE = Path(__file__).resolve().parent.parent / "tools" / "leaderboard" / "generate.py"


def load_generate():
    spec = importlib.util.spec_from_file_location("leaderboard_generate", GENERATE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class CompletenessCheckTest(unittest.TestCase):
    """Run 38335083387 failed with "INCOMPLETE: fetched 219 merged PRs, GitHub
    reports 218": a PR merged while the generator was fetching, so the list
    was newer than the count. Only a list shorter than the count is a real
    gap; the job must not fail (and the badge go red) for a fresher list."""

    def setUp(self):
        self.gen = load_generate()

    def test_fresher_list_than_count_is_complete(self):
        self.assertTrue(self.gen.merged_list_complete(219, 218))

    def test_equal_is_complete(self):
        self.assertTrue(self.gen.merged_list_complete(218, 218))

    def test_short_list_is_incomplete(self):
        self.assertFalse(self.gen.merged_list_complete(217, 218))

    def test_count_is_read_before_the_list(self):
        src = GENERATE.read_text()
        self.assertLess(src.index('"search/issues"'), src.index('"pr", "list"'))


class CommitFetchTest(unittest.TestCase):
    def setUp(self):
        self.gen = load_generate()

    def test_fetches_overlap_with_four_workers_and_keep_input_order(self):
        barrier = Barrier(4)
        lock = Lock()
        active = peak = 0

        def fetch(repo, number):
            nonlocal active, peak
            self.assertEqual(repo, "owner/repo")
            with lock:
                active += 1
                peak = max(peak, active)
            barrier.wait(timeout=5)
            with lock:
                active -= 1
            return [number]

        numbers = [8, 3, 7, 1, 6, 2, 5, 4]
        with patch.object(self.gen, "pr_commits", side_effect=fetch):
            result = list(self.gen.fetch_commits("owner/repo", numbers))
        self.assertEqual(result, [[n] for n in numbers])
        self.assertEqual(peak, 4)

    def test_failure_is_not_silently_dropped(self):
        with patch.object(self.gen, "pr_commits", side_effect=SystemExit("API failed")):
            with self.assertRaisesRegex(SystemExit, "API failed"):
                list(self.gen.fetch_commits("owner/repo", [1]))

    def test_commit_fetch_keeps_every_page(self):
        import json

        def commit(message):
            return {"commit": {"message": message, "author": {"email": "a@b"},
                               "committer": {}}, "author": {"login": "alice"}}

        with patch.object(self.gen, "gh", return_value=json.dumps([
            [commit("first\nbody")], [commit("second")],
        ])) as gh:
            result = self.gen.pr_commits("owner/repo", 1)
        gh.assert_called_once_with("api", "--paginate", "--slurp",
                                   "repos/owner/repo/pulls/1/commits")
        self.assertEqual([c["messageHeadline"] for c in result], ["first", "second"])
        self.assertEqual(result[0]["messageBody"], "body")


class ConversionBadgeTest(unittest.TestCase):
    def test_measure_emits_badge_and_check_detects_stale_badge(self):
        import json
        import tempfile

        path = GENERATE.parents[1] / "conversion" / "measure.py"
        spec = importlib.util.spec_from_file_location("conversion_measure_badge", path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        data = {"date": "2026-11-08", "libobs": {"all": {"percent": 55.04}}}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "conversion.json"
            badge = output.with_name("conversion-badge.json")
            with patch.object(mod, "measure", return_value=data), patch.object(mod, "render_svg", return_value=""):
                self.assertEqual(mod.main(["--json", str(output)]), 0)
                self.assertTrue(badge.is_file(), "measurement must emit its badge")
                self.assertEqual(json.loads(badge.read_text()), {
                    "schemaVersion": 1, "label": "converted to Rust",
                    "message": "55.0% · 2026-11-08", "color": "e8631a",
                })
                self.assertEqual(mod.main(["--json", str(output), "--check"]), 0)
                badge.write_text("{}")
                self.assertEqual(mod.main(["--json", str(output), "--check"]), 1)

    def test_badges_are_next_to_conversion_graph(self):
        readme = (WORKFLOW.parent.parent.parent / "README.md").read_text()
        section = readme.split("# Conversion progress", 1)[1].split("# Features", 1)[0]
        self.assertIn("img.shields.io/endpoint?url=", section)
        self.assertIn("conversion-badge.json", section)
        self.assertIn("actions/workflows/leaderboard.yml/badge.svg?branch=master", section)


class LeaderboardScheduleTest(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text()

    def test_runs_every_four_hours(self):
        crons = re.findall(r'cron:\s*"([^"]+)"', self.text)
        self.assertEqual(len(crons), 1, crons)
        minute, hour, *rest = crons[0].split()
        self.assertEqual(hour, "*/4", crons[0])
        self.assertEqual(rest, ["*", "*", "*"], crons[0])

    def test_no_time_of_day_gate(self):
        # A scheduled run must always regenerate; nothing may skip it by hour.
        self.assertNotIn("America/Los_Angeles", self.text)
        self.assertNotIn("steps.gate", self.text)

    def test_readme_badge_points_at_workflow(self):
        readme = (WORKFLOW.parent.parent.parent / "README.md").read_text()
        self.assertIn("actions/workflows/leaderboard.yml/badge.svg?branch=master", readme)

    def test_still_dispatchable(self):
        self.assertIn("workflow_dispatch:", self.text)


if __name__ == "__main__":
    unittest.main()
