#!/usr/bin/env -S uv run --script
"""Cheap jobs stay off the scarce runner pools (#427).

    uv run python -m unittest ci/test_runner_pools.py

On 2026-10-16 only 3 jobs ran repo-wide while 12 runs queued: the
`ubuntu-26.04` pool was the bottleneck, and the format checks and the
queue-run canceller took slots from Tier 2 and Rust Tests for jobs that
need nothing from 26.04. They now run on the standard `ubuntu-latest`
pool. swift-format took one of the 5 macOS slots on every PR even with no
Swift change; it now runs only when a Swift file changed.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORMAT = ROOT / ".github" / "workflows" / "check-format.yaml"
PULL = ROOT / ".github" / "workflows" / "ci.yml"


def job(path: Path, name: str) -> str:
    text = path.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name} in {path.name}"
    return m.group(1)


class PoolTest(unittest.TestCase):
    def test_cheap_jobs_use_the_standard_pool(self):
        self.assertIn("runs-on: ubuntu-latest", job(FORMAT, "checks"))
        self.assertIn("runs-on: ubuntu-latest", job(PULL, "cancel-superseded"))


class SwiftGateTest(unittest.TestCase):
    def test_swift_format_runs_only_for_swift_changes(self):
        j = job(FORMAT, "swift-format")
        self.assertIn("runs-on: macos-26", j)
        self.assertIn("needs: checks", j)
        self.assertIn("if: needs.checks.outputs.swift == 'true'", j)

    def test_change_check_compares_against_the_base(self):
        j = job(FORMAT, "checks")
        self.assertIn("github.event.pull_request.base.sha", j)
        self.assertIn("github.event.merge_group.base_sha", j)
        # The comparison, and its '*.swift' pathspec, live in the script (#962).
        self.assertIn("run: uv run --no-project ci/swift_changes.py", j)


if __name__ == "__main__":
    unittest.main()
