#!/usr/bin/env -S uv run --script
"""The opt-in #3437 compiler-cache qualification must retain trustworthy evidence."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import cache_source_probe as probe


class SourceProbeTest(unittest.TestCase):
    def qualify(self, outcomes, *, environment=None, executable="/selected/soldr"):
        calls = []

        def run(args, **kwargs):
            calls.append((args, kwargs))
            if args[1:] == ["--version"]:
                return subprocess.CompletedProcess(args, 0, "soldr 0.9.51\n", "")
            outcome = next(outcomes)
            if isinstance(outcome, BaseException):
                raise outcome
            code, output = outcome
            return subprocess.CompletedProcess(args, code, output, "")

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "qualification"
            with patch.dict(os.environ, environment or {}, clear=True), patch.object(
                probe.subprocess, "run", side_effect=run
            ):
                result = probe.probe(directory, executable, "0.9.51")
            persisted = json.loads((directory / "result.json").read_text())
            self.assertEqual(result, persisted)
            self.logs = {path.name: path.read_text() for path in directory.glob("*.log")}
            return result, calls

    @staticmethod
    def diagnostic():
        return 101, f"error: {probe.MARKER}\n"

    def test_porcelain_and_inherited_target_cannot_hide_compiler_evidence(self):
        result, calls = self.qualify(
            iter([(0, ""), (0, "soldr[cache] obs_cache_source_probe [HIT!]"), self.diagnostic(), self.diagnostic()]),
            environment={"SOLDR_PORCELAIN": "1", "CARGO_TARGET_DIR": "/unrelated/target"},
        )
        self.assertTrue(result["safe"])
        for _, kwargs in calls[1:]:
            self.assertEqual(kwargs["env"]["SOLDR_PORCELAIN"], "0")
            self.assertEqual(kwargs["env"]["CARGO_INCREMENTAL"], "0")
            self.assertEqual(Path(kwargs["env"]["CARGO_TARGET_DIR"]), kwargs["cwd"] / "target")

    def test_no_warm_hit_cannot_qualify_a_bypassed_cache(self):
        result, _ = self.qualify(iter([(0, ""), (0, ""), self.diagnostic(), self.diagnostic()]))
        self.assertFalse(result["safe"])
        self.assertFalse(result["unchanged"]["cache_hit_reported"])

    def test_stale_false_green_is_rejected_despite_uncached_diagnostic(self):
        result, _ = self.qualify(iter([(0, ""), (0, "soldr[cache] obs_cache_source_probe [HIT!]"), (0, "soldr[cache] obs_cache_source_probe [HIT!]"), self.diagnostic()]))
        self.assertFalse(result["safe"])
        self.assertEqual(result["cached"]["returncode"], 0)
        self.assertTrue(result["control"]["probe_diagnostic"])

    def test_an_unrelated_crates_hit_does_not_qualify_the_probe(self):
        result, calls = self.qualify(iter([(0, ""), (0, "soldr[cache] unrelated [HIT!]")]))
        self.assertFalse(result["safe"])
        self.assertEqual(len(calls), 3)

    def test_timeout_keeps_partial_log_and_result_instead_of_losing_evidence(self):
        timeout = subprocess.TimeoutExpired("soldr", 120, output=b"partial compiler output\n")
        result, _ = self.qualify(iter([(0, ""), timeout]))
        self.assertFalse(result["safe"])
        self.assertTrue(result["unchanged"]["timed_out"])
        self.assertIsNone(result["unchanged"]["returncode"])
        self.assertEqual(self.logs["unchanged-replay.log"], "partial compiler output\n")

    def test_an_uncached_control_that_succeeds_cannot_qualify_the_fixture(self):
        result, _ = self.qualify(iter([(0, ""), (0, "soldr[cache] obs_cache_source_probe [HIT!]"), self.diagnostic(), (0, "")]))
        self.assertFalse(result["safe"])

    def test_failed_seed_is_persisted_and_stops_before_editing(self):
        result, calls = self.qualify(iter([(1, "failed seed")]))
        self.assertFalse(result["safe"])
        self.assertEqual(len(calls), 2)
        self.assertEqual(self.logs["warm.log"], "failed seed")

    def test_invalid_timeouts_are_rejected_before_starting_compilers(self):
        for timeout in (0, -1, float("inf"), float("nan")):
            with self.subTest(timeout=timeout), patch.object(probe.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "finite positive"):
                    probe.probe(Path("unused"), "/selected/soldr", None, timeout=timeout)
                run.assert_not_called()

    def test_relative_executable_is_resolved_before_changing_directory(self):
        result, calls = self.qualify(
            iter([(0, ""), (0, "soldr[cache] obs_cache_source_probe [HIT!]"), self.diagnostic(), self.diagnostic()]),
            executable="./candidate/soldr",
        )
        selected = str(Path("candidate/soldr").resolve())
        self.assertEqual(result["command"][0], selected)
        self.assertTrue(all(args[0] == selected for args, _ in calls))


if __name__ == "__main__":
    unittest.main()
