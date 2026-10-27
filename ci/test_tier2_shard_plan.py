#!/usr/bin/env -S uv run --script
"""Tier 2 Linux shards are balanced by recorded test duration (#1048).

    uv run python -m unittest ci/test_tier2_shard_plan.py

`ctest -I <i>,,3` striped tests by number, so one shard ran tests for
153 s and another for 63 s (master run 38874213546). shard-plan.py assigns
tests longest-first to the lightest shard using ctest's own cost data
(CTestCostData.txt, carried in the Tier 2 ccache since #931) and writes one
exact-name regex per shard for `ctest -R`.
"""

from __future__ import annotations

import os
import re
import json
import subprocess
import sys
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PLAN = ROOT / "rust" / "tools" / "linux-validate" / "shard-plan.py"
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
CHECK = PLAN.parent / "check-plan.py"

# Shaped like the real data: a few long real-time tests, many short ones.
COSTS = {
    "test_deinterlace_2x_field_order": 82.0,
    "test_async_delay_audio": 59.0,
    "test_startup_cycle_stress": 48.0,
    "test_portal_shortcuts": 43.0,
    "test_vlc_audio_pacing": 35.0,
    "test_vlc_playlist_gap": 26.0,
    **{f"test_short_{i:02d}": 0.5 + (i % 7) * 0.3 for i in range(40)},
}


def run_plan(costs: dict[str, float], names: list[str], shards: int) -> dict[int, list[str]]:
    d = Path(tempfile.mkdtemp())
    cost = d / "CTestCostData.txt"
    cost.write_text("".join(f"{n} 3 {c}\n" for n, c in costs.items()) + ("---\ntest_flaky\n" if costs else ""))
    listing = d / "names.txt"
    listing.write_text("\n".join(names) + "\n")
    out = d / "out"
    r = subprocess.run([sys.executable, str(PLAN), str(cost), str(listing), str(shards), str(out)],
                       capture_output=True, text=True,
                       env={k: v for k, v in os.environ.items() if k != "DEFAULT_COST"})
    assert r.returncode == 0, r.stdout + r.stderr
    plan = {}
    for i in range(1, shards + 1):
        regex = (out / f"{i}-of-{shards}.regex").read_text().strip()
        m = re.fullmatch(r"\^\((.*)\)\$", regex)
        assert m, regex
        plan[i] = m.group(1).split("|") if m.group(1) else []
    return plan


class ShardPlanTest(unittest.TestCase):
    def test_every_test_in_exactly_one_shard(self):
        names = list(COSTS)
        plan = run_plan(COSTS, names, 3)
        assigned = [n for tests in plan.values() for n in tests]
        self.assertEqual(sorted(assigned), sorted(names))

    def test_loads_are_balanced(self):
        plan = run_plan(COSTS, list(COSTS), 3)
        loads = [sum(COSTS[n] for n in tests) for tests in plan.values()]
        total = sum(COSTS.values())
        # Longest-first greedy keeps the heaviest shard near total/3 (the
        # 82 s test alone bounds it from below).
        self.assertLessEqual(max(loads), max(total / 3 * 1.15, 82.0 + 5))

    def test_longest_tests_go_to_different_shards(self):
        plan = run_plan(COSTS, list(COSTS), 3)
        where = {n: i for i, tests in plan.items() for n in tests}
        top3 = ["test_deinterlace_2x_field_order", "test_async_delay_audio", "test_startup_cycle_stress"]
        self.assertEqual(len({where[n] for n in top3}), 3)

    def test_tests_without_cost_data_are_still_planned(self):
        names = list(COSTS) + ["test_brand_new"]
        plan = run_plan(COSTS, names, 3)
        self.assertIn("test_brand_new", [n for t in plan.values() for n in t])

    def test_no_cost_data_still_splits_every_test(self):
        """First run, or no saved timings: the cost file is empty. awk's
        FNR == NR then misread the names as costs and every shard came out
        empty."""
        plan = run_plan({}, ["test_a", "test_b", "test_c", "test_d"], 2)
        self.assertEqual(sorted(n for t in plan.values() for n in t), ["test_a", "test_b", "test_c", "test_d"])
        self.assertEqual(sorted(len(t) for t in plan.values()), [2, 2])

    def test_regex_matches_exact_names_only(self):
        plan = run_plan({"test_a": 1.0, "test_ab": 1.0}, ["test_a", "test_ab"], 1)
        self.assertEqual(sorted(plan[1]), ["test_a", "test_ab"])

    def test_default_cost_applies_to_tests_without_timings(self):
        """DEFAULT_COST replaces the 1 s a test with no recorded cost gets."""
        with tempfile.TemporaryDirectory() as directory:
            d = Path(directory)
            (d / "cost").write_text("")
            (d / "names").write_text("test_a\ntest_b\n")
            env = {k: v for k, v in os.environ.items() if k != "DEFAULT_COST"} | {"DEFAULT_COST": "5"}
            r = subprocess.run([sys.executable, str(PLAN), str(d / "cost"), str(d / "names"), "1", str(d / "out")],
                               capture_output=True, text=True, env=env)
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn("shard 1/1: 10.0 s", r.stdout)

    def test_wrong_argument_count_prints_usage(self):
        r = subprocess.run([sys.executable, str(PLAN)], capture_output=True, text=True)
        self.assertEqual(r.returncode, 2)
        self.assertIn("usage: shard-plan.py", r.stderr)


class ValidateUsesPlanTest(unittest.TestCase):
    def test_build_stage_writes_and_test_stage_uses_the_plan(self):
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertIn('uv run --script "$SRC/rust/tools/linux-validate/shard-plan.py"', text)
        self.assertIn("tier2-shards", text)
        self.assertRegex(text, r'shard_args=\(-R "\$\(cat "\$plan"\)"\)')

    def test_sharded_execution_checks_complete_plan_and_rejects_missing_plan(self):
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertIn('uv run "$SRC/rust/tools/linux-validate/check-plan.py"', text)
        self.assertIn('FAIL: missing Tier 2 shard plan', text)


class PlanCoverageTest(unittest.TestCase):
    def check_plan(self, patterns):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            listing = root / "tests.json"
            listing.write_text(json.dumps({"tests": [{"name": name} for name in ("test_a", "test_ab", "test_serial")]}))
            for index, pattern in enumerate(patterns, 1):
                (root / f"{index}-of-2.regex").write_text(pattern + "\n")
            return subprocess.run([sys.executable, str(CHECK),
                                   str(listing), str(root), "2"], capture_output=True, text=True)

    def test_complete_disjoint_plan_passes(self):
        result = self.check_plan(["^(test_a|test_serial)$", "^(test_ab)$"])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("3 tests across 2 disjoint shards", result.stdout)

    def test_omission_overlap_empty_and_missing_shards_fail(self):
        for patterns, diagnostic in [
            (["^(test_a)$", "^(test_ab)$"], "test_serial"),
            (["^(test_a|test_ab|test_serial)$", "^(test_a)$"], "test_a"),
            (["^(test_a|test_ab|test_serial)$", "^()$"], "empty shard"),
            (["^(test_a|test_ab|test_serial)$"], "missing shard"),
            (["[", "^(test_a|test_ab|test_serial)$"], "invalid regex"),
        ]:
            with self.subTest(patterns=patterns):
                result = self.check_plan(patterns)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(diagnostic, result.stderr)

    @unittest.skipUnless(shutil.which("ctest"), "requires CTest")
    def test_real_ctest_filters_partition_registry_and_preserve_serial_property(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            names = ["test_a", "test_ab", "test_serial"]
            (root / "CTestTestfile.cmake").write_text(
                "".join(f'add_test({name} "{sys.executable}" "-c" "pass")\n' for name in names)
                + 'set_tests_properties(test_serial PROPERTIES RUN_SERIAL TRUE)\n'
            )
            def listing(*args):
                result = subprocess.run(["ctest", "--test-dir", str(root), "--show-only=json-v1", *args],
                                        check=True, capture_output=True, text=True)
                return json.loads(result.stdout)
            registry = listing()
            serial = next(test for test in registry["tests"] if test["name"] == "test_serial")
            self.assertIn({"name": "RUN_SERIAL", "value": True}, serial["properties"])
            plans = run_plan({"test_serial": 20}, names, 2)
            selected = []
            for index, group in plans.items():
                pattern = "^(" + "|".join(group) + ")$"
                (root / f"{index}-of-2.regex").write_text(pattern)
                selected.extend(test["name"] for test in listing("-R", pattern)["tests"])
            self.assertEqual(sorted(selected), sorted(names))
            (root / "tests.json").write_text(json.dumps(registry))
            checked = subprocess.run([sys.executable, str(CHECK),
                                      str(root / "tests.json"), str(root), "2"], capture_output=True, text=True)
            self.assertEqual(checked.returncode, 0, checked.stderr)


if __name__ == "__main__":
    unittest.main()
