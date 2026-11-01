"""`ci/benchmark_libobs.py --runtime` summarises repeated runtime benchmark runs (#3385).

Usage:
    uv run python -m unittest ci/test_benchmark_libobs_runtime.py
"""

import importlib.util
import json
import os
import stat
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("benchmark_libobs", ROOT / "ci/benchmark_libobs.py")
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def sample(frame_ns: int, graphics: float) -> dict:
    return {
        "average_frame_time_ns": frame_ns,
        "lagged_frames": 10,
        "cpu_seconds_per_second": 2.0,
        "thread_cpu_seconds_per_second": {"graphics": graphics, "audio": 0.003},
    }


class SummaryTest(unittest.TestCase):
    def test_median_and_spread_of_every_numeric_field(self):
        summary = MODULE.summarize_runtime([sample(30, 0.2), sample(10, 0.1), sample(20, 0.3)])
        self.assertEqual(summary["average_frame_time_ns"], {"median": 20, "min": 10, "max": 30, "spread": 1.0})
        self.assertEqual(summary["thread_cpu_seconds_per_second.graphics"]["median"], 0.2)
        self.assertEqual(summary["lagged_frames"]["spread"], 0.0)

    def test_no_runs_is_refused(self):
        with self.assertRaises(ValueError):
            MODULE.summarize_runtime([])


class RunTest(unittest.TestCase):
    def test_runs_the_executable_and_keeps_every_sample(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        fake = tmp / "obs_runtime_bench"
        fake.write_text('#!/bin/sh\necho "log line"\necho \'{"average_frame_time_ns": 5}\'\n', encoding="utf-8")
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        result = MODULE.measure_runtime(fake, tmp / "out", 2, 1.0, 0.0)
        self.assertEqual(len(result["runs"]), 2)
        self.assertEqual(result["summary"]["average_frame_time_ns"]["median"], 5)
        self.assertEqual(json.loads((tmp / "out/runtime.json").read_text())["runs"][0]["average_frame_time_ns"], 5)
        self.assertIn("load_average", result["runs"][0])

    def test_a_skipped_run_fails(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        fake = tmp / "obs_runtime_bench"
        fake.write_text("#!/bin/sh\nexit 77\n", encoding="utf-8")
        fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
        with self.assertRaises(RuntimeError):
            MODULE.measure_runtime(fake, tmp / "out", 1, 1.0, 0.0)


if __name__ == "__main__":
    os.environ.setdefault("PYTHONHASHSEED", "0")
    unittest.main()
