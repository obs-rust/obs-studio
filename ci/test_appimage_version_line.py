#!/usr/bin/env -S uv run --script
"""test-appimage.sh finds the --version line even after loader warnings.

    uv run python -m unittest ci.test_appimage_version_line

Since the glibc 2.28 base (#927) the loader prints "libobs-va.so.2: no version
information available" before --version's output, and the check required the
whole captured output to start with "OBS-Studio-Rust", so every distro test
failed though the app reported "OBS-Studio-Rust 0.1.0 (based on OBS ...)".
"""

from __future__ import annotations

import re
import subprocess
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "build-aux/appimage/test-appimage.sh"
WARNING = (
    "/tmp/appimage_extracted_x/usr/bin/obs-studio-rust-gui: /tmp/appimage_extracted_x/usr/bin/../lib/"
    "libobs-va.so.2: no version information available (required by /tmp/appimage_extracted_x/usr/bin/"
    "../lib/libavutil.so.60)"
)
VERSION = "OBS-Studio-Rust 0.1.0 (based on OBS 33.0.0-beta6-1314-g8b5bb415e)"


def version_line(output: str) -> str:
    source = SCRIPT.read_text()
    match = re.search(r"^app_version_line\(\) \{\n.*?^\}\n", source, re.M | re.S)
    if match is None:
        raise AssertionError("test-appimage.sh defines no app_version_line()")
    result = subprocess.run(
        ["bash", "-c", match.group(0) + 'app_version_line "$1"', "bash", output],
        capture_output=True, text=True, check=True,
    )
    return result.stdout.rstrip("\n")


class AppImageVersionLineTest(unittest.TestCase):
    def test_loader_warning_before_the_version_is_skipped(self):
        self.assertEqual(version_line(f"{WARNING}\n{VERSION}"), VERSION)

    def test_plain_version_output(self):
        self.assertEqual(version_line(VERSION), VERSION)
        self.assertEqual(version_line("OBS Studio 32.1.2"), "OBS Studio 32.1.2")

    def test_no_version_line_gives_nothing(self):
        self.assertEqual(version_line(WARNING), "")

    def test_both_checks_use_the_version_line(self):
        source = SCRIPT.read_text()
        for check in (
            '[[ "$VERSION_LINE" != OBS-Studio-Rust* ]]',
            '[[ "$VERSION_LINE" != "OBS-Studio-Rust $EXPECT_RUST_VERSION (based on OBS "* ]]',
            '[[ "$VERSION_LINE" == OBS-Studio-Rust* ]]',  # Rust paths and identity
        ):
            self.assertTrue(check in source, f"test-appimage.sh lacks {check}")
        # Only the version line decides; the raw output may start with a warning.
        self.assertNotRegex(source, r'\[\[ "\$VERSION_OUT" [!=]= ')


if __name__ == "__main__":
    unittest.main()
