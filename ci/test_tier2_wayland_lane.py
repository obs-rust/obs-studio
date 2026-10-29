#!/usr/bin/env -S uv run --script
"""The Tier 2 Linux lane compiles the Wayland host sources (#2890).

    uv run python -m unittest ci/test_tier2_wayland_lane.py

`ENABLE_WAYLAND` defaults to `ON` (cmake/linux/defaults.cmake:6), so a
developer's default CMake build compiles `libobs/obs-nix-wayland.c.h`,
`libobs/obs-nix-portal-shortcuts.c.h` and `libobs-opengl/gl-wayland-egl.c.h`.
The Tier 2 build configured the feature OFF instead, so those translation
units reached no lane: #2885 broke a default Linux build with nine
`-Werror=sign-conversion` diagnostics while master stayed green.

The lane now configures `-DENABLE_WAYLAND=ON` and lists the Wayland ported
groups among the objects excluded from hybrid libobs, matching what CMake
builds. These assertions fail on the tree before #2890.
"""

from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
PORTED_SOURCES = ROOT / "rust" / "tools" / "ported_sources.py"


def load_ported_sources():
    spec = importlib.util.spec_from_file_location("ported_sources", PORTED_SOURCES)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules["ported_sources"] = module
    spec.loader.exec_module(module)
    return module


class Tier2WaylandLaneTest(unittest.TestCase):
    def setUp(self):
        self.text = VALIDATE.read_text(encoding="utf-8")

    def test_every_tier2_configure_enables_wayland(self):
        values = []
        for line in self.text.splitlines():
            if "-DENABLE_WAYLAND=" not in line:
                continue
            for token in line.split():
                if token.startswith("-DENABLE_WAYLAND="):
                    values.append(token.split("=", 1)[1])
        self.assertTrue(values, "no -DENABLE_WAYLAND option in validate.sh")
        self.assertEqual(
            ["ON"] * len(values),
            values,
            "every Tier 2 configure must build the Wayland host sources (#2890)",
        )

    def test_no_wayland_opt_out_is_passed_to_the_source_sets(self):
        self.assertNotIn(
            "--without-wayland",
            self.text,
            "the Wayland ported groups are built, so they must be excluded "
            "from hybrid libobs like every other ported group (#2890)",
        )

    def test_wayland_ported_groups_join_the_exclusion_list(self):
        ported = load_ported_sources()
        with_wayland = set(ported.ported_objects(ROOT, "linux"))
        without_wayland = set(ported.ported_objects(ROOT, "linux", wayland=False))
        self.assertTrue(
            with_wayland - without_wayland,
            "no ported group is behind ENABLE_WAYLAND, so the lane's "
            "exclusion list cannot prove the feature reaches the build",
        )

    def test_cmake_builds_the_wayland_host_unity_group(self):
        os_linux = (ROOT / "libobs" / "cmake" / "os-linux.cmake").read_text(encoding="utf-8")
        self.assertIn("if(ENABLE_WAYLAND)", os_linux)
        self.assertIn("build/libobs.nix-wayland+.h.c", os_linux)


if __name__ == "__main__":
    unittest.main()
