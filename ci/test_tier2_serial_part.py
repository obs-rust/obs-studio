"""Tier 2 runs its RUN_SERIAL tests on their own runner (#2071).

Usage:
    uv run python -m unittest ci/test_tier2_serial_part.py

test_deinterlace_2x_field_order is RUN_SERIAL (#1343, #1643): concurrent CI
dropped fields. Inside the one ctest run per mode it ran alone for 146 of the
238 s while the runner's other cores sat idle. Each mode now runs two parts
on separate runners: "suite" (every test but the RUN_SERIAL ones, in
parallel) and "serial" (only those, so they keep a runner to themselves).
The tests and their assertions are unchanged.
"""

import importlib.util
import json
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "rust/tools/linux-validate"
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"


def test_job() -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    return re.search(r"^  rust-linux-tier2:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)[1]


def registry(*tests: tuple[str, bool]) -> dict:
    return {"tests": [{"name": name, "properties": [{"name": "RUN_SERIAL", "value": True}] if serial else []}
                      for name, serial in tests]}


class SerialTestsTest(unittest.TestCase):
    def select(self, data: dict, shard: str = "", companions: dict | None = None) -> str:
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "registry.json"
            path.write_text(json.dumps(data))
            extra = []
            if companions is not None:
                extra = ["--companions", str(Path(tmp) / "companions.json")]
                Path(extra[1]).write_text(json.dumps(companions))
            return subprocess.run(["uv", "run", "--no-project", str(TOOLS / "serial-tests.py"), str(path),
                                   *([shard] if shard else []), *extra],
                                  check=True, capture_output=True, text=True).stdout.strip()

    def test_regex_matches_exactly_the_run_serial_tests(self):
        regex = self.select(registry(("test_a", False), ("test_deinterlace_2x_field_order", True),
                                     ("test_xshm_resolution_change", True), ("test_deinterlace_2x", False)))
        self.assertRegex("test_deinterlace_2x_field_order", regex)
        self.assertRegex("test_xshm_resolution_change", regex)
        self.assertNotRegex("test_a", regex)
        self.assertNotRegex("test_deinterlace_2x", regex)

    def test_shards_split_the_serial_tests_round_robin(self):
        data = registry(*((f"test_d_{i}", True) for i in range(5)), ("test_a", False))
        first, second = self.select(data, "1/2"), self.select(data, "2/2")
        names = [f"test_d_{i}" for i in range(5)]
        self.assertEqual([n for n in names if re.search(first, n)], ["test_d_0", "test_d_2", "test_d_4"])
        self.assertEqual([n for n in names if re.search(second, n)], ["test_d_1", "test_d_3"])

    def test_no_serial_tests_is_an_error(self):
        # An empty selection would make the serial part pass with no tests.
        with self.assertRaises(subprocess.CalledProcessError):
            self.select(registry(("test_a", False)))


class CompanionsTest(unittest.TestCase):
    """The serial runners also take a fixed set of suite tests (#3021): the
    suite ran ~1000 test-seconds at -j8 (124-128 s) while the serial shards
    idled after 47-71 s. ctest never runs them beside a RUN_SERIAL test."""

    COMPANIONS = {"shards": 2, "companions": [["test_c"], ["test_b", "test_gone"]]}

    def data(self) -> dict:
        return registry(("test_a", False), ("test_b", False), ("test_c", False),
                        *((f"test_d_{i}", True) for i in range(3)))

    def matched(self, regex: str) -> list[str]:
        return [t["name"] for t in self.data()["tests"] if re.search(regex, t["name"])]

    def test_every_test_runs_exactly_once_per_mode(self):
        select = SerialTestsTest.select
        suite = select(self, self.data(), companions=self.COMPANIONS)  # the suite's -E set
        first = select(self, self.data(), "1/2", self.COMPANIONS)
        second = select(self, self.data(), "2/2", self.COMPANIONS)
        names = [t["name"] for t in self.data()["tests"]]
        excluded = self.matched(suite)
        runs = [n for n in names if n not in excluded] + self.matched(first) + self.matched(second)
        self.assertEqual(sorted(runs), sorted(names))
        self.assertEqual(self.matched(first), ["test_c", "test_d_0", "test_d_2"])
        self.assertEqual(self.matched(second), ["test_b", "test_d_1"])

    def test_shard_count_must_match_the_companions_file(self):
        with self.assertRaises(subprocess.CalledProcessError):
            SerialTestsTest.select(self, self.data(), "1/3", self.COMPANIONS)

    def test_checked_in_companions_fit_the_workflow_matrix(self):
        companions = json.loads((TOOLS / "serial-companions.json").read_text(encoding="utf-8"))
        n = companions["shards"]
        self.assertEqual(len(companions["companions"]), n)
        parts = [f"serial-{i}-of-{n}" for i in range(1, n + 1)]
        self.assertIn('part: ["suite", ' + ", ".join(f'"{p}"' for p in parts) + "]", test_job())
        names = [name for shard in companions["companions"] for name in shard]
        self.assertEqual(len(names), len(set(names)))
        # Stays in the suite: it alone takes ~102 s and would run after the
        # serial tests on a serial runner (47 + 102 s).
        self.assertNotIn("test_slideshow_mem_cap", names)

    def test_validate_passes_the_companions_to_both_parts(self):
        text = (TOOLS / "validate.sh").read_text(encoding="utf-8")
        self.assertIn("--companions", text[text.index("select_part()"):text.index("select_part()") + 900])


class ValidateTest(unittest.TestCase):
    def test_parts_select_with_exclude_and_include(self):
        text = (TOOLS / "validate.sh").read_text(encoding="utf-8")
        self.assertIn("OBS_CTEST_PART", text)
        self.assertIn("serial-tests.py", text)
        self.assertRegex(text, r"suite\)[^;]*-E ")
        self.assertRegex(text, r"serial\)[^;]*-R ")
        self.assertIn("OBS_CTEST_SERIAL_SHARD", text)


class RuntimeTest(unittest.TestCase):
    def test_ci_runtime_takes_the_part(self):
        spec = importlib.util.spec_from_file_location("ci_runtime", TOOLS / "ci-runtime.py")
        runtime = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runtime)
        text = (TOOLS / "ci-runtime.py").read_text(encoding="utf-8")
        self.assertIn("OBS_CTEST_PART=", text)
        self.assertIn("OBS_CTEST_SERIAL_SHARD=", text)
        self.assertEqual(runtime.part_env("suite"), ["OBS_CTEST_PART=suite"])
        self.assertEqual(runtime.part_env("serial-2-of-2"), ["OBS_CTEST_PART=serial", "OBS_CTEST_SERIAL_SHARD=2/2"])


class WorkflowTest(unittest.TestCase):
    def test_each_mode_runs_both_parts_on_separate_runners(self):
        job = test_job()
        self.assertRegex(job, r'mode: \["OFF", "ON"\]')
        self.assertRegex(job, r'part: \["suite", "serial-1-of-2", "serial-2-of-2"\]')
        self.assertIn("--part '${{ matrix.part }}'", job)
        # Two parts of one mode write timing artifacts; the names must differ.
        self.assertIn("name: tier2-cost-${{ matrix.mode }}-${{ matrix.part }}", job)
        self.assertIn("name: tier2-runtime-${{ matrix.mode }}-${{ matrix.part }}", job)


if __name__ == "__main__":
    unittest.main()
