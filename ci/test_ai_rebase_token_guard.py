#!/usr/bin/env -S uv run --script
"""The optional ai-rebase workflow must skip when its token is unset (#4461).

    uv run python -m unittest ci/test_ai_rebase_token_guard.py

`actions/checkout` fails with `Input required and not supplied: token` when
`secrets.AI_REBASE_TOKEN` is empty, so a master push armed the ai-rebase job,
paid for setup, and then died before its rebase logic (run 42724921146 of the
#4450 master push). The workflow is optional: with no token configured it must
skip its root job instead. Every root job whose body reads
`secrets.AI_REBASE_TOKEN` now carries `secrets.AI_REBASE_TOKEN != ''` in its
`if:`, so an unconfigured repository skips the whole job.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "ai-rebase.yaml"

TOKEN_REF = "secrets.AI_REBASE_TOKEN"
GUARD = "secrets.AI_REBASE_TOKEN != ''"


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


def unguarded_token_jobs(text: str) -> list[str]:
    return [
        name
        for name, job in jobs(text).items()
        if TOKEN_REF in job and GUARD not in condition(job)
    ]


WITH_GUARD = """\
on:
  push:
    branches: [master]
jobs:
  rebase:
    if: >-
      (github.event_name != 'schedule' || github.repository == 'obs-rust/obs-studio') &&
      secrets.AI_REBASE_TOKEN != ''
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout
        with:
          token: ${{ secrets.AI_REBASE_TOKEN }}
"""

WITHOUT_GUARD = WITH_GUARD.replace("      secrets.AI_REBASE_TOKEN != ''\n", "")


class ParserTest(unittest.TestCase):
    def test_accepts_a_folded_guarded_condition(self):
        self.assertEqual(unguarded_token_jobs(WITH_GUARD), [])

    def test_reports_the_unguarded_token_job(self):
        self.assertEqual(unguarded_token_jobs(WITHOUT_GUARD), ["rebase"])

    def test_ignores_jobs_without_the_token(self):
        self.assertEqual(unguarded_token_jobs("on:\nx\njobs:\n  other:\n    runs-on: x\n"), [])


class WorkflowTest(unittest.TestCase):
    def test_ai_rebase_skips_when_the_token_is_unset(self):
        self.assertEqual(unguarded_token_jobs(WORKFLOW.read_text(encoding="utf-8")), [])


if __name__ == "__main__":
    unittest.main()
