"""Fast gate test scoping (#992).

    uv run python -m unittest ci/test_rust_scope.py
"""

from __future__ import annotations

import re
import tomllib
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
    def test_amalgamated_suite_owners_are_selected_for_borrowed_tests(self):
        root = BUILD.parent.parent.parent
        crates = {}
        for manifest in sorted((root / "rust").glob("*/Cargo.toml")):
            data = tomllib.loads(manifest.read_text())
            deps = set(data.get("dependencies", {})) | set(data.get("dev-dependencies", {}))
            crates[data["package"]["name"]] = (str(manifest.parent.relative_to(root)), deps)
        for component, owner in (
            ("obs-properties", "obs-data"), ("obs-hotkey", "obs-data"),
            ("obs-codec", "obs-nvenc"), ("obs-c-oracle", "obs-util"),
            ("obs-build", "obs-studio-rust"), ("obs-sys", "obs-studio-rust"),
        ):
            with self.subTest(component=component):
                mode, packages = affected([f"rust/{component}/tests/changed.rs"], crates)
                self.assertEqual(mode, "packages")
                self.assertIn(owner, packages)

    def test_crate_change_tests_it_and_dependents(self):
        self.assertEqual(affected(["rust/obs-util/src/dstr.rs"], CRATES), ("packages", ["obs-data", "obs-hotkey", "obs-util"]))
        self.assertEqual(affected(["rust/obs-gpu-status/src/lib.rs"], CRATES), ("packages", ["obs-gpu-status"]))

    def test_shared_inputs_test_the_workspace(self):
        for f in ("libobs/obs-source.c.h", "Cargo.lock", "dylints/x/src/lib.rs", "rust-toolchain.toml", "rust/stray.txt",
                  "ci/local_gate.py", "ci/nextest_tool.py"):
            self.assertEqual(affected([f], CRATES)[0], "workspace", f)

    def test_native_test_graph_inputs_test_obs_build(self):
        # obs-build's graph contract counts test/cmocka targets (#1891).
        crates = CRATES | {
            "obs-build": ("rust/obs-build", set()),
            "obs-studio-rust": ("rust/obs-studio-rust", {"obs-build"}),
        }
        for f in ("test/cmocka/build/test-graph.json", "test/cmocka/test_qsv_probe.c"):
            self.assertEqual(affected([f], crates), ("packages", ["obs-build", "obs-studio-rust"]), f)

    def test_unrelated_files_skip(self):
        self.assertEqual(affected(["docs/a.md", "frontend/x.cpp", "ci/y.py"], CRATES), ("skip", []))

    def test_mixed_with_workspace_wins(self):
        self.assertEqual(affected(["rust/obs-util/a.rs", "libobs/a.c"], CRATES)[0], "workspace")


class FastGateWorkflowTest(unittest.TestCase):
    def job(self, name):
        m = re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", BUILD.read_text(), re.S | re.M)
        return m.group(1)

    def test_fast_gate_runs_no_qt_lane(self):
        # Distro Qt is installed only so CMake configure passes (frontend-tools is a
        # core module with find_package(Qt6 REQUIRED)); the gate never fetches the
        # prebuilt Qt, builds the frontend or runs the Qt-linked tests.
        j = self.job("rust-tests-linux").lower()
        for qt_lane in ("linux_qt_frontend", "fetch_libs_asset", "qt6-6.11", "-denable_frontend=on"):
            self.assertNotIn(qt_lane, j)
        self.assertIn("-denable_decklink=off", j)  # decklink-captions requires Qt6 Widgets
        self.assertIn("-denable_frontend=off", j)

    def test_costly_lanes_overlap_the_fast_gate(self):
        # Tier 2 compilation, the independent Qt lane and the macOS/Windows
        # Rust lanes overlap Fast Gate; their run-level failures are still
        # caught by CI Gate (ci/test_platform_lane_overlap.py). The cross lint
        # lane stays serialized: it runs on the gate's own Linux pool, so
        # starting it early saves no wall time.
        def needs_of(name):
            match = re.search(r"^    needs: \[([^]]+)\]", self.job(name), re.M)
            self.assertIsNotNone(match, name)
            assert match is not None
            return [n.strip() for n in match[1].split(",")]

        for name in ("rust-tests-macos", "rust-tests-windows", "rust-tier2-macos", "rust-tier2-windows"):
            self.assertNotIn("rust-tests-linux", needs_of(name), name)
        self.assertIn("rust-tests-linux", needs_of("rust-cross-lint"))
        self.assertIn("needs.rust-tests-linux.result != 'failure'", self.job("rust-cross-lint"))

    def test_fast_gate_builds_only_touched_linux_targets(self):
        j = self.job("rust-tests-linux")
        self.assertIn("needs.check-event.outputs.linux_targets", j)
        self.assertIn("-DENABLE_FRONTEND=OFF", j)

    def test_fast_gate_runs_scoped_tests_on_the_pr_path(self):
        j = self.job("rust-tests-linux")
        self.assertIn("--test-diff", j)
        self.assertIn("Fast Gate", j)


if __name__ == "__main__":
    unittest.main()
