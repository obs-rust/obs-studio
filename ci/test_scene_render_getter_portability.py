#!/usr/bin/env -S uv run --script
"""The real scene layout needs the Windows pthread header dependency."""
import json
from pathlib import Path
import unittest

try:
    from ci.native_test_graph import load_graph
except ModuleNotFoundError:
    from native_test_graph import load_graph

ROOT = Path(__file__).resolve().parents[1]


class SceneRenderGetterPortability(unittest.TestCase):
    def test_scene_layout_imports_windows_pthreads(self):
        graph = load_graph(ROOT / "test/cmocka/build/test-graph.d")
        self.assertIn(
            {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"},
            graph["targets"]["test_scene_item_render_getters"]["libraries"],
        )


if __name__ == "__main__":
    unittest.main()
