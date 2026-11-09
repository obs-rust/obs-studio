#!/usr/bin/env -S uv run --script
"""Keep single-hotkey registration extracted and bound to each fixture's core."""

import json
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
ORIGINAL = "libobs/obs-hotkey.c.h"
EXTRACTED = "libobs/obs-hotkey-register.c.h"


class HotkeyRegistrationExtractionTest(unittest.TestCase):
    def test_single_registration_definitions_are_extracted(self):
        extracted = ROOT / EXTRACTED
        self.assertTrue(extracted.is_file(), EXTRACTED)
        moved = extracted.read_text()
        original = (ROOT / ORIGINAL).read_text()
        for owner in ("frontend", "encoder", "output", "service", "source"):
            name = f"obs_hotkey_register_{owner}"
            definition = rf"^obs_hotkey_id {name}\([^;]*?\)\s*\{{"
            with self.subTest(function=name):
                self.assertEqual(len(re.findall(definition, moved, re.M | re.S)), 1)
                self.assertIsNone(re.search(definition, original, re.M | re.S))
        self.assertRegex(original, r"obs_hotkey_register_internal\([^;]*?\)\s*\{")
        group = (ROOT / "libobs/build/libobs.core+.h.c").read_text()
        self.assertEqual(group.count(f'#include "{EXTRACTED}"'), 1)

    def test_isolated_cores_compile_registration_in_the_same_translation_unit(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        consumers = {
            name: target
            for name, target in graph["targets"].items()
            if ORIGINAL in target.get("sources", [])
        }
        self.assertTrue(consumers)
        for name, target in consumers.items():
            with self.subTest(target=name):
                # Library fallback registers against a different `obs` global.
                self.assertIn(EXTRACTED, target["sources"])
                self.assertEqual(target.get("pch"), "libobs/build/libobs.pch.h")
                groups = list(target.get("unity_groups", {}).values())
                self.assertIn([EXTRACTED, ORIGINAL], groups)
                self.assertCountEqual(
                    [source for group in groups for source in group], target["sources"]
                )
                for group in groups:
                    if ORIGINAL not in group:
                        self.assertEqual(len(group), 1, group)


if __name__ == "__main__":
    unittest.main()
