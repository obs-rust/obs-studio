#!/usr/bin/env -S uv run --script
"""PR CI must also run in GitHub's merge queue (obs-rust/obs-studio#692).

    uv run python -m unittest ci/test_merge_queue.py

The master ruleset queues merges and requires the `CI Gate ✅` check, so the
required checks run on the merge result instead of a stale PR head. That only
works if:
- pr-pull.yaml runs on `merge_group` and on every PR (no paths-ignore, or a
  docs-only PR would never get the required check);
- one always-running job (`CI Gate ✅`) fails when any other job failed or was
  cancelled, so the ruleset needs a single, stable check name;
- build-project.yaml scopes a merge group against its base commit and never
  lets a local-gate stamp skip tests there: the queue commit is not the
  stamped head.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PULL = (ROOT / ".github" / "workflows" / "pr-pull.yaml").read_text()
BUILD = (ROOT / ".github" / "workflows" / "build-project.yaml").read_text()


def on_block(text: str) -> str:
    m = re.search(r"^on:\n((?:[ \t].*\n|\n)*)", text, re.M)
    return m.group(1) if m else ""


def job_block(text: str, job: str) -> str:
    m = re.search(rf"^  {re.escape(job)}:\n((?:    .*\n|\s*\n)*)", text, re.M)
    return m.group(1) if m else ""


def top_level_jobs(text: str) -> list[str]:
    body = text.split("\njobs:\n", 1)[1]
    return re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", body, re.M)


class MergeQueue(unittest.TestCase):
    def test_pull_runs_in_merge_queue(self):
        self.assertRegex(on_block(PULL), r"(?m)^  merge_group:")

    def test_pull_runs_on_every_pr(self):
        self.assertNotIn("paths-ignore", on_block(PULL))

    def test_gate_job_needs_every_other_job(self):
        gate = job_block(PULL, "ci-gate")
        self.assertIn("name: CI Gate ✅", gate)
        self.assertIn("if: always()", gate)
        needs = re.search(r"needs: \[([^\]]*)\]", gate)
        self.assertIsNotNone(needs)
        listed = {n.strip() for n in needs.group(1).split(",")}
        self.assertEqual(listed, set(top_level_jobs(PULL)) - {"ci-gate"})
        self.assertIn("contains(needs.*.result, 'failure')", gate)
        self.assertIn("contains(needs.*.result, 'cancelled')", gate)

    def test_scope_uses_merge_group_base(self):
        self.assertIn("github.event.merge_group.base_sha", job_block(BUILD, "check-event"))

    def test_stamp_never_skips_in_merge_queue(self):
        ci_mode = job_block(BUILD, "ci-mode")
        self.assertRegex(ci_mode, r"if: github\.event_name != 'merge_group'")


if __name__ == "__main__":
    unittest.main()
