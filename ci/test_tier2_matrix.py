"""Runner-capacity regression for #2072: retain all tests on fewer runners."""

import importlib.util
from pathlib import Path
import unittest

from tier2_matrix import matrix

ROOT = Path(__file__).resolve().parent.parent


class Tier2MatrixTest(unittest.TestCase):
    def test_queue_and_pr_run_both_complete_backends(self):
        for event in ("merge_group", "pull_request"):
            with self.subTest(event=event):
                self.assertEqual(matrix(event), {"mode": ["OFF", "ON"], "part": ["all"]})

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


if __name__ == "__main__":
    unittest.main()
