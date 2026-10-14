#!/usr/bin/env -S uv run --script
"""Release-step shell pitfalls that only fail on a real release run.

The 2026-10-14 beta (release-rust run 38492393948) built every platform and
then failed every package check:
- Windows: `$Run = if (...) { @('--run') } else { @() }` unwraps the array
  to a string, and `@Run` splatted it as `- - r u n`.
- macOS: `PlistBuddy ... /dev/stdin` cannot read a pipe ("Cannot parse a NULL
  or zero-length data").

    uv run python -m unittest ci/test_release_step_pitfalls.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILES = sorted([*ROOT.glob(".github/workflows/*.y*ml"), *ROOT.glob(".github/actions/*/action.y*ml")])


class ReleaseStepPitfallsTest(unittest.TestCase):
    def test_splatted_arrays_are_not_unwrapped_by_if(self):
        bad = []
        for path in FILES:
            text = path.read_text()
            for m in re.finditer(r"^\s*\$(\w+)\s*=\s*if\s*\(", text, re.M):
                if re.search(r"@" + m.group(1) + r"\b", text):
                    bad.append(f"{path.relative_to(ROOT)}: ${m.group(1)} = if (...) then @{m.group(1)}")
        self.assertEqual(bad, [], "wrap it: $X = @(if (...) { ... })")

    def test_plistbuddy_does_not_read_stdin(self):
        bad = [
            str(p.relative_to(ROOT))
            for p in FILES
            if re.search(r"PlistBuddy[^\n]*/dev/stdin", p.read_text())
        ]
        self.assertEqual(bad, [], "PlistBuddy cannot read a pipe; use plutil -extract ... -o - -")


    def test_release_step_keeps_the_appimage_for_continuous(self):
        # Run 38500530155 published the beta, then "Move the continuous build"
        # found no OBS_Studio-*-x86_64.AppImage: the release step had mv'd it
        # into dist/ under the release name.
        text = (ROOT / ".github" / "workflows" / "release-rust.yaml").read_text()
        self.assertIsNone(re.search(r"^\s*mv appimage/OBS_Studio-", text, re.M), "copy it (cp); continuous uploads it later")


if __name__ == "__main__":
    unittest.main()
