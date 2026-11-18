#!/usr/bin/env -S uv run --script
"""The linked module manifest must come from native module selection."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class ModuleTest(unittest.TestCase):
    def test_native_plugin_selection_generates_the_cross_manifest(self):
        text = (ROOT / "cmake/cross/macos-tier2.cmake").read_text()
        self.assertIn('add_subdirectory("${OBS_SOURCE_ROOT}/plugins"', text)
        self.assertIn(
            "get_property(_enabled GLOBAL PROPERTY OBS_MODULES_ENABLED)", text
        )
        self.assertIn("tier2-modules.txt", text)
        self.assertNotIn("set(OBS_CORE_MODULE_COUNT 0)", text)

    def test_objc_visibility_and_capture_bundle_identity_match_native(self):
        text = (ROOT / "cmake/cross/macos-tier2.cmake").read_text()
        self.assertIn("OBJC_VISIBILITY_PRESET hidden", text)
        self.assertIn('MACOSX_BUNDLE_GUI_IDENTIFIER "com.obsproject.${target}"', text)


if __name__ == "__main__":
    unittest.main()
