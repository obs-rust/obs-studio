#!/usr/bin/env -S uv run --script
"""The Tier 2 Linux build compiles the Wayland host TUs (#2890).

    uv run python -m unittest ci/test_wayland_tier2.py

`ENABLE_WAYLAND` defaults to `ON` (`cmake/linux/defaults.cmake`), so a
developer's plain `cmake` build compiles `libobs/obs-nix-wayland.c.h` and
`libobs/obs-nix-portal-shortcuts.c.h` (and `libobs-opengl`'s wayland EGL
host). The Tier 2 Linux volume used to configure with `-DENABLE_WAYLAND=OFF`,
so the conversion ratchet of #2625 could land red there and stay green in
every lane (#2885: nine `-Werror=sign-conversion` diagnostics that only the
AppImage/Flatpak builds saw).

The volume now configures with Wayland on and builds `libobs-opengl`, and its
apt list carries the client/xkbcommon development packages, so the two libobs
unity TUs and the OpenGL wayland host are compiled with the same flags as the
rest of the tree.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
APT = ROOT / "rust" / "tools" / "linux-validate" / "apt-packages.txt"
LINUX_CMAKE = ROOT / "libobs" / "cmake" / "os-linux.cmake"

WAYLAND_UNITS = ("build/libobs.nix-wayland+.h.c", "build/libobs.nix-portal-shortcuts+.h.c")


def configure_block() -> str:
    """The Tier 2 test-tree `cmake` invocation, up to its version override."""
    text = VALIDATE.read_text(encoding="utf-8")
    start = text.index('cmake -S "$SRC" -B "$build" -G Ninja')
    end = text.index('"$build.configure.log"', start)
    return text[start:end]


class WaylandTier2Test(unittest.TestCase):
    def test_tier2_configures_with_wayland_on(self):
        block = configure_block()
        self.assertIn("-DENABLE_WAYLAND=ON", block)
        self.assertNotIn("-DENABLE_WAYLAND=OFF", block)

    def test_tier2_builds_libobs_opengl(self):
        """The wayland EGL host lives in libobs-opengl, not libobs."""
        text = VALIDATE.read_text(encoding="utf-8")
        m = re.search(r"tier2_targets=\(([^)]*)\)", text)
        assert m, "no tier2_targets list"
        self.assertIn("libobs-opengl", m.group(1).split())

    def test_wayland_units_exist_in_the_libobs_sources(self):
        """The flag has a referent: both unity TUs are added under the if."""
        cmake = LINUX_CMAKE.read_text(encoding="utf-8")
        for unit in WAYLAND_UNITS:
            self.assertIn(unit, cmake)
        self.assertIn("if(ENABLE_WAYLAND)", cmake)

    def test_ported_wayland_sources_stay_in_the_c_reference(self):
        """The hybrid check must include the Wayland groups once they build."""
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertNotIn("--without-wayland", text)

    def test_wayland_headers_are_in_the_tier2_image(self):
        packages = APT.read_text(encoding="utf-8").split()
        for package in ("libwayland-dev", "libxkbcommon-dev"):
            self.assertIn(package, packages)


if __name__ == "__main__":
    unittest.main()
