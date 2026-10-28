#!/usr/bin/env -S uv run --script
"""Qt's useful compiler working set fits its declared and enforced budget."""
import re
import os
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class QtCacheBudgetTest(unittest.TestCase):
    def setUp(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.job = text.split("  linux-qt-frontend:\n", 1)[1].split("\n  rust-tier2-macos:", 1)[0]
        self.family = tomllib.loads((ROOT / "ci.toml").read_text())["cache"]["family"]["qt-ccache"]

    def test_budget_fits_observed_entry_and_enforced_limit(self):
        # After current-build pruning, real warm consumer #1696 used 508,489,050 bytes.
        amount, unit = re.fullmatch(r"([0-9.]+)(MB|GB)", self.family["max"]).groups()
        declared = float(amount) * (1024**2 if unit == "MB" else 1024**3)
        self.assertGreaterEqual(declared, 508_489_050)
        limit = re.search(r"CCACHE_MAXSIZE: (\d+)M", self.job)
        self.assertIsNotNone(limit, "bound cache contents before upload")
        self.assertLess(int(limit.group(1)) * 1_000_000, declared)
        self.assertIn('ccache --max-size="$CCACHE_MAXSIZE"', self.job)
        self.assertIn("ccache --cleanup", self.job)

    def test_bounded_allocation_preserves_measured_working_headroom(self):
        # #1696's successful real warm consumer: 306/310 hits and all Qt tests passed.
        observed = 508_489_050
        amount, unit = re.fullmatch(r"([0-9.]+)(MB|GB)", self.family["max"]).groups()
        declared = float(amount) * {"MB": 1024**2, "GB": 1024**3}[unit]
        working = int(re.search(r"CCACHE_MAXSIZE: (\d+)M", self.job)[1]) * 1_000_000
        self.assertGreaterEqual(working, observed * 1.25, "preserve 25% working growth room")
        self.assertLessEqual(working, 650_000_000, "bound the measured working allocation")
        self.assertGreater(declared, working, "archive allowance must cover the working cap")
        self.assertLessEqual(declared, 750 * 1024**2, "release unused reservation after pruning")

    def test_hit_statistics_describe_this_job_only(self):
        self.assertLess(self.job.index("ccache --zero-stats"), self.job.index("- name: Build Frontend"))
        self.assertGreater(self.job.index("ccache --show-stats --verbose"), self.job.index("- name: Build Frontend"))

    def run_cache_steps(self, *, prepare=True, ref="refs/heads/master"):
        def script(name):
            step = self.job.split(f"- name: {name}", 1)[1].split("\n      - ", 1)[0]
            body = step.split("        run: |\n", 1)[1]
            return "\n".join(line[10:] for line in body.splitlines())

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tools = root / "tools"
            tools.mkdir()
            (tools / "ccache").write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$CACHE_CALLS"\n')
            (tools / "date").write_text('#!/bin/sh\nprintf "%s\\n" "$TEST_NOW"\n')
            for tool in tools.iterdir():
                tool.chmod(0o755)
            output, calls, cache = root / "outputs", root / "calls", root / "cache"
            output.touch()
            cache.mkdir()
            env = {**os.environ, "PATH": f"{tools}{os.pathsep}{os.environ['PATH']}",
                   "GITHUB_OUTPUT": str(output), "CACHE_CALLS": str(calls),
                   "CCACHE_DIR": str(cache),
                   "CCACHE_MAXSIZE": re.search(r"CCACHE_MAXSIZE: (\d+M)", self.job)[1],
                   "GITHUB_REF": ref, "TEST_NOW": "1000"}
            if prepare:
                subprocess.run(["bash", "-euo", "pipefail", "-c", script("Set Qt Cache Limit")],
                               env=env, check=True, capture_output=True, text=True)
            epoch = output.read_text().strip().removeprefix("epoch=")
            subprocess.run(["bash", "-euo", "pipefail", "-c", script("Report Qt Cache")],
                           env={**env, "TEST_NOW": "1300", "QT_CACHE_EPOCH": epoch},
                           check=True, capture_output=True, text=True)
            return output.read_text(), calls.read_text().splitlines()

    def test_recorded_epoch_keeps_current_build_entries_with_a_margin(self):
        self.assertIn("id: qt-cache-epoch", self.job)
        self.assertIn("QT_CACHE_EPOCH: ${{ steps.qt-cache-epoch.outputs.epoch }}", self.job)
        output, calls = self.run_cache_steps()
        self.assertEqual(output, "epoch=1000\n")
        self.assertIn("--evict-older-than 360s", calls)
        self.assertLess(calls.index("--evict-older-than 360s"), calls.index("--cleanup"))

    def test_pr_job_measures_pruning_without_publishing_a_cache(self):
        _, calls = self.run_cache_steps(ref="refs/pull/1/merge")
        self.assertIn("--evict-older-than 360s", calls)
        save = self.job.split("- name: Save Qt ccache", 1)[1]
        self.assertIn("github.ref == 'refs/heads/master'", save)

    def test_missing_preparation_epoch_does_not_age_evict(self):
        _, calls = self.run_cache_steps(prepare=False)
        self.assertFalse(any("--evict-older-than" in call for call in calls))


if __name__ == "__main__":
    unittest.main()
