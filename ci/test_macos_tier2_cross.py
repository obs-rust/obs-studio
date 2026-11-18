#!/usr/bin/env -S uv run --script
"""Guard the opt-in macOS Tier 2 prototype (#5009)."""

from __future__ import annotations

import subprocess
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class MacosTier2CrossTest(unittest.TestCase):
    def test_wrong_target_is_rejected_before_provisioning(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [
                    "cmake",
                    "-S",
                    str(ROOT),
                    "-B",
                    directory,
                    "-DOBS_CROSS_TARGET=win-x64",
                    "-DOBS_CROSS_TIER2=ON",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                "OBS_CROSS_TIER2 requires mac-arm64", result.stdout + result.stderr
            )

    def test_compile_only_remains_default(self):
        source = (ROOT / "cmake/cross/CMakeLists.txt").read_text()
        self.assertRegex(source, r'option\(OBS_CROSS_TIER2\s+"[^"]+"\s+OFF\)')

    def test_environment_alias_reaches_the_toolchain_without_provisioning(self):
        with tempfile.TemporaryDirectory() as directory:
            missing = str(Path(directory) / "missing-toolchain.cmake")
            result = subprocess.run(
                [
                    "cmake",
                    "-S",
                    str(ROOT),
                    "-B",
                    directory,
                    "-DOBS_CROSS_TIER2=ON",
                    f"-DCMAKE_TOOLCHAIN_FILE={missing}",
                ],
                env={**os.environ, "OBS_CROSS_TARGET": "mac-arm64"},
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(result.returncode, 0)
            output = result.stdout + result.stderr
            self.assertNotIn("OBS_CROSS_TIER2 requires mac-arm64", output)
            self.assertIn("missing-toolchain.cmake", output)


if __name__ == "__main__":
    unittest.main()
