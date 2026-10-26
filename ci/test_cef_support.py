#!/usr/bin/env -S uv run --script
"""A pinned CEF whose Chromium is older than extended stable fails (#2380).

CEF 7871 (Chromium 150) ran past its end of extended-stable support with two
in-the-wild V8 CVEs before anyone noticed (#1092). ci/cef_support.py compares
every pinned CEF branch with Chrome's extended-stable branch; the scheduled
workflow runs it daily, since the release data changes without a commit here.

    uv run python -m unittest ci/test_cef_support.py
"""

import json
import unittest
from pathlib import Path


import cef_support

FIXTURE = json.loads((Path(__file__).resolve().parent / "fixtures" / "chrome-extended-versions.json").read_text())


class CefSupport(unittest.TestCase):
    def test_extended_stable_branch_from_the_release_data(self):
        self.assertEqual(cef_support.extended_stable(FIXTURE), (156, 8078))

    def test_an_outdated_pin_fails_and_names_the_platform(self):
        failures = cef_support.check({"windows-x64": 7871, "ubuntu-x86_64": 8078}, (156, 8078))
        self.assertEqual(len(failures), 1)
        self.assertIn("windows-x64", failures[0])
        self.assertIn("7871", failures[0])

    def test_current_pins_pass(self):
        self.assertEqual(cef_support.check({"ubuntu-x86_64": 8078, "flatpak": 8078}, (156, 8078)), [])

    def test_reads_every_pin_in_the_tree(self):
        pins = cef_support.pins()
        for platform in ("windows-x64", "windows-arm64", "macos-x86_64", "macos-arm64", "ubuntu-x86_64", "flatpak"):
            self.assertIn(platform, pins)
        # CMakePresets.json overrides one platform, and the Flatpak pins its own.
        self.assertNotEqual(pins["ubuntu-x86_64"], pins["windows-x64"])

    def test_a_rebuild_release_keeps_its_branch(self):
        # A rebuild of a branch is published as <branch>-r<n> (8078-r2, #2363);
        # its Flatpak pin must still count, not drop out of the check.
        url = "https://github.com/obs-rust/cef/releases/download/{}/cef_binary_8078_linux_x86_64.tar.xz"
        self.assertEqual(cef_support.flatpak_branch(url.format("8078")), 8078)
        self.assertEqual(cef_support.flatpak_branch(url.format("8078-r2")), 8078)
        self.assertIsNone(cef_support.flatpak_branch("no cef url here"))


if __name__ == "__main__":
    unittest.main()
