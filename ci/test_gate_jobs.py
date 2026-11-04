#!/usr/bin/env -S uv run --script
"""CI Gate fails on a cancelled job nested in a reusable workflow (#1793).

    uv run python -m unittest ci/test_gate_jobs.py
"""

from __future__ import annotations

import contextlib
import io
import os
import re
import subprocess
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
import gate_jobs
from gate_jobs import blocking

PULL = (ROOT / ".github" / "workflows" / "ci.yml").read_text()


def job(name: str, conclusion: str | None, status: str = "completed") -> dict:
    return {"name": name, "status": status, "conclusion": conclusion}


# Run 39677084695 attempt 1 (2026-10-21): both macOS jobs were never acquired
# by a hosted runner and ended cancelled, yet CI Gate succeeded.
RUN_39677084695 = [
    job("Validate Data 🕵️", "success"),
    job("Format 🔍 / checks", "success"),
    job("Build 🧱 / Event Data 🔎", "success"),
    job("Build 🧱 / CI Mode 🛂", "success"),
    job("Build 🧱 / Fast Gate ⚡", "success"),
    job("Build 🧱 / Rust Tests 🦀 (windows-2025-vs2026)", "success"),
    job("Build 🧱 / Rust Tier 2 windows-2025-vs2026", "success"),
    job("Build 🧱 / Rust Tier 2 Linux 🐧 (build)", "success"),
    job("Build 🧱 / Rust Cross Lint 🦀 (Windows, macOS)", "success"),
    job("Build 🧱 / Rust Tests 🦀 (macos-26)", "cancelled"),
    job("Build 🧱 / Rust Tier 2 macos-26", "cancelled"),
    job("Build 🧱 / Build macOS", "skipped"),
    job("CI Gate ✅", None, "in_progress"),
]


class GateJobs(unittest.TestCase):
    def test_runnerless_cancellation_is_diagnosed_even_without_scope(self):
        jobs = [dict(job("Build / macOS", "cancelled"), runner_name="", runner_id=0, steps=[])]
        out = io.StringIO()
        with (
            mock.patch.dict(os.environ, {"GITHUB_EVENT_NAME": "merge_group", "CORE_SCOPE": ""}),
            mock.patch.object(gate_jobs, "run_jobs", return_value=jobs),
            contextlib.redirect_stdout(out),
        ):
            self.assertEqual(gate_jobs.main(), 1)
        self.assertIn("runner capacity", out.getvalue())
        self.assertIn("Build / macOS", out.getvalue())

    def test_queue_scope_is_independent_of_reusable_output(self):
        with (
            mock.patch.dict(os.environ, {"GITHUB_EVENT_NAME": "merge_group", "CORE_SCOPE": ""}),
            mock.patch.object(gate_jobs, "queue_core_scope", return_value=True, create=True),
            mock.patch.object(gate_jobs, "run_jobs", return_value=[job("Build 🧱 / Fast Gate ⚡", "success")]),
            contextlib.redirect_stdout(io.StringIO()),
        ):
            self.assertEqual(gate_jobs.main(), 0)

    def test_core_batch_cannot_pass_with_native_tier2_skipped(self):
        from ci_scope import scope

        core = scope(["rust/obs-graphics/src/effect_parser.rs", "libobs/graphics/effect.c.h"], event="merge_group").core
        names = ["Build 🧱 / Rust Tier 2 (windows-2025-vs2026)", "Build 🧱 / Rust Tier 2 (macos-26)"]
        for conclusion in ("skipped", "failure", "cancelled", None):
            with self.subTest(conclusion=conclusion):
                self.assertEqual(blocking([job(n, conclusion) for n in names], require_native=core), names)
        self.assertEqual(blocking([], require_native=core), names)
        self.assertEqual(blocking([job(n, "success") for n in names], require_native=core), [])
        self.assertEqual(blocking([job(n, "skipped") for n in names]), [])

    def test_gate_receives_core_scope_from_reusable_workflow(self):
        build = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertTrue("value: ${{ jobs.check-event.outputs.core }}" in build)
        gate = PULL.split("  ci-gate:\n", 1)[1].split("  tier2-lane-proof:", 1)[0]
        self.assertIn("CORE_SCOPE: ${{ needs.build-project.outputs.core }}", gate)

    def test_main_requires_native_success_only_for_core_merge_groups(self):
        names = ["Build 🧱 / Rust Tier 2 (windows-2025-vs2026)", "Build 🧱 / Rust Tier 2 (macos-26)"]
        for event, core, conclusion, expected in (
            ("merge_group", "true", "skipped", 1),
            ("merge_group", "true", "success", 0),
            ("merge_group", "false", "skipped", 0),
            ("pull_request", "true", "skipped", 0),
            ("merge_group", "", "success", 1),
        ):
            with (
                self.subTest(event=event, core=core, conclusion=conclusion),
                mock.patch.dict(os.environ, {"GITHUB_EVENT_NAME": event, "CORE_SCOPE": core}),
                mock.patch.object(gate_jobs, "run_jobs", return_value=[job(n, conclusion) for n in names]),
                contextlib.redirect_stdout(io.StringIO()),
            ):
                self.assertEqual(gate_jobs.main(), expected)

    def test_historical_cancelled_macos_jobs_block(self):
        self.assertEqual(blocking(RUN_39677084695), [
            "Build 🧱 / Rust Tests 🦀 (macos-26)",
            "Build 🧱 / Rust Tier 2 macos-26",
        ])

    def test_scope_skips_and_successes_pass(self):
        self.assertEqual(blocking([j for j in RUN_39677084695 if j["conclusion"] != "cancelled"]), [])

    def test_every_unsuccessful_conclusion_blocks(self):
        for conclusion in ("failure", "cancelled", "timed_out", "startup_failure", "action_required"):
            self.assertEqual(blocking([job("Build 🧱 / x", conclusion)]), ["Build 🧱 / x"], conclusion)

    def test_maintenance_jobs_never_block(self):
        # ci-pre's sweep and the end-of-queue janitor sit outside the gate.
        self.assertEqual(blocking([
            job("CI Pre 🧹 / Cache Janitor", "failure"),
            job("Cache Janitor 🧹", "cancelled"),
            job("CI Gate ✅", "failure"),
        ]), [])

    def test_unfinished_jobs_are_ignored(self):
        self.assertEqual(blocking([job("Cache Janitor 🧹", None, "queued"), job("Build 🧱 / y", None, "in_progress")]), [])

    def test_gate_runs_the_check_on_every_event(self):
        gate = re.search(r"^  ci-gate:\n((?:    .*\n|\s*\n)*)", PULL, re.M).group(1)
        step = re.search(r"- name: Check Run Jobs ✅\n((?:        .*\n)*)", gate)
        self.assertIsNotNone(step, "CI Gate must list the run's jobs (ci/gate_jobs.py)")
        self.assertIn("uv run --no-project ci/gate_jobs.py", step.group(1))
        self.assertNotIn("if:", step.group(1))


def done(returncode: int, stdout: str = "", stderr: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess(args=["gh"], returncode=returncode, stdout=stdout, stderr=stderr)


class JobListCall(unittest.TestCase):
    """The jobs API is a network call: retry it, and say why it failed (#3124)."""

    def setUp(self):
        # This fixture tests API errors, independently of the host workflow's event.
        patcher = mock.patch.dict(os.environ, {
            "GITHUB_REPOSITORY": "obs-rust/obs-studio", "GITHUB_RUN_ID": "1",
            "GITHUB_EVENT_NAME": "pull_request",
        })
        patcher.start()
        self.addCleanup(patcher.stop)

    def test_a_transient_api_failure_is_retried(self):
        outputs = [
            done(1, stderr="HTTP 502: Bad Gateway"),
            done(1, stderr="gh: Server Error (502)"),
            done(0, '{"name": "Build 🧱 / x"}\n{"name": "CI Gate ✅"}\n'),
        ]
        calls = []
        slept = []

        def runner(argv, **kwargs):
            calls.append(list(argv))
            return outputs.pop(0)

        jobs = gate_jobs.run_jobs(runner=runner, sleep=slept.append)

        self.assertEqual([j["name"] for j in jobs], ["Build 🧱 / x", "CI Gate ✅"])
        self.assertEqual(len(calls), 3, "the call must be retried until it succeeds")
        self.assertEqual(len(slept), 2, "a backoff must separate the attempts")
        self.assertIn("api", calls[0])
        self.assertTrue(any(".jobs[]" in arg for arg in calls[0]))

    def test_every_attempt_failing_reports_ghs_stderr_and_the_attempt_count(self):
        slept = []

        def runner(argv, **kwargs):
            return done(1, stderr="gh: API rate limit exceeded for installation")

        with self.assertRaises(gate_jobs.JobListError) as caught:
            gate_jobs.run_jobs(runner=runner, sleep=slept.append)

        message = str(caught.exception)
        self.assertIn("API rate limit exceeded", message)
        self.assertIn(f"{gate_jobs.ATTEMPTS} times", message)
        self.assertEqual(len(slept), gate_jobs.ATTEMPTS - 1)

    def test_main_reports_the_reason_instead_of_a_traceback(self):
        def runner(argv, **kwargs):
            return done(1, stderr="gh: Server Error (502)")

        out = io.StringIO()
        env = {"GITHUB_REPOSITORY": "obs-rust/obs-studio", "GITHUB_RUN_ID": "1"}
        with (
            mock.patch.dict(os.environ, env),
            mock.patch.object(gate_jobs.subprocess, "run", runner),
            mock.patch.object(gate_jobs.time, "sleep", lambda _s: None),
            contextlib.redirect_stdout(out),
        ):
            self.assertEqual(gate_jobs.main(), 1)

        printed = out.getvalue()
        self.assertIn("::error::", printed)
        self.assertIn("Server Error (502)", printed)


if __name__ == "__main__":
    unittest.main()
