#!/usr/bin/env -S uv run --script
"""The CI cache report reads what `soldr cache report --json` really emits.

    uv run --no-project python -m unittest ci/test_soldr_cache_report.py

The fixture is unedited output of soldr 0.9.45, the version build-project.yaml
pins, after one real cached rebuild (1 hit, 3653 ms saved). Its counters sit
under `last_session`; a top-level parser reads them all as zero.
"""

from __future__ import annotations

import contextlib
import io
import json
import os
import re
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "ci"))

import soldr_cache_report as report  # noqa: E402

FIXTURE = ROOT / "ci" / "fixtures" / "soldr_cache_report_0.9.45.json"
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
NO_SESSION = "no soldr cache session recorded"


def job_block(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    match = re.search(rf"^  {name}:\n(.*?)(?=^  [\w-]+:\n|\Z)", text, re.M | re.S)
    assert match, name
    return match.group(1)


def step_block(job: str, step: str) -> str:
    match = re.search(rf"^      - name: {step}.*?(?=^      - |\Z)", job, re.M | re.S)
    assert match, step
    return match.group(0)


class RealFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.raw = FIXTURE.read_text(encoding="utf-8")

    def test_counters_come_from_last_session(self) -> None:
        out = report.format_report(self.raw, wrapper=None)
        self.assertIn("**100.0%** (1 hits / 0 misses)", out)
        self.assertIn("time saved: 3.7 s", out)
        self.assertIn("compilations: 1 (non-cacheable 0)", out)

    def test_status_errors_version_and_source(self) -> None:
        out = report.format_report(self.raw, wrapper=None)
        self.assertIn("status: ok (errors 0)", out)
        self.assertIn("zccache: 1.15.5", out)
        self.assertIn("session source: global-fallback", out)

    def test_bypassed_wrapper_with_no_compilations_is_not_zero_percent(self) -> None:
        data = json.loads(self.raw)
        data["last_session"].update(compilations=0, hits=0, hit_rate=0.0, time_saved_ms=0)
        out = report.format_report(json.dumps(data), wrapper="none")
        self.assertIn("Rust compiler cache: n/a (bypassed)", out)
        self.assertIn("hit rate: n/a (bypassed)", out)
        self.assertNotIn("0.0%", out)

    def test_bypassed_wrapper_keeps_real_non_rust_numbers_labelled(self) -> None:
        out = report.format_report(self.raw, wrapper="none")
        self.assertIn("Rust compiler cache: n/a (bypassed)", out)
        self.assertIn("hit rate (non-Rust compilers): **100.0%**", out)

    def test_active_wrapper_with_no_compilations_is_not_zero_percent(self) -> None:
        data = json.loads(self.raw)
        data["last_session"].update(compilations=0, hits=0, hit_rate=0.0)
        out = report.format_report(json.dumps(data), wrapper=None)
        self.assertIn("hit rate: n/a (no compilations)", out)
        self.assertNotIn("0.0%", out)

    def test_absent_or_null_session(self) -> None:
        data = json.loads(self.raw)
        data["last_session"] = None
        self.assertIn(NO_SESSION, report.format_report(json.dumps(data), wrapper=None))
        del data["last_session"]
        self.assertIn(NO_SESSION, report.format_report(json.dumps(data), wrapper=None))


class FailOpen(unittest.TestCase):
    def run_main(self, env: dict[str, str]) -> tuple[int, str]:
        stdout = io.StringIO()
        with mock.patch.dict(os.environ, env), contextlib.redirect_stdout(stdout):
            code = report.main()
        return code, stdout.getvalue()

    def stub_soldr(self, directory: str, body: str) -> dict[str, str]:
        stub = Path(directory) / "soldr"
        stub.write_text(f"#!/bin/sh\n{body}\n", encoding="utf-8")
        stub.chmod(0o755)
        return {"PATH": directory}

    def test_missing_soldr(self) -> None:
        with tempfile.TemporaryDirectory() as empty:
            code, out = self.run_main({"PATH": empty})
        self.assertEqual(code, 0)
        self.assertIn(NO_SESSION, out)

    @unittest.skipIf(os.name == "nt", "the stub is a POSIX shell script")
    def test_failing_soldr(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            code, out = self.run_main(self.stub_soldr(directory, "exit 3"))
        self.assertEqual(code, 0)
        self.assertIn(NO_SESSION, out)

    @unittest.skipIf(os.name == "nt", "the stub is a POSIX shell script")
    def test_malformed_json(self) -> None:
        for body in ("echo '{not json'", "echo '[1]'", """echo '{"last_session": {"hit_rate": "x", "compilations": 1}}'"""):
            with self.subTest(body=body), tempfile.TemporaryDirectory() as directory:
                code, out = self.run_main(self.stub_soldr(directory, body))
                self.assertEqual(code, 0)
                self.assertIn("### soldr cache", out)


class WorkflowWiring(unittest.TestCase):
    STEP = "Report soldr cache"

    def test_report_runs_the_parser_through_uv_and_stays_fail_open(self) -> None:
        for name in ("macos-build", "rust-tests-linux"):
            with self.subTest(job=name):
                step = step_block(job_block(name), self.STEP)
                self.assertIn("uv run --no-project ci/soldr_cache_report.py", step)
                self.assertIn("||", step)
                self.assertIn("always()", step)

    def test_macos_build_installs_uv_when_native_reuse_did_not(self) -> None:
        job = job_block("macos-build")
        step = step_block(job, "Set Up uv for the soldr Cache Report")
        self.assertIn("native-cache-reuse-source == ''", step)
        self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", step)
        self.assertLess(job.index("Set Up uv for the soldr Cache Report"), job.index(self.STEP))

    def test_s3_size_step_lives_where_its_condition_can_hold(self) -> None:
        self.assertNotIn("Report S3 cache size", job_block("macos-build"))
        job = job_block("rust-tests-linux")
        self.assertIn("runs-on: ubuntu-", job)
        step = step_block(job, "Report S3 cache size")
        self.assertNotIn("matrix.os", step)
        self.assertIn("command -v aws", step)

    def test_wrapper_stays_bypassed(self) -> None:
        self.assertIn("  SOLDR_RUSTC_WRAPPER: none\n", WORKFLOW.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
