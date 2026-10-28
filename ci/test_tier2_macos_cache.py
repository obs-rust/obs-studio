#!/usr/bin/env -S uv run --script
"""Tier 2 macOS reuses master's Xcode compilation cache (#930).

    uv run python -m unittest ci/test_tier2_macos_cache.py

rust/tools/tier2-validate/validate.sh already builds through the macos-ci
preset's Xcode compilation cache (XCODE_CAS_PATH), but the CAS started empty
every run: on queue runs the two cold builds took about 6.5 of the job's
10 min on a macOS runner (run 38908431573). Like the macOS OBS build, non-
master runs restore master's CAS; master builds cold and saves, so the CAS
holds only what the current tree uses (it cannot be pruned by age).
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
VALIDATE = ROOT / "rust" / "tools" / "tier2-validate" / "validate.sh"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name}"
    return m.group(1)


def step(j: str, marker: str) -> str:
    i = j.index(marker)
    start = j.rfind("\n      - ", 0, i) + 1
    end = j.find("\n      - ", i)
    return j[start:end if end != -1 else None]


class MacosTier2CacheTest(unittest.TestCase):
    def setUp(self):
        self.job = job("rust-tier2-macos")

    def test_validate_takes_the_cas_path_from_the_environment(self):
        self.assertIn('XCODE_CAS_PATH="${XCODE_CAS_PATH:-', VALIDATE.read_text(encoding="utf-8"))

    def test_non_master_runs_restore_before_the_build(self):
        restore = step(self.job, "key: macos-tier2-xcas-${{ github.sha }}")
        self.assertRegex(restore, r"uses: (?:actions/cache/restore@|\./\.github/actions/cache)")
        self.assertIn("runner.os == 'macOS'", restore)
        self.assertIn("refs/heads/master", restore)
        self.assertIn("restore-keys: macos-tier2-xcas-", restore)
        self.assertLess(self.job.index("macos-tier2-xcas-"), self.job.index("Validate Tier 2 on macOS"))

    def test_build_uses_the_restored_path(self):
        build = step(self.job, "Validate Tier 2 on macOS")
        self.assertIn("XCODE_CAS_PATH: ${{ github.workspace }}/.tier2-xcas", build)

    def test_only_master_saves(self):
        save = step(self.job, "Save Tier 2 Xcode Cache")
        self.assertRegex(save, r"uses: (?:actions/cache/save@|\./\.github/actions/cache)")
        self.assertIn("runner.os == 'macOS'", save)
        self.assertIn("github.ref == 'refs/heads/master'", save)
        # Scheduled and dispatched master runs too: master pushes run macOS
        # Tier 2 only when their paths select it, so a push-only save rarely ran.
        self.assertIn("github.event_name != 'pull_request'", save)
        restore = step(self.job, "Restore Tier 2 Xcode Cache")
        self.assertIn("!(github.event_name != 'pull_request' && github.ref == 'refs/heads/master')", restore)
        self.assertIn("path: ${{ github.workspace }}/.tier2-xcas", save)


if __name__ == "__main__":
    unittest.main()
