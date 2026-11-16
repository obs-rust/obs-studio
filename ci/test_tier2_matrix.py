"""Runner-capacity regression for #2072: retain all tests on fewer runners."""

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest

from tier2_matrix import matrix

ROOT = Path(__file__).resolve().parent.parent


class Tier2MatrixTest(unittest.TestCase):
    def test_queue_and_pr_run_both_complete_backends(self):
        for event in ("merge_group", "pull_request"):
            with self.subTest(event=event):
                self.assertEqual(matrix(event), {"mode": ["OFF", "ON"], "part": ["all"]})

    def test_queue_cannot_expand_to_redundant_partitions(self):
        self.assertEqual(matrix("merge_group", full=True), {"mode": ["OFF", "ON"], "part": ["all"]})

    def test_background_and_release_keep_every_partition(self):
        for event in ("push", "schedule", "workflow_dispatch", "workflow_call", ""):
            with self.subTest(event=event):
                self.assertEqual(matrix(event), {
                    "mode": ["OFF", "ON"],
                    "part": ["suite", "serial-1-of-2", "serial-2-of-2"],
                })
        self.assertEqual(matrix("pull_request", full=True), matrix("schedule"))

    def test_complete_runtime_explicitly_clears_every_selection(self):
        spec = importlib.util.spec_from_file_location(
            "ci_runtime", ROOT / "rust/tools/linux-validate/ci-runtime.py"
        )
        runtime = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(runtime)
        self.assertEqual(runtime.part_env("all"), [
            "OBS_CTEST_PART=", "OBS_CTEST_SHARD=", "OBS_CTEST_SERIAL_SHARD=",
            "OBS_TIER2_SERIAL_CPUS=",
        ])

    def test_workflow_consumes_python_matrix_without_changing_gate(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertIn("tier2_matrix: ${{ steps.tier2-matrix.outputs.matrix }}", workflow)
        self.assertIn("matrix: ${{ fromJSON(needs.check-event.outputs.tier2_matrix) }}", workflow)
        self.assertIn("run: uv run --no-project ci/tier2_matrix.py", workflow)

    def test_actions_output_is_a_parseable_matrix(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "output"
            subprocess.run([
                "uv", "run", "--no-project", str(ROOT / "ci/tier2_matrix.py"),
            ], check=True, capture_output=True, env={
                **os.environ, "GITHUB_OUTPUT": str(output),
                "GITHUB_EVENT_NAME": "merge_group", "TIER2_FULL": "false",
            })
            self.assertEqual(json.loads(output.read_text().removeprefix("matrix=")), matrix("merge_group"))

    def test_scheduling_changes_invalidate_cached_proofs(self):
        from lane_proof import SURFACES

        self.assertIn("ci/tier2_matrix.py", SURFACES)
        trust = tomllib.loads((ROOT / "local-gate.toml").read_text())["gate"]["trust"]
        self.assertIn("ci/tier2_matrix.py", trust["surfaces"])


if __name__ == "__main__":
    unittest.main()
