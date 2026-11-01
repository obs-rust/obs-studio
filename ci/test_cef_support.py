#!/usr/bin/env -S uv run --script
"""A pinned CEF whose Chromium is older than extended stable fails (#2380).

CEF 7871 (Chromium 150) ran past its end of extended-stable support with two
in-the-wild V8 CVEs before anyone noticed (#1092). ci/cef_support.py compares
every pinned CEF branch with Chrome's extended-stable branch; the scheduled
workflow runs it daily, since the release data changes without a commit here.

    uv run python -m unittest ci/test_cef_support.py
"""

import json
import re
import unittest
from pathlib import Path


import cef_support

FIXTURE = json.loads(
    (
        Path(__file__).resolve().parent / "fixtures" / "chrome-extended-versions.json"
    ).read_text()
)
WORKFLOW = (
    Path(__file__).resolve().parent.parent / ".github" / "workflows" / "scheduled.yaml"
)


def cef_support_job(text: str) -> str:
    """The `cef-support` job of scheduled.yaml, without its neighbours."""
    match = re.search(r"^  cef-support:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.S | re.M)
    assert match, "scheduled.yaml has no cef-support job"
    return match.group(1)


class CefSupport(unittest.TestCase):
    def test_the_job_does_not_fail_the_nightly_run(self):
        """A red run marks every code scanning configuration "Workflow runs failing" (#3144)."""
        self.assertIn(
            "\n    continue-on-error: true\n",
            cef_support_job(WORKFLOW.read_text(encoding="utf-8")),
        )

    def test_the_continue_on_error_line_says_why(self):
        """The reminder for #2380 stays visible, and the line is removed when #1864 lands."""
        job = cef_support_job(WORKFLOW.read_text(encoding="utf-8"))
        before = job.split("continue-on-error:", 1)[0]
        self.assertIn("#2380", before)
        self.assertIn("#1864", before)

    def test_extended_stable_branch_from_the_release_data(self):
        self.assertEqual(cef_support.extended_stable(FIXTURE), (156, 8078))

    def test_an_outdated_pin_fails_when_the_mirror_has_a_supported_build(self):
        failures, warnings = cef_support.check(
            {"macos-arm64": 7871, "ubuntu-x86_64": 8078},
            (156, 8078),
            {"macos_arm64": {7871, 8078}},
        )
        self.assertEqual(warnings, [])
        self.assertEqual(len(failures), 1)
        self.assertIn("macos-arm64", failures[0])
        self.assertIn("7871", failures[0])

    def test_an_outdated_pin_only_warns_while_the_mirror_lacks_a_supported_build(self):
        # Windows and Linux aarch64 have no 8078 build host yet (#1864): no pin
        # change can fix them, so they warn instead of failing every night.
        mirror = {"windows_x64": {7871}, "macos_arm64": {8078}}
        failures, warnings = cef_support.check(
            {"windows-x64": 7871, "windows-arm64": 7871}, (156, 8078), mirror
        )
        self.assertEqual(failures, [])
        self.assertEqual(len(warnings), 2)
        self.assertIn("windows-x64", warnings[1])
        # The same lagging pin fails as soon as the mirror publishes the build.
        mirror["windows_x64"].add(8078)
        failures, _ = cef_support.check({"windows-x64": 7871}, (156, 8078), mirror)
        self.assertEqual(len(failures), 1)

    def test_current_pins_pass(self):
        self.assertEqual(
            cef_support.check(
                {"ubuntu-x86_64": 8078, "flatpak": 8078}, (156, 8078), {}
            ),
            ([], []),
        )

    def test_published_reads_branches_per_platform_from_the_mirror(self):
        releases = [
            {
                "assets": [
                    {"name": "cef_binary_8078_linux_x86_64.tar.xz"},
                    {"name": "cef_binary_8078_macos_arm64.tar.xz"},
                ]
            },
            {
                "assets": [
                    {"name": "cef_binary_7871_windows_x64.zip"},
                    {"name": "notes.txt"},
                ]
            },
        ]
        self.assertEqual(
            cef_support.published(releases),
            {"linux_x86_64": {8078}, "macos_arm64": {8078}, "windows_x64": {7871}},
        )

    def test_every_pinned_platform_maps_to_a_mirror_archive(self):
        # An unmapped platform would never find a mirror build and only warn.
        self.assertLessEqual(set(cef_support.pins()), set(cef_support.ASSET_PLATFORMS))

    def test_reads_every_pin_in_the_tree(self):
        pins = cef_support.pins()
        for platform in (
            "windows-x64",
            "windows-arm64",
            "macos-x86_64",
            "macos-arm64",
            "ubuntu-x86_64",
            "flatpak",
        ):
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
