#!/usr/bin/env -S uv run --script
"""Which cross-compile aliases a PR's changes need (#1294 part 2).

cmake/cross/ compiles libobs and the Windows/macOS host groups of the
components it lists (each `DIR` of its CMakeLists.txt) with soldr's LLVM and
SDKs, on Linux. A PR that changes a Windows or macOS file in one of those
components, or the cross check itself, runs it for that OS; nothing else
does, so a PR with no host C/C++ change pays nothing.

    uv run python -m unittest ci/test_cross_scope.py
"""

from __future__ import annotations

import unittest
from pathlib import Path

from ci_scope import cross_dirs, cross_targets, scope

ROOT = Path(__file__).resolve().parent.parent
CROSS = (ROOT / "cmake/cross/CMakeLists.txt").read_text(encoding="utf-8")


def read(path: str) -> str:
    return CROSS if path == "cmake/cross/CMakeLists.txt" else ""


class CrossScopeTest(unittest.TestCase):
    def targets(self, *files: str) -> str:
        return cross_targets(list(files), read)

    def test_dirs_come_from_the_cross_cmakelists(self):
        dirs = cross_dirs(read)
        for d in ("libobs", "plugins/win-capture", "plugins/mac-capture", "plugins/frontend-tools", "frontend", "shared"):
            self.assertIn(d, dirs)

    def test_windows_host_files_select_win_x64(self):
        for path in (
            "plugins/win-capture/window-capture.c.h",
            "libobs/util/platform-windows.c.h",
            "plugins/frontend-tools/frontend-tools-host-windows.c.h",
            "plugins/frontend-tools/build/frontend-tools.host-windows+.h.c",
            "frontend/updater/updater.cpp.hpp",
            # The frontend's Qt host groups (#2408): #2176 broke this PCH and
            # only the nightly Windows app build caught it (#2359).
            "frontend/build/frontend-windows-obs-main.pch.hpp",
            "frontend/utility/platform-windows.cpp.hpp",
        ):
            self.assertEqual(self.targets(path), "win-x64", path)

    def test_macos_host_files_select_mac_arm64(self):
        for path in (
            "plugins/mac-capture/mac-sck-video-capture.m.h",
            "libobs/obs-cocoa.m.h",
            "plugins/frontend-tools/frontend-tools-host-macos.c.h",
            "libobs/util/platform-cocoa.m.h",
            "frontend/build/frontend-macos-arc-mm.pch.hpp",
            "frontend/utility/platform-osx.mm.hpp",
        ):
            self.assertEqual(self.targets(path), "mac-arm64", path)

    def test_the_check_itself_selects_both(self):
        for path in ("cmake/cross/CMakeLists.txt", "cmake/cross/docker.py", "cmake/common/soldr.cmake"):
            self.assertEqual(self.targets(path), "mac-arm64,win-x64", path)

    def test_pr_1292_selects_both(self):
        from test_ci_scope import PR_1292_FILES

        self.assertEqual(self.targets(*PR_1292_FILES), "mac-arm64,win-x64")

    def test_other_changes_select_nothing(self):
        for path in (
            "docs/rust-port/naming.md",
            "libobs/obs.c",
            "plugins/linux-pipewire/pipewire.c",
            "plugins/frontend-tools/frontend-tools-host-linux.c.h",
            "rust/obs-core/src/platform/windows.rs",
            # Shared frontend code: no host lane.
            "frontend/OBSApp.cpp.hpp",
        ):
            self.assertEqual(self.targets(path), "", path)

    def test_scope_reports_the_targets(self):
        got = scope(["plugins/win-capture/window-capture.c.h"], read=read)
        self.assertEqual(got.cross_targets, "win-x64")
        self.assertEqual(scope(["libobs/obs.c"], read=read).cross_targets, "")


if __name__ == "__main__":
    unittest.main()
