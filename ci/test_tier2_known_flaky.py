#!/usr/bin/env -S uv run --script
"""Known-flaky Tier 2 retry contracts, using a fake CTest (Refs #4944)."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TOOLS = ROOT / "rust/tools/linux-validate"
FLAKE = "test_pulse_default_follow_pipewire"

FAKE_CTEST = r'''
import json
import sys
from pathlib import Path
from xml.etree import ElementTree as ET

state = Path(sys.argv[1])
plan = json.loads(state.read_text())
call = plan['calls']
plan['calls'] += 1
plan.setdefault('argv', []).append(sys.argv[2:])
state.write_text(json.dumps(plan))
result = plan['runs'][min(call, len(plan['runs']) - 1)]
if '--output-junit' in sys.argv and not result.get('missing_report'):
    path = Path(sys.argv[sys.argv.index('--output-junit') + 1])
    suite = ET.Element('testsuite')
    for name, status in result['tests'].items():
        case = ET.SubElement(suite, 'testcase', name=name, status=status)
        if status not in ('run', 'disabled'):
            if status in ('notrun', 'skip'):
                case.set('status', 'notrun')
                ET.SubElement(case, 'skipped', message=(
                    'SKIP_RETURN_CODE=77' if status == 'skip' else 'Unable to find executable'))
            else:
                ET.SubElement(case, 'failure')
    ET.ElementTree(suite).write(path)
print('fake ctest run', call + 1, flush=True)
sys.exit(result['status'])
'''


class KnownFlakyTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.registry = self.root / "known-flaky.txt"
        self.registry.write_text(f"^{FLAKE}$ #4944 default output switch race\n")
        self.fake = self.root / "ctest.py"
        self.fake.write_text(FAKE_CTEST)
        self.state = self.root / "state.json"
        self.summary = self.root / "summary.md"

    def invoke(self, runs, mode="OFF"):
        self.state.write_text(json.dumps({"calls": 0, "runs": runs}))
        result = subprocess.run(
            [sys.executable, str(TOOLS / "known_flaky.py"),
             "--registry", str(self.registry), "--build", str(self.root), "--mode", mode,
             "--", sys.executable, str(self.fake), str(self.state),
             "--test-dir", str(self.root), "--output-on-failure", "-j", "8", "-R", "initial-shard"],
            env={**os.environ, "GITHUB_STEP_SUMMARY": str(self.summary)},
            text=True, capture_output=True,
        )
        return result, json.loads(self.state.read_text())

    def test_registered_failures_pass_once_with_warning_and_summary(self):
        for mode in ("OFF", "ON"):
            with self.subTest(mode=mode):
                result, state = self.invoke([
                    {"status": 8, "tests": {FLAKE: "fail", "healthy": "run"}},
                    {"status": 0, "tests": {FLAKE: "run"}},
                ], mode)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(state["calls"], 2)
                self.assertNotIn("--rerun-failed", state["argv"][0])
                self.assertIn("--rerun-failed", state["argv"][1])
                self.assertIn("--no-tests=error", state["argv"][1])
                for arguments in state["argv"]:
                    self.assertEqual(arguments[:7], ["--test-dir", str(self.root),
                                     "--output-on-failure", "-j", "8", "-R", "initial-shard"])
                message = f"known-flaky {FLAKE} failed then passed on retry (#4944)"
                self.assertIn("::warning::" + message, result.stdout)
                self.assertIn(message, self.summary.read_text())
                self.assertIn(mode, self.summary.read_text())

    def test_registered_failure_twice_fails(self):
        result, state = self.invoke([{"status": 8, "tests": {FLAKE: "fail"}}])
        self.assertEqual(result.returncode, 8)
        self.assertEqual(state["calls"], 2)
        self.assertNotIn("::warning::", result.stdout)

    def test_mixed_failure_does_not_retry(self):
        result, state = self.invoke([{"status": 8, "tests": {FLAKE: "fail", "unknown": "fail"}}])
        self.assertEqual(result.returncode, 8)
        self.assertEqual(state["calls"], 1)
        self.assertFalse(self.summary.exists())

    def test_mixed_not_run_error_does_not_retry(self):
        result, state = self.invoke([{"status": 8, "tests": {FLAKE: "fail", "missing": "notrun"}}])
        self.assertEqual(result.returncode, 8)
        self.assertEqual(state["calls"], 1)

    def test_explicit_skip_does_not_prevent_registered_failure_retry(self):
        for optional_status in ('skip', 'disabled'):
            with self.subTest(optional_status=optional_status):
                result, state = self.invoke([
                    {"status": 8, "tests": {FLAKE: "fail", "optional": optional_status}},
                    {"status": 0, "tests": {FLAKE: "run"}},
                ])
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(state["calls"], 2)

    def test_malformed_registry_is_error_before_ctest(self):
        for line in (f"{FLAKE} missing issue", f"{FLAKE} #0 reason",
                     f"{FLAKE} #4944", "[ #4944 invalid regex"):
            with self.subTest(line=line):
                self.registry.write_text(line + "\n")
                result, state = self.invoke([{"status": 0, "tests": {FLAKE: "run"}}])
                self.assertEqual(result.returncode, 2, result.stdout)
                self.assertIn("known-flaky.txt:1", result.stderr)
                self.assertEqual(state["calls"], 0)

    def test_normal_pass_does_not_retry(self):
        result, state = self.invoke([{"status": 0, "tests": {"healthy": "run"}}])
        self.assertEqual(result.returncode, 0)
        self.assertEqual(state["calls"], 1)
        self.assertFalse(self.summary.exists())

    def test_regex_must_match_whole_test_name(self):
        self.registry.write_text(f"{FLAKE} #4944 default output race\n")
        result, state = self.invoke([{"status": 8, "tests": {FLAKE + "_other": "fail"}}])
        self.assertEqual(result.returncode, 8)
        self.assertEqual(state["calls"], 1)

    def test_multiple_registered_failures_retry_together(self):
        self.registry.write_text("# tracked tests\n\nfixture_[ab] #4944 fixture race\n")
        result, state = self.invoke([
            {"status": 8, "tests": {"fixture_a": "fail", "fixture_b": "fail"}},
            {"status": 0, "tests": {"fixture_a": "run", "fixture_b": "run"}},
        ])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state["calls"], 2)
        self.assertEqual(result.stdout.count("::warning::"), 2)

    def test_validate_wraps_normal_suites_but_not_determinism_repeat(self):
        text = (TOOLS / "validate.sh").read_text()
        self.assertIn('"$SRC/test/cmocka/known-flaky.txt"', text)
        self.assertIn('--mode "$mode" --)', text)
        self.assertIn('"${known_flaky[@]}" "${mode_env[@]}" xvfb-run', text)
        self.assertIn('"${known_flaky[@]}" uv run --no-project', text)
        repeat = text.split('if [ "$repeat" -gt 0 ]; then', 1)[1]
        self.assertNotIn('known_flaky', repeat)

    def test_missing_report_or_ctest_error_does_not_retry(self):
        for run in ({"status": 8, "tests": {}, "missing_report": True},
                    {"status": 8, "tests": {}},
                    {"status": 1, "tests": {FLAKE: "fail"}}):
            with self.subTest(run=run):
                result, state = self.invoke([run])
                self.assertEqual(result.returncode, run["status"])
                self.assertEqual(state["calls"], 1)

    def test_retry_must_actually_pass_every_failed_test(self):
        for tests in ({}, {FLAKE: "notrun"}, {"healthy": "run"}, {FLAKE: "fail"}):
            with self.subTest(tests=tests):
                result, state = self.invoke([
                    {"status": 8, "tests": {FLAKE: "fail"}},
                    {"status": 0, "tests": tests},
                ])
                self.assertEqual(result.returncode, 8)
                self.assertEqual(state["calls"], 2)
                self.assertNotIn("::warning::", result.stdout)


if __name__ == "__main__":
    unittest.main()
