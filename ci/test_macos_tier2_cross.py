#!/usr/bin/env -S uv run --script
"""Guard the opt-in macOS Tier 2 prototype (#5009)."""
from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class MacosTier2CrossTest(unittest.TestCase):
    def test_wrong_target_is_rejected_before_provisioning(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                ["cmake", "-S", str(ROOT / "cmake/cross"), "-B", directory,
                 "-DOBS_CROSS_TARGET=win-x64", "-DOBS_CROSS_TIER2=ON"],
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("OBS_CROSS_TIER2 requires mac-arm64", result.stdout + result.stderr)

    def test_compile_only_remains_default(self):
        source = (ROOT / "cmake/cross/CMakeLists.txt").read_text()
        self.assertRegex(source, r'option\(OBS_CROSS_TIER2\s+"[^"]+"\s+OFF\)')


if __name__ == "__main__":
    unittest.main()
