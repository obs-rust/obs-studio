#!/usr/bin/env -S uv run --script
"""Keep isolated hotkey cores and their public enumeration in one executable."""

import json
from pathlib import Path
import re

from native_test_graph import load_graph

import unittest


class HotkeyEnumerationConsumersTest(unittest.TestCase):
    def test_isolated_cores_compile_the_real_enumeration_source(self):
        root = Path(__file__).resolve().parents[1]
        graph = load_graph(root / "test/cmocka/build/test-graph.d")
        consumers = {
            name: target
            for name, target in graph["targets"].items()
            if "libobs/obs-hotkey.c.h" in target.get("sources", [])
        }
        self.assertTrue(consumers)
        for name, target in consumers.items():
            with self.subTest(target=name):
                # Falling back to libobs enumerates its different `obs` global,
                # even though registration succeeded in the fixture's core.
                self.assertIn("libobs/obs-hotkey-enum.c.h", target["sources"])

    def test_context_enumeration_has_the_same_extraction_boundary(self):
        root = Path(__file__).resolve().parents[1]
        core = (root / "libobs/obs-hotkey.c.h").read_text()
        enumeration = (root / "libobs/obs-hotkey-enum.c.h").read_text()
        for name in ("enum_context_hotkeys", "obs_hotkey_seam_enum_context"):
            definition = rf"\b{name}\s*\([^;{{}}]*\)\s*\{{"
            with self.subTest(function=name):
                self.assertRegex(enumeration, definition)
                self.assertIsNone(re.search(definition, core))

        # The core inserts keys with a custom integer hash. A default uthash
        # lookup in the extracted translation unit silently misses live IDs.
        self.assertIn(
            "#define HASH_FIND_HKEY(head, id, out) ((out) = obs_hotkey_seam_find(id))",
            enumeration,
        )


if __name__ == "__main__":
    unittest.main()
