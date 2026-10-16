#!/usr/bin/env -S uv run --script
"""Tier 2 Linux builds once and fans the tests out over parallel runners (#937).

    uv run python -m unittest ci/test_tier2_fanout.py

Builds are about 1 min of Tier 2; the two ctest runs were 9.4 min. One job
now builds both ENABLE_RUST_LIBOBS modes (and compares their exports, so a
mismatch fails before any test runs), then hands each build tree to
`mode x shard` test jobs that run `ctest -I <shard>,,<N>` on it.
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

    def test_bad_stage_is_rejected(self):
        r = subprocess.run(["bash", str(VALIDATE)], env={"OBS_TIER2_STAGE": "nope", "PATH": "/usr/bin:/bin"},
                           capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("OBS_TIER2_STAGE", r.stdout + r.stderr)


class WorkflowTest(unittest.TestCase):
    def test_build_job_builds_both_modes_and_uploads_trees(self):
        b = job("rust-linux-tier2-build")
        self.assertIn("OBS_TIER2_STAGE: build", b)
        self.assertIn('OBS_TIER2_MODES: "OFF ON"', b)
        self.assertIn("name: tier2-tree-OFF", b)
        self.assertIn("name: tier2-tree-ON", b)

    def test_test_jobs_are_a_mode_by_shard_matrix(self):
        t = job("rust-linux-tier2")
        self.assertIn("needs: [check-event, ci-mode, rust-linux-tier2-build]", t)
        self.assertRegex(t, r"mode:\s*\[\s*OFF,\s*ON\s*\]")
        self.assertRegex(t, r"shard:\s*\[\s*1,\s*2,\s*3\s*\]")
        self.assertIn("OBS_TIER2_STAGE: test", t)
        self.assertIn("OBS_CTEST_SHARD: ${{ matrix.shard }}/3", t)
        self.assertIn("name: tier2-tree-${{ matrix.mode }}", t)

    def test_lane_proof_waits_for_every_shard(self):
        d = job("rust-linux-tier2-done")
        self.assertIn("rust-linux-tier2", d)
        self.assertIn("Upload Lane Proof", d)
        self.assertNotIn("Upload Lane Proof", job("rust-linux-tier2"))


if __name__ == "__main__":
    unittest.main()
