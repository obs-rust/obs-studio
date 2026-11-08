#!/usr/bin/env -S uv run --script
"""Keep the new private-layout characterization portable on Windows."""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class GetterPortability(unittest.TestCase):
    def test_private_layout_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        self.assertIn(
            {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"},
            graph["targets"]["test_canvas_readonly_getters"]["libraries"],
        )


if __name__ == "__main__":
    unittest.main()
