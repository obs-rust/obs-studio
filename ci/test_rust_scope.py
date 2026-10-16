"""Fast gate test scoping (#992).

    uv run python -m unittest ci/test_rust_scope.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

from rust_scope import affected

CRATES = {
    "obs-util": ("rust/obs-util", set()),
    "obs-data": ("rust/obs-data", {"obs-util"}),
    "obs-hotkey": ("rust/obs-hotkey", {"obs-data"}),
    "obs-gpu-status": ("rust/obs-gpu-status", set()),
}
BUILD = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "build-project.yaml"


class AffectedTest(unittest.TestCase):
    def test_crate_change_tests_it_and_dependents(self):
        self.assertEqual(affected(["rust/obs-util/src/dstr.rs"], CRATES), ("packages", ["obs-data", "obs-hotkey", "obs-util"]))
        self.assertEqual(affected(["rust/obs-gpu-status/src/lib.rs"], CRATES), ("packages", ["obs-gpu-status"]))

    def test_shared_inputs_test_the_workspace(self):
        for f in ("libobs/obs-source.c", "Cargo.lock", "dylints/x/src/lib.rs", "rust-toolchain.toml", "rust/stray.txt"):
            self.assertEqual(affected([f], CRATES)[0], "workspace", f)

    def test_unrelated_files_skip(self):
        self.assertEqual(affected(["docs/a.md", "frontend/x.cpp", "ci/y.py"], CRATES), ("skip", []))

    def test_mixed_with_workspace_wins(self):
        self.assertEqual(affected(["rust/obs-util/a.rs", "libobs/a.c"], CRATES)[0], "workspace")


class FastGateWorkflowTest(unittest.TestCase):
    def job(self, name):
        m = re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", BUILD.read_text(), re.S | re.M)
        return m.group(1)

    def test_fast_gate_has_no_qt(self):
        lines = [l for l in self.job("rust-tests-linux").lower().splitlines() if not l.lstrip().startswith("#")]
        self.assertNotIn("qt", "\n".join(lines))

    def test_fast_gate_runs_scoped_tests_on_the_pr_path(self):
        j = self.job("rust-tests-linux")
        self.assertIn("--test-diff", j)
        self.assertIn("Fast Gate", j)


if __name__ == "__main__":
    unittest.main()
