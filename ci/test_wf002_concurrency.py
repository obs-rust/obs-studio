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
    "dispatch.yaml": "dispatch-${{ inputs.job }}-${{ github.event_name }}-${{ github.ref }}",
    "push.yaml": "push-${{ github.event_name }}-${{ github.ref }}",
    "sign-windows.yaml": "sign-windows-${{ github.event_name }}-${{ github.ref }}",
    "analyze-project.yaml": "analyze-project-${{ github.event_name }}-${{ github.ref }}",
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

    def test_a_dispatch_never_cancels_an_in_flight_run(self):
        """dispatch.yaml is manual and multi-job, so its group keys on `inputs.job`
        (two different dispatches never serialize) and a second dispatch never
        kills an in-flight release operation (the claim-bot.yaml precedent)."""
        text = (WORKFLOWS / "dispatch.yaml").read_text(encoding="utf-8")
        self.assertIn("cancel-in-progress: false", text)

    def test_a_push_run_waits_for_the_one_in_progress(self):
        # push.yaml warms master's caches; #1286 preserves the one active warm
        # build and only drops older pending snapshots, so its superseded run
        # must not be cancelled.
        text = (WORKFLOWS / "push.yaml").read_text(encoding="utf-8")
        self.assertIn("cancel-in-progress: false", text)

    def test_a_sign_windows_run_cancels_the_one_it_supersedes(self):
        text = (WORKFLOWS / "sign-windows.yaml").read_text(encoding="utf-8")
        self.assertIn("cancel-in-progress: true", text)

    def test_a_superseded_analyze_project_run_is_cancelled(self):
        text = (WORKFLOWS / "analyze-project.yaml").read_text(encoding="utf-8")
        self.assertIn("cancel-in-progress: true", text)

if __name__ == "__main__":
    unittest.main()
