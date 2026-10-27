#!/usr/bin/env -S uv run --script
"""A workflow carries a top-level concurrency group (ci-lint WF-002, #962).

    uv run python -m unittest ci/test_wf002_concurrency.py

Without a top-level `concurrency` key every run of a ref stacks up: a
superseded run keeps its runner and, when the check is required, holds the
merge queue behind work nobody is waiting for. ci-lint WF-002 reports each
upstream workflow that predates the convention; ci.toml records them as
[[exceptions]] until they gain their group, and the exception is deleted in
the same PR. This file pins the group each burned-down workflow must keep, so
a later edit that drops it fails here the same way the linter would.
"""

from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"

# Only the workflows already burned down. Each value is the exact top-level
# `concurrency` group the workflow must carry; add a row with the workflow and
# delete its ci.toml WF-002 exception in the same PR.
EXPECTED = {
    "check-format.yaml": "check-format-${{ github.event_name }}-${{ github.ref }}",
}


def concurrency_group(text: str) -> str:
    """Return the top-level concurrency group value, or "" when there is none.

    Reads a top-level `concurrency:` mapping whether it is written in block
    form (a `group:` line under it) or as a single-line flow mapping.
    """
    lines = text.splitlines()
    for i, line in enumerate(lines):
        if not line.startswith("concurrency:"):
            continue
        inline = line[len("concurrency:") :].strip()
        if inline:
            return inline
        for following in lines[i + 1 :]:
            stripped = following.strip()
            if stripped.startswith("group:"):
                return stripped[len("group:") :].strip()
            if stripped and not following[:1].isspace():
                break
        return ""
    return ""


class Wf002ConcurrencyTest(unittest.TestCase):
    def test_burned_down_workflows_keep_their_group(self):
        for name, group in EXPECTED.items():
            with self.subTest(workflow=name):
                path = WORKFLOWS / name
                self.assertTrue(path.exists(), f"{name} is gone")
                self.assertEqual(
                    concurrency_group(path.read_text(encoding="utf-8")), group
                )

    def test_a_check_format_run_cancels_the_one_it_supersedes(self):
        text = (WORKFLOWS / "check-format.yaml").read_text(encoding="utf-8")
        self.assertIn("cancel-in-progress: true", text)


if __name__ == "__main__":
    unittest.main()
