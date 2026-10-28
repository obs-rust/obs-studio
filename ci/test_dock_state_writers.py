#!/usr/bin/env -S uv run --script
"""The dock layout is persisted only through the clamp-safe accessor (#2225).

Usage:
    uv run python -m unittest ci/test_dock_state_writers.py

`OBSBasic::saveAll()` used to write whatever `QMainWindow::saveState()`
returned, so a session that was narrower than the docks' minimums saved every
dock at its minimum and the next start restored that clamp at full width.
#2727 fixed the global writer (`BasicWindow/DockState`) by having the
`DockProportions` helper hand out the last layout it saw with no dock at its
minimum, `OBSBasic::DockStateToSave()`.

The browser auth panels write and restore a whole main-window snapshot of
their own (`<service>/DockState`) from `Auth::Save()`, which runs inside
`saveAll()` before the global write. Those calls still used the live
`saveState()`, so the same clamp was persisted for anyone with a Twitch,
Restream or YouTube dock and restored over the good layout on the next start:
the same lost layout by the other writer.

This pins the rule rather than the helper: every writer of a dock-state
snapshot goes through `OBSBasic::DockStateToSave()`, and no frontend source
writes a dock state from the live `saveState()`.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OBS_BASIC_HPP = ROOT / "frontend" / "widgets" / "OBSBasic.hpp"
OBS_BASIC = ROOT / "frontend" / "widgets" / "OBSBasic.cpp.hpp"
OAUTH = ROOT / "frontend" / "oauth"

# The three browser auth panels that persist a whole-window dock snapshot in
# their own config section (`<service>/DockState`).
OAUTH_WRITERS = (
    OAUTH / "TwitchAuth.cpp.hpp",
    OAUTH / "RestreamAuth.cpp.hpp",
    OAUTH / "YoutubeAuth.cpp.hpp",
)

# `main->saveState()` / `App()->saveState()`: a live layout, clamp included.
LIVE_SAVE = re.compile(r"(?:main|this|App\(\))->saveState\(\)")


class DockStateWritersTest(unittest.TestCase):
    def test_obsbasic_declares_the_clamp_safe_accessor(self):
        """`OBSBasic::DockStateToSave()` is the one accessor writers use."""
        declaration = OBS_BASIC_HPP.read_text(encoding="utf-8")
        self.assertTrue(
            "QByteArray DockStateToSave() const;" in declaration,
            "OBSBasic does not declare DockStateToSave()",
        )

    def test_obsbasic_persists_the_clamp_safe_state(self):
        """The global `BasicWindow/DockState` writer uses the accessor."""
        source = OBS_BASIC.read_text(encoding="utf-8")
        self.assertTrue(
            '"DockState", dockState.toBase64()' in source,
            "saveAll() does not write the clamp-safe dock state",
        )
        self.assertTrue(
            '"DockState", saveState().toBase64()' not in source,
            "saveAll() still writes the live dock state",
        )

    def test_auth_panels_do_not_write_a_live_snapshot(self):
        """The OAuth `<service>/DockState` writer uses the accessor too."""
        for path in OAUTH_WRITERS:
            source = path.read_text(encoding="utf-8")
            self.assertTrue(
                "DockStateToSave()" in source,
                f"{path.name} does not use DockStateToSave()",
            )
            writes = [
                line.strip()
                for line in source.splitlines()
                if '"DockState"' in line and LIVE_SAVE.search(line)
            ]
            self.assertEqual(writes, [], f"{path.name} writes a live snapshot")


if __name__ == "__main__":
    unittest.main()
