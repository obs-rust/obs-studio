#!/usr/bin/env -S uv run --script
"""The dock layout is loaded and saved only through `DockLayout` (#2225, #5455).

Usage:
    uv run python -m unittest ci/test_dock_state_writers.py

`OBSBasic::saveAll()` used to write whatever `QMainWindow::saveState()`
returned, so a session that was narrower than the docks' minimums saved every
dock at its minimum and the next start restored that clamp at full width
(#2225). The browser auth panels (Twitch, Restream, YouTube) wrote and
restored a whole main-window snapshot of their own (`<service>/DockState`) the
same way, and every restore had to be followed by a second call so that the
layout model knew it (#5230).

Since #5455, `DockLayout::Load(config, section)` restores `<section>/DockState`
and starts the model with `<section>/DockSizes`, and `DockLayout::Save()`
writes what the model selects (the restored layout while a clamped start has
changed nothing) with its sizes. This pins the rule rather than the helper:

- no frontend source but `DockLayout` writes `DockState` or `DockSizes`;
- no frontend source but `DockLayout` restores a dock layout of the main
  window (Reset Docks is `DockLayout::Reset()`, which restores the default
  layout from `SetDefault()` and sizes it, #5462);
- the start, `saveAll()` and the three auth panels use `Load()` and `Save()`,
  Reset Docks uses `SetDefault()` and `Reset()`.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRONTEND = ROOT / "frontend"
DOCK_LAYOUT = FRONTEND / "utility" / "DockLayout.cpp.hpp"
OBS_BASIC_HPP = FRONTEND / "widgets" / "OBSBasic.hpp"
OBS_BASIC = FRONTEND / "widgets" / "OBSBasic.cpp.hpp"
OBS_BASIC_DOCKS = FRONTEND / "widgets" / "OBSBasic_Docks.cpp.hpp"
OAUTH = FRONTEND / "oauth"

# The three browser auth panels that keep a whole-window dock layout in their
# own config section (`<service>/DockState`).
OAUTH_PANELS = (
    OAUTH / "TwitchAuth.cpp.hpp",
    OAUTH / "RestreamAuth.cpp.hpp",
    OAUTH / "YoutubeAuth.cpp.hpp",
)

# A config write of a dock layout key.
WRITE = re.compile(r"config_set_string\([^;]*\"Dock(State|Sizes)\"")
# A restore of a dock layout.
RESTORE = re.compile(r"\brestoreState\(")


def frontend_sources():
    for path in sorted(FRONTEND.rglob("*")):
        if path.suffix in (".hpp", ".cpp", ".h") and path != DOCK_LAYOUT:
            yield path


class DockStateWritersTest(unittest.TestCase):
    def test_only_dock_layout_writes_a_dock_state(self):
        """Every `DockState`/`DockSizes` write is `DockLayout::Save()`."""
        writes = [
            f"{path.relative_to(ROOT)}: {line.strip()}"
            for path in frontend_sources()
            for line in path.read_text(encoding="utf-8").splitlines()
            if WRITE.search(line)
        ]
        self.assertEqual(writes, [], "dock layout written outside DockLayout")
        self.assertTrue(
            '"DockState", blob.toBase64()' in DOCK_LAYOUT.read_text(encoding="utf-8"),
            "DockLayout::Save() does not write DockState",
        )

    def test_only_dock_layout_restores_a_dock_state(self):
        """Every restore of the main window's saved layout is `DockLayout::Load()`.

        Other windows restore their own splitters and geometry; only the
        files that hold the main window's dock layout are checked.
        """
        restores = [
            f"{path.relative_to(ROOT)}: {line.strip()}"
            for path in (OBS_BASIC, OBS_BASIC_DOCKS, *OAUTH_PANELS)
            for line in path.read_text(encoding="utf-8").splitlines()
            if RESTORE.search(line)
        ]
        self.assertEqual(restores, [], "dock layout restored outside DockLayout")

    def test_main_window_loads_and_saves_through_dock_layout(self):
        """The start and `saveAll()` use `BasicWindow` through DockLayout."""
        source = OBS_BASIC.read_text(encoding="utf-8")
        self.assertIn('dockLayout->Load(App()->GetUserConfig(), "BasicWindow")', source)
        self.assertIn('dockLayout->Save(App()->GetUserConfig(), "BasicWindow")', source)
        self.assertIn("DockLayout *GetDockLayout() const", OBS_BASIC_HPP.read_text(encoding="utf-8"))

    def test_reset_docks_goes_through_dock_layout(self):
        """The default layout is kept and restored only by DockLayout (#5462)."""
        self.assertIn("dockLayout->SetDefault(", OBS_BASIC.read_text(encoding="utf-8"))
        self.assertIn("dockLayout->Reset();", OBS_BASIC_DOCKS.read_text(encoding="utf-8"))
        self.assertNotIn("startingDockLayout", OBS_BASIC_HPP.read_text(encoding="utf-8"))

    def test_auth_panels_load_and_save_through_dock_layout(self):
        """Each auth panel's `<service>` layout goes through DockLayout."""
        for path in OAUTH_PANELS:
            source = path.read_text(encoding="utf-8")
            self.assertIn("GetDockLayout()->Save(main->Config(), service())", source, path.name)
            self.assertIn("GetDockLayout()->Load(main->Config(), service())", source, path.name)


if __name__ == "__main__":
    unittest.main()
