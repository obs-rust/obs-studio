#!/usr/bin/env -S uv run --script
"""Lists that every port appends to stay sorted, one entry per line (#1997).

Two ports that both append at the end of a shared list touch the same hunk
and the second one is ejected from the merge queue. Sorted lists put each
new entry at its own place. Order is not meaning in any of these:

- test/cmocka/build/test-graph.json `targets` and `tests`: obs-build reads
  them into BTreeMaps; CMake defines each target independently.
- libobs/build/source-sets.json `*_PORTED_SOURCES`: unity groups, each its
  own translation unit, with no constructors.
- rust/obs-c-oracle/benches/port_comparison.rs: the `mod` lines and the
  `modules![...]` list in comparisons(); each unit's benchmarks stand alone.
- ci/test_ported_sources.py: the per-port object tables.
"""

import importlib.util
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def load_json(relative):
    return json.loads((ROOT / relative).read_text())


class SortedSharedListsTest(unittest.TestCase):
    def assert_sorted(self, name, items):
        items = list(items)
        self.assertTrue(items, f"{name} is empty: the check would pass vacuously")
        self.assertEqual(items, sorted(items), f"{name} must be sorted")

    def test_native_test_graph(self):
        graph = load_json("test/cmocka/build/test-graph.json")
        self.assert_sorted("test-graph.json targets", graph["targets"])
        self.assert_sorted("test-graph.json tests", graph["tests"])

    def test_ported_source_sets(self):
        sets = load_json("libobs/build/source-sets.json")
        ported = {k: v for k, v in sets.items() if k.endswith("_PORTED_SOURCES")}
        self.assertIn("LIBOBS_PORTED_SOURCES", ported)
        for name, members in ported.items():
            self.assert_sorted(f"source-sets.json {name}", members)

    def test_port_comparison_lists(self):
        text = (ROOT / "rust/obs-c-oracle/benches/port_comparison.rs").read_text()
        mods = re.findall(r"^mod (\w+);$", text, re.M)
        listed = re.search(r"^    modules!\[\n(.*?)^    \];$", text, re.M | re.S)
        self.assertIsNotNone(listed, "port_comparison.rs comparisons() has a modules![...] list")
        calls = re.findall(r"^        (\w+),$", listed.group(1), re.M)
        self.assert_sorted("port_comparison.rs mod lines", mods)
        self.assert_sorted("port_comparison.rs modules![...] entries", calls)
        self.assertEqual(sorted(mods), sorted(calls), "every bench module is called once")

    def test_ported_sources_tables(self):
        spec = importlib.util.spec_from_file_location(
            "test_ported_sources", ROOT / "ci/test_ported_sources.py"
        )
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assert_sorted("PORTED_ON_EVERY_HOST", module.PORTED_ON_EVERY_HOST)
        self.assert_sorted("PORTED_ON_LINUX", module.PORTED_ON_LINUX)
        self.assert_sorted("NOT_PORTED", module.NOT_PORTED)


if __name__ == "__main__":
    unittest.main()
