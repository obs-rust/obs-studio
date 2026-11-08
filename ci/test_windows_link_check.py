#!/usr/bin/env -S uv run --script
"""The Windows link check runs on the PR/merge-queue tier (queue coverage).

    uv run --no-project python -m unittest ci/test_windows_link_check.py

A COFF-only link failure (`undefined symbol: obs_encoder_get_caps`) passed
the merge queue because every Windows-target job is deferred to master.
These checks pin the cheap proxy: it links (not only checks) every test
binary for x86_64-pc-windows-msvc, C oracle included, and it runs on the
deferred tier: every non-docs PR and queue run.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path
from unittest import mock

import windows_link_check

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
JOB = "rust-windows-link"


def job_block(name: str) -> str:
    text = WORKFLOW.read_text()
    match = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)
    assert match, f"{name} job missing"
    return match.group(1)


class LinkArgsTest(unittest.TestCase):
    def test_links_every_test_binary_for_msvc(self):
        args = windows_link_check.link_args()
        self.assertEqual(args[0], "test")
        self.assertIn("--no-run", args)  # links; `check` would never reach lld-link
        self.assertIn("--workspace", args)
        self.assertNotIn("--exclude", args)  # the oracle's archive is the defect site
        self.assertEqual(args[args.index("--target") + 1], "x86_64-pc-windows-msvc")

    def test_main_adds_target_then_links_through_soldr(self):
        calls = []
        with mock.patch.object(windows_link_check.local_gate, "add_target", lambda t: calls.append(("add", t))), \
             mock.patch.object(windows_link_check.local_gate, "soldr_cargo", lambda a: calls.append(("cargo", a))):
            self.assertEqual(windows_link_check.main(), 0)
        self.assertEqual(calls, [("add", "x86_64-pc-windows-msvc"), ("cargo", windows_link_check.link_args())])

    def test_link_failure_propagates(self):
        import subprocess

        def fail(_args):
            raise subprocess.CalledProcessError(101, ["soldr", "cargo"])

        with mock.patch.object(windows_link_check.local_gate, "add_target", lambda t: None), \
             mock.patch.object(windows_link_check.local_gate, "soldr_cargo", fail):
            with self.assertRaises(subprocess.CalledProcessError):
                windows_link_check.main()


class WorkflowTest(unittest.TestCase):
    def setUp(self):
        self.block = job_block(JOB)
        self.condition = re.search(r"^    if: (.*)$", self.block, re.M).group(1)

    def test_runs_the_script_in_one_line(self):
        self.assertIn("run: uv run --no-project ci/windows_link_check.py", self.block)

    def test_selected_on_the_deferred_pr_and_queue_tier(self):
        self.assertIn("needs.check-event.outputs.deferred == 'true'", self.condition)
        self.assertNotIn("github.event_name", self.condition)  # pull_request and merge_group alike

    def test_never_stands_down_on_proofs_or_attestation(self):
        # A lane proof or stamp would leave the queue run with no conclusion.
        for skip in ("ci-mode", "cross_hit", "skip_rust-cross-lint", "cross_lint", "outputs.windows"):
            self.assertNotIn(skip, self.condition)
        self.assertIn("needs.check-event.outputs.docs_only != 'true'", self.condition)

    def test_is_linux_hosted_and_bounded(self):
        self.assertIn("runs-on: ubuntu-26.04", self.block)
        minutes = int(re.search(r"timeout-minutes: (\d+)", self.block).group(1))
        self.assertLessEqual(minutes, 30)  # far inside the queue's 60-minute response timeout


if __name__ == "__main__":
    unittest.main()
