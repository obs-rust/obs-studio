#!/usr/bin/env -S uv run --script
"""Pin the macOS disk-image detach helper against a stale mountpoint (#2728).

`.github/scripts/utils.zsh/create_diskimage` used to detach the staged volume with
`safe_hdiutil detach /Volumes/<name>`, retrying the same argv five times. When the first
attempt failed with `Resource busy`, the partially ejected volume removed the mountpoint and
every later attempt failed while resolving the path (`No such file or directory`), so the
release aborted in `Package OBS-Studio-Rust` (run 40627344990).

The detach therefore lives in its own helper, and this test pins the three properties that
made the failure possible to reintroduce:

1. the detach site calls the volume helper, not the generic argv retry;
2. an already-unmounted volume is success, not an error;
3. the retry targets the device node (which outlives the mountpoint) and ends with `-force`.

The script only runs on macOS, so this is a source-shape pin: it reads the tracked file and
needs no runner.
"""

from __future__ import annotations

import pathlib
import unittest

SCRIPT = (
    pathlib.Path(__file__).resolve().parent.parent
    / ".github"
    / "scripts"
    / "utils.zsh"
    / "create_diskimage"
)


def _helper_body(text: str) -> str:
    """Return the body of `safe_detach_volume` (its first column-0 closing brace)."""
    start = text.index("safe_detach_volume()")
    end = text.index("\n}\n", start)
    return text[start:end]


class MacosDiskImageDetachTest(unittest.TestCase):
    """The detach retry must survive a mountpoint that disappears mid-retry."""

    def setUp(self) -> None:
        self.text = SCRIPT.read_text(encoding="utf-8")
        self.body = _helper_body(self.text)

    def test_detach_site_uses_the_volume_helper(self) -> None:
        self.assertIn("safe_detach_volume /Volumes/${output_name}", self.text)
        self.assertNotIn("\nsafe_hdiutil detach ", self.text)

    def test_already_unmounted_volume_is_success(self) -> None:
        self.assertIn("[[ ! -d ${mount_point} ]]", self.body)
        self.assertIn("return 0", self.body)

    def test_retry_targets_the_device_node(self) -> None:
        self.assertIn("hdiutil info", self.body)
        self.assertIn("${device:-${mount_point}}", self.body)

    def test_force_detach_is_the_last_resort(self) -> None:
        self.assertIn("detach -force", self.body)

    def test_create_and_attach_keep_the_generic_retry(self) -> None:
        self.assertIn("safe_hdiutil create ", self.text)
        self.assertIn("safe_hdiutil attach ", self.text)


if __name__ == "__main__":
    unittest.main()
