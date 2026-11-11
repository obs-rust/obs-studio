#!/usr/bin/env -S uv run --script
"""Keep isolated hotkey cores and their public enumeration in one executable."""

import json
from pathlib import Path
import unittest


class HotkeyEnumerationConsumersTest(unittest.TestCase):
    def test_isolated_cores_compile_the_real_enumeration_source(self):
        root = Path(__file__).resolve().parents[1]
        graph = json.loads((root / "test/cmocka/build/test-graph.json").read_text())
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


if __name__ == "__main__":
    unittest.main()
