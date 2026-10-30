#!/usr/bin/env -S uv run --script
"""Tier 2 Linux builds once and fans the tests out over parallel runners (#937).

    uv run python -m unittest ci/test_tier2_fanout.py

Builds are about 1 min of Tier 2; the two ctest runs were 9.4 min. One job
builds one tree holding both libobs binaries (#1065: libobs and
libobs-with-rust; it compares their exports, so a mismatch fails before any
test runs), then hands the tree to two mode jobs that each run the complete CTest
suite against one libobs.
"""

from __future__ import annotations

import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    if not m:
        raise AssertionError(f"no job {name}")
    return m.group(1)


class ValidateStagesTest(unittest.TestCase):
    def setUp(self):
        self.text = VALIDATE.read_text(encoding="utf-8")

    def test_stage_switch_defaults_to_all(self):
        self.assertIn('stage="${OBS_TIER2_STAGE:-all}"', self.text)

    def test_build_stage_packs_the_tree_without_objects(self):
        self.assertIn("OBS_TIER2_PACK", self.text)
        self.assertRegex(self.text, r"tar --zstd -cf[\s\S]{0,200}--exclude='\*\.o'")

    def test_test_stage_runs_a_shard(self):
        self.assertIn("OBS_CTEST_SHARD", self.text)
        self.assertRegex(self.text, r'-I "\$shard_index,,\$shard_count"')

    def test_test_stage_needs_no_rust_toolchain(self):
        """The shard jobs have no soldr; `rustc -V` at the top failed every
        shard with 'rustc: command not found' (master run 38871391393)."""
        line = 'rustc_version=$(cd "$SRC" && rustc -V)'
        idx = self.text.index(line)
        guard = self.text.rfind('if [ "$stage" != test ]; then', 0, idx)
        self.assertNotEqual(guard, -1)
        self.assertNotIn("\nfi\n", self.text[guard:idx])

    def test_bad_stage_is_rejected(self):
        r = subprocess.run(["bash", str(VALIDATE)], env={"OBS_TIER2_STAGE": "nope", "PATH": "/usr/bin:/bin"},
                           capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("OBS_TIER2_STAGE", r.stdout + r.stderr)

    def test_invalid_shard_never_reaches_environment_setup(self):
        for shard in ("0/2", "3/2", "1/0", "garbage", "1/2/3"):
            with self.subTest(shard=shard):
                result = subprocess.run(["bash", str(VALIDATE)],
                                        env={"OBS_TIER2_STAGE": "test", "OBS_CTEST_SHARD": shard,
                                             "PATH": "/usr/bin:/bin"},
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 2)
                self.assertIn("OBS_CTEST_SHARD must be index/count", result.stderr)


class WorkflowTest(unittest.TestCase):
    def test_build_job_builds_one_tree_and_uploads_it(self):
        b = job("rust-linux-tier2-build")
        self.assertIn("OBS_TIER2_STAGE: build", b)
        self.assertIn('OBS_TIER2_MODES: "OFF ON"', b)
        self.assertIn("name: tier2-tree\n", b)
        self.assertNotIn("tier2-tree-OFF", b)

    def test_modes_run_one_complete_suite_each(self):
        """Keep both complete suites on two independent runners."""
        t = job("rust-linux-tier2")
        self.assertIn("needs: [check-event, ci-mode, rust-linux-tier2-build]", t)
        self.assertIn("fail-fast: false", t)
        self.assertIn('mode: ["OFF", "ON"]', t)
        self.assertNotIn("shard:", t)
        self.assertNotIn("OBS_CTEST_SHARD", t)
        self.assertNotIn("matrix.shard", t)
        helper = (ROOT / 'rust/tools/linux-validate/ci-runtime.py').read_text()
        self.assertIn("'OBS_CTEST_JOBS=8'", helper)
        self.assertIn("'OBS_TIER2_STAGE=test'", helper)
        self.assertIn("name: tier2-tree\n", t)
        self.assertIn("--mode '${{ matrix.mode }}'", t)
        self.assertIn("f'OBS_TIER2_MODES={args.mode}'", helper)
        self.assertIn("name: tier2-cost-${{ matrix.mode }}-${{ matrix.part }}\n", t)
        self.assertIn("path: .tier2-cost/ctest-cost-${{ matrix.mode }}.txt", t)
        self.assertNotIn("OBS_TIER2_SHARDS", job("rust-linux-tier2-build"))

    def test_lane_proof_waits_for_both_complete_suites(self):
        # The proof is published after CI Gate, off the critical path (#3018):
        # the gate passes only when every Tier 2 shard passed, and the build
        # job sets the proof name only when it ran for an unproven lane.
        build = job("rust-linux-tier2-build")
        self.assertRegex(build, r"outputs:\n\s+tier2-linux-proof: \$\{\{ needs\.ci-mode\.outputs\.tier2-linux_name \}\}")
        self.assertIn("needs.ci-mode.outputs.tier2-linux_hit != 'true'", build.split("steps:", 1)[0])
        head = WORKFLOW.read_text(encoding="utf-8").split("\njobs:\n", 1)[0]
        self.assertRegex(head, r"tier2-linux-proof:\n(?:\s+description: .*\n)?\s+value: \$\{\{ jobs\.rust-linux-tier2-build\.outputs\.tier2-linux-proof \}\}")
        for name in ("rust-linux-tier2", "rust-linux-tier2-done"):
            self.assertNotIn("Upload Lane Proof", job(name))
        ci = (ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        m = re.search(r"^  tier2-lane-proof:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", ci, re.S | re.M)
        self.assertIsNotNone(m, "ci.yml has no tier2-lane-proof job")
        proof = m.group(1)
        self.assertIn("needs: [build-project, ci-gate]", proof)
        self.assertIn("needs.ci-gate.result == 'success'", proof)
        self.assertIn("needs.build-project.outputs.tier2-linux-proof != ''", proof)
        self.assertIn("name: ${{ needs.build-project.outputs.tier2-linux-proof }}", proof)
        self.assertIn("Upload Lane Proof", proof)


if __name__ == "__main__":
    unittest.main()
