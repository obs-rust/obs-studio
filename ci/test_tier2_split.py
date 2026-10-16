#!/usr/bin/env -S uv run --script
"""Tier 2 Linux runs ENABLE_RUST_LIBOBS=OFF and =ON as parallel jobs (#930 A2).

    uv run python -m unittest ci/test_tier2_split.py

validate.sh built and tested OFF, then ON, in one job: about 13 min, nearly
all of it two back-to-back ctest runs. Each mode now runs in its own matrix
job; a small job compares the two exported-symbol lists and publishes the
lane proof once both pass. The local gate still runs both modes in one go.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
RUN = ROOT / "rust" / "tools" / "linux-validate" / "run.sh"


def job(text: str, name: str) -> str:
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    if not m:
        raise AssertionError(f"no job {name}")
    return m.group(1)


class WorkflowTest(unittest.TestCase):
    def setUp(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        self.tier2 = job(text, "rust-linux-tier2")
        self.exports = job(text, "rust-linux-tier2-exports")

    def test_modes_run_as_a_matrix(self):
        self.assertRegex(self.tier2, r"mode:\s*\[\s*OFF,\s*ON\s*\]")
        self.assertIn("fail-fast: false", self.tier2)
        self.assertIn("OBS_TIER2_MODES: ${{ matrix.mode }}", self.tier2)

    def test_each_mode_keeps_its_own_ccache(self):
        self.assertIn("tier2-ccache-linux-${{ matrix.mode }}-${{ github.sha }}", self.tier2)

    def test_each_mode_uploads_its_exports(self):
        self.assertIn("name: tier2-exports-${{ matrix.mode }}", self.tier2)

    def test_exports_job_compares_both_lists(self):
        self.assertIn("needs: [check-event, ci-mode, rust-linux-tier2]", self.exports)
        self.assertIn("pattern: tier2-exports-*", self.exports)
        self.assertRegex(self.exports, r"diff\s+\S*exports-OFF\.txt\s+\S*exports-ON\.txt")

    def test_lane_proof_is_published_only_after_both_modes(self):
        self.assertNotIn("Upload Lane Proof", self.tier2)
        self.assertIn("Upload Lane Proof", self.exports)


class HarnessTest(unittest.TestCase):
    def test_validate_runs_the_requested_modes(self):
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertIn('modes="${OBS_TIER2_MODES:-OFF ON}"', text)
        self.assertIn("for mode in $modes; do", text)

    def test_validate_writes_exports_for_the_host(self):
        self.assertIn("/exports", VALIDATE.read_text(encoding="utf-8"))

    def test_run_passes_modes_and_exports_dir(self):
        text = RUN.read_text(encoding="utf-8")
        self.assertIn("OBS_TIER2_MODES", text)
        self.assertIn("OBS_TIER2_EXPORTS_DIR", text)


if __name__ == "__main__":
    unittest.main()
