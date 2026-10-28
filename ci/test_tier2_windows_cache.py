#!/usr/bin/env -S uv run --script
"""Tier 2 Windows reuses compiles instead of starting cold (#1038).

    uv run python -m unittest ci/test_tier2_windows_cache.py

Tier 2 Windows spent about 5.5 of its 9.1 min compiling libobs and the
cmocka tests twice from scratch, plus 37 s building cmocka through vcpkg.
The windows-ci-x64 preset already enables ccache (ENABLE_CCACHE) whenever
ccache is on PATH, as the Windows OBS build does; Tier 2 never installed or
restored it. It now does, and vcpkg restores cmocka from a binary cache.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "build-project.yaml"


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name}"
    return m.group(1)


class WindowsCcacheTest(unittest.TestCase):
    def setUp(self):
        self.job = job("rust-tier2-windows")

    def test_restores_and_saves_a_tier2_windows_ccache(self):
        self.assertIn("key: windows-tier2-ccache-${{ github.sha }}", self.job)
        self.assertIn("restore-keys: windows-tier2-ccache-", self.job)
        idx = self.job.index("name: Save Tier 2 ccache")
        save = self.job[idx: self.job.find("\n      - ", idx)]
        self.assertIn("refs/heads/master", save)
        self.assertIn("windows-tier2-ccache-${{ github.sha }}", save)

    def test_installs_the_same_pinned_ccache_as_the_windows_build(self):
        build = job("windows-build")
        version = re.search(r"\$Version = '([0-9.]+)'", build).group(1)
        digest = re.search(r"\$Hash = '([0-9a-f]{64})'", build).group(1)
        self.assertIn(f"$Version = '{version}'", self.job)
        self.assertIn(f"$Hash = '{digest}'", self.job)

    def test_cache_steps_run_on_windows_only(self):
        for name in ("Set Up ccache", "Restore Tier 2 ccache", "Save Tier 2 ccache", "Cache vcpkg Binaries"):
            idx = self.job.index(name)
            step = self.job[idx: self.job.find("\n      - ", idx)]
            self.assertIn("runner.os == 'Windows'", step, name)

    def test_vcpkg_uses_a_binary_cache(self):
        self.assertRegex(self.job, r"VCPKG_BINARY_SOURCES: clear;files,[^\n]+,readwrite")


if __name__ == "__main__":
    unittest.main()
