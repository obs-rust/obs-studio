#!/usr/bin/env -S uv run --script
"""Scheduled workflows do not run in forks (#1258).

    uv run python -m unittest ci/test_workflow_schedule_guard.py

GitHub runs a workflow's `schedule:` in every fork whose default branch has
it. In a contributor's fork the release, sanitizer, leaderboard, claim-bot
and nightly runs failed every day (no tags, no push rights) and sent the
fork owner a failure email for each. Every root job (no `needs:`) of a
scheduled workflow now skips a scheduled run outside obs-rust/obs-studio;
the jobs that need it are skipped with it. `workflow_dispatch` and the other
triggers still run in a fork.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"

GUARD = "github.event_name != 'schedule' || github.repository == 'obs-rust/obs-studio'"
# Inherited from upstream OBS: these jobs never run outside obsproject, so
# they do not run in a fork either.
UPSTREAM_ONLY = "github.repository_owner == 'obsproject'"


def is_scheduled(text: str) -> bool:
    on = re.search(r"^on:\n(.*?)(?=^\S)", text, re.S | re.M)
    return bool(on and re.search(r"^  schedule:", on.group(1), re.M))


def jobs(text: str) -> dict[str, str]:
    """Each job's name and body (the lines below `  <name>:`)."""
    body = text.split("\njobs:\n", 1)[1]
    parts = re.split(r"^  ([A-Za-z0-9_-]+):\n", body, flags=re.M)
    return dict(zip(parts[1::2], parts[2::2]))


def condition(job: str) -> str:
    """The job's `if:`, with a folded block (`>-`) joined into one line."""
    m = re.search(r"^    if: *(.*)\n((?:      .*\n)*)", job, re.M)
    if not m:
        return ""
    lines = [m.group(1)] if m.group(1) not in (">-", ">", "|") else []
    lines += [line.strip() for line in m.group(2).splitlines()]
    return " ".join(lines)


def unguarded_roots(text: str) -> list[str]:
    roots = {name: job for name, job in jobs(text).items() if not re.search(r"^    needs:", job, re.M)}
    return [
        name
        for name, job in roots.items()
        if GUARD not in condition(job) and UPSTREAM_ONLY not in condition(job)
    ]


SAMPLE = """\
on:
  schedule:
    - cron: '0 9 * * *'
  workflow_dispatch:

jobs:
  first:
    if: >-
      (github.event_name != 'schedule' || github.repository == 'obs-rust/obs-studio') &&
      github.event_name != 'issue_comment'
    runs-on: ubuntu-latest
  second:
    needs: first
    runs-on: ubuntu-latest
  third:
    runs-on: ubuntu-latest
"""


class ParserTest(unittest.TestCase):
    def test_finds_the_schedule_trigger(self):
        self.assertTrue(is_scheduled(SAMPLE))
        self.assertFalse(is_scheduled(SAMPLE.replace("  schedule:\n    - cron: '0 9 * * *'\n", "")))

    def test_joins_a_folded_condition(self):
        self.assertIn(GUARD, condition(jobs(SAMPLE)["first"]))

    def test_reports_only_unguarded_root_jobs(self):
        self.assertEqual(unguarded_roots(SAMPLE), ["third"])


class WorkflowTest(unittest.TestCase):
    def setUp(self):
        self.scheduled = {
            path.name: path.read_text(encoding="utf-8")
            for path in sorted(WORKFLOWS.glob("*.y*ml"))
            if is_scheduled(path.read_text(encoding="utf-8"))
        }

    def test_finds_the_scheduled_workflows(self):
        expected = {"claim-bot.yaml", "leaderboard.yml", "release-rust.yaml", "sanitizers.yaml", "scheduled.yaml"}
        self.assertLessEqual(expected, set(self.scheduled))

    def test_every_root_job_skips_a_scheduled_run_in_a_fork(self):
        missing = {name: roots for name, text in self.scheduled.items() if (roots := unguarded_roots(text))}
        self.assertEqual(missing, {}, f"add `if: {GUARD}` to these root jobs")


if __name__ == "__main__":
    unittest.main()
