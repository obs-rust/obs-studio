#!/usr/bin/env -S uv run --script
"""Keep the Rust control adapter in each isolated hotkey fixture's core."""

import json
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
CONTROL = "libobs/obs-hotkey-background-control.c.h"
ADAPTER = "libobs/obs-hotkey-background-control-fields.c.h"


class HotkeyBackgroundControlExtractionTest(unittest.TestCase):
    def test_control_has_a_separate_extraction_boundary(self):
        core = (ROOT / "libobs/obs-hotkey.c.h").read_text()
        self.assertTrue((ROOT / CONTROL).is_file(), "background control must be extracted")
        control = (ROOT / CONTROL).read_text()
        for name in (
            "wake_background_client",
            "obs_hotkeys_retry_background",
            "obs_hotkeys_configure_background",
            "obs_hotkeys_set_background_probe",
        ):
            definition = rf"\b{name}\s*\([^;{{}}]*\)\s*\{{"
            with self.subTest(function=name):
                self.assertRegex(control, definition)
                self.assertIsNone(re.search(definition, core))
        production = (ROOT / "libobs/build/libobs.core+.h.c").read_text()
        self.assertNotIn(f'#include "{CONTROL}"', production)
        self.assertIn(f'#include "{ADAPTER}"', production)
        reference_group = "build/libobs.ported-hotkey-background-control+.h.c"
        self.assertIn(f'#include "{CONTROL}"', (ROOT / "libobs" / reference_group).read_text())
        source_set = json.loads((ROOT / "libobs/build/ported-source-sets/hotkey-background-control.json").read_text())
        self.assertEqual(source_set["LIBOBS_PORTED_SOURCES"], [reference_group])

    def test_isolated_cores_compile_the_real_control_source(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        consumers = {
            name: target
            for name, target in graph["targets"].items()
            if "libobs/obs-hotkey.c.h" in target.get("sources", [])
            or name == "test_hotkey_background_control"
        }
        self.assertTrue(consumers)
        for name, target in consumers.items():
            with self.subTest(target=name):
                # libobs owns another `obs` global; fallback must not update it
                # instead of the registry the fixture registered its keys in.
                self.assertNotIn(CONTROL, target["sources"])
                self.assertIn(ADAPTER, target["sources"])
                self.assertIn("libobs_rust-static", target["libraries"])
                for public, private in (
                    ("obs_hotkeys_retry_background", "obs_hotkey_background_control_retry_request"),
                    ("obs_hotkeys_configure_background", "obs_hotkey_background_control_configure_request"),
                    ("obs_hotkeys_set_background_probe", "obs_hotkey_background_control_probe_request"),
                ):
                    self.assertIn(f"{public}={private}", target["definitions"])


if __name__ == "__main__":
    unittest.main()
