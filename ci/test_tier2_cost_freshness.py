#!/usr/bin/env -S uv run --script
"""Tier 2 Linux shards hand their test timings back (#930).

    uv run python -m unittest ci/test_tier2_cost_freshness.py

Since the build/test split (#975) the build job plans the shards from
ctest-cost-<mode>.txt in the Tier 2 ccache, but only shards run ctest and
they kept no cache, so the cost data froze at its last pre-split value and
new or slower tests were planned at the 1 s default. Each shard now uploads
its CTestCostData.txt, the done job merges them per mode (merge_costs.py)
and caches the result on master, and the build job restores it.
"""

from __future__ import annotations

import re
import itertools
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MERGE = ROOT / "rust" / "tools" / "linux-validate" / "merge_costs.py"
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
CHECK = ROOT / "rust" / "tools" / "linux-validate" / "check-costs.py"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name}"
    return m.group(1)


def merge(*files: str) -> str:
    d = Path(tempfile.mkdtemp())
    paths = []
    for i, body in enumerate(files):
        p = d / f"{i}.txt"
        p.write_text(body)
        paths.append(str(p))
    r = subprocess.run([sys.executable, str(MERGE), *paths], capture_output=True, text=True)
    assert r.returncode == 0, r.stdout + r.stderr
    return r.stdout


class MergeCostsTest(unittest.TestCase):
    def test_takes_each_test_from_the_shard_that_ran_it(self):
        # Each shard starts from the same old data (a 3 10, b 3 20, c 3 30)
        # and updates only the tests it ran.
        s1 = "a 4 12\nb 3 20\nc 3 30\n---\n"
        s2 = "a 3 10\nb 4 25\nc 3 30\n---\n"
        s3 = "a 3 10\nb 3 20\nc 4 33\nnew 1 7\n---\nc\n"
        out = merge(s1, s2, s3)
        lines, _, failed = out.partition("---\n")
        self.assertEqual(sorted(lines.splitlines()), ["a 4 12", "b 4 25", "c 4 33", "new 1 7"])
        self.assertEqual(failed.split(), ["c"])

    def test_empty_inputs_give_empty_data(self):
        self.assertEqual(merge("", ""), "---\n")


class WorkflowTest(unittest.TestCase):
    def test_publisher_runs_only_for_successful_suites_with_work(self):
        condition = re.search(r"(?m)^    if: (.+)$", job("rust-linux-tier2-done")).group(1)
        condition = condition.removeprefix("${{ ").removesuffix(" }}")
        for result, name, hit, event, ref in itertools.product(
            ("success", "failure", "cancelled", "skipped"), ("", "proof-key"),
            ("true", "false"), ("pull_request", "merge_group", "push"),
            ("refs/heads/master", "refs/heads/feature")
        ):
            with self.subTest(result=result, name=name, hit=hit, event=event, ref=ref):
                expression = condition
                for field, value in {
                    "needs.rust-linux-tier2.result == 'success'": result == "success",
                    "needs.ci-mode.outputs.tier2-linux_name != ''": name != "",
                    "needs.ci-mode.outputs.tier2-linux_hit != 'true'": hit != "true",
                    "github.event_name == 'push'": event == "push",
                    "github.ref == 'refs/heads/master'": ref == "refs/heads/master",
                }.items():
                    expression = expression.replace(field, str(value))
                expression = expression.replace("&&", " and ").replace("||", " or ")
                self.assertRegex(expression, r"^(?:True|False|and|or|\s|\(|\))+$")
                actual = eval(expression, {"__builtins__": {}})
                # Only the master timing merge runs here; the lane proof is
                # published after CI Gate in ci.yml (#3018).
                work = event == "push" and ref == "refs/heads/master"
                self.assertEqual(actual, result == "success" and work)

    def test_upload_includes_hidden_timings_and_rejects_missing_files(self):
        j = job("rust-linux-tier2")
        upload = j[j.index("- name: Upload Test Timings"):]
        self.assertIn("include-hidden-files: true", upload)
        self.assertIn("if-no-files-found: error", upload)

    def test_cache_save_requires_valid_timings(self):
        j = job("rust-linux-tier2-done")
        self.assertLess(j.index("check-costs.py .tier2-cost"), j.index("uses: ./.github/actions/cache"))

    def test_validate_reads_a_cost_dir(self):
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertIn("OBS_TIER2_COST_DIR", text)

    def test_test_job_uploads_its_costs(self):
        j = job("rust-linux-tier2")
        self.assertIn('--cost "$GITHUB_WORKSPACE/.tier2-cost"', j)
        self.assertIn("name: tier2-cost-${{ matrix.mode }}-${{ matrix.part }}\n", j)
        self.assertIn("path: .tier2-cost/ctest-cost-${{ matrix.mode }}.txt", j)

    def test_shards_start_from_the_merged_costs(self):
        # ctest starts the longest tests first only with cost data; shards
        # ran with an empty dir, so the 59 s and 35 s tests started last and
        # shard 2 took 168 s for 228 s of work at -j4 (run 38892442457).
        j = job("rust-linux-tier2")
        restore = j.index("restore-keys: tier2-ctest-cost-")
        self.assertLess(restore, j.index("- name: Test Tier 2 ("))
        self.assertIn("path: .tier2-cost", j)
        self.assertIn('--cost "$GITHUB_WORKSPACE/.tier2-cost"', j)
        helper = (WORKFLOW.parents[2] / 'rust/tools/linux-validate/ci-runtime.py').read_text()
        self.assertIn("(cost, COST, False)", helper)
        self.assertIn("f'OBS_TIER2_COST_DIR={COST}'", helper)

    def test_done_job_merges_and_caches_on_master(self):
        j = job("rust-linux-tier2-done")
        self.assertIn("pattern: tier2-cost*", j)
        self.assertIn("merge_costs.py", j)
        self.assertRegex(j, r"uses: (?:actions/cache/save@|\./\.github/actions/cache)")
        self.assertIn("key: tier2-ctest-cost-${{ github.sha }}", j)

    def test_merge_handles_single_and_multiple_artifact_layouts(self):
        # download-artifact v8 flattens a single match even when
        # merge-multiple is false. Execute the actual workflow shell.
        block = job("rust-linux-tier2-done")
        script = (
            block.split("      - name: Merge Test Timings", 1)[1]
            .split("        run: |\n", 1)[1]
            .split("      - uses:", 1)[0]
        )
        script = "\n".join(line[10:] for line in script.splitlines())
        import os

        for nested in (False, True):
            with (
                self.subTest(nested=nested),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory)
                destination = root / "tier2-cost-shards"
                sources = (
                    [destination / "tier2-cost-a", destination / "tier2-cost-b"]
                    if nested
                    else [destination]
                )
                for source in sources:
                    source.mkdir(parents=True)
                    for mode in ("OFF", "ON"):
                        (source / f"ctest-cost-{mode}.txt").write_text(
                            "test_one 1 0.2\n---\n"
                        )
                # Resolve repo scripts while retaining an isolated output dir.
                script_here = script.replace("uv run --script rust/", f"{sys.executable} {ROOT}/rust/")
                script_here = script_here.replace("bash rust/", f"bash {ROOT}/rust/")
                result = subprocess.run(
                    ["bash", "-e", "-c", script_here],
                    cwd=root,
                    env={**os.environ, "RUNNER_TEMP": directory},
                    capture_output=True,
                    text=True,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                for mode in ("OFF", "ON"):
                    self.assertIn(
                        "test_one 1 0.2",
                        (root / ".tier2-cost" / f"ctest-cost-{mode}.txt").read_text(),
                    )

    def test_build_job_restores_the_merged_costs(self):
        j = job("rust-linux-tier2-build")
        self.assertIn("restore-keys: tier2-ctest-cost-", j)
        self.assertIn("OBS_TIER2_COST_DIR: ${{ github.workspace }}/.tier2-cost", j)


class CheckCostsTest(unittest.TestCase):
    def check(self, off: str | None, on: str | None) -> subprocess.CompletedProcess:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for mode, body in (("OFF", off), ("ON", on)):
                if body is not None:
                    (root / f"ctest-cost-{mode}.txt").write_text(body)
            return subprocess.run([sys.executable, str(CHECK), directory], capture_output=True, text=True)

    def test_both_modes_have_measured_tests(self):
        result = self.check("test_one 1 0.2\n---\n", "test_one 2 0.3\n---\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("OFF: 1 measured tests", result.stdout)
        self.assertIn("ON: 1 measured tests", result.stdout)
        self.assertIn(f"({len('test_one 1 0.2\n---\n')} bytes)", result.stdout)

    def test_only_measured_rows_before_the_terminator_count(self):
        # ctest's failure list follows "---"; a row needs three fields, a
        # positive integer run count and a numeric time.
        off = (
            "test_one 1 0.2\n"
            "test_two 3 4\n"
            "test_three 2 1e-3\n"
            "test_four 0 5\n"  # never ran
            "test_five 2 0.4 extra\n"  # four fields
            "test_six x 0.5\n"  # count is not a number
            "test_seven 1 fast\n"  # time is not a number
            "---\n"
            "test_eight 1 0.6\n"
        )
        result = self.check(off, "test_one 1 0.2\n---\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("OFF: 3 measured tests", result.stdout)

    def test_empty_or_unmeasured_modes_fail(self):
        for bad in (None, "", "---\n", "test_one 0 0\n---\n"):
            with self.subTest(body=bad):
                result = self.check("test_one 1 0.2\n---\n", bad)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("ON", result.stderr)

    def test_a_missing_directory_argument_prints_usage(self):
        result = subprocess.run([sys.executable, str(CHECK)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage: check-costs.py COST_DIR", result.stderr)


if __name__ == "__main__":
    unittest.main()
