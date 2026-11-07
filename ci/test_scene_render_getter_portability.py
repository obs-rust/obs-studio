#!/usr/bin/env -S uv run --script
"""The real scene layout needs the Windows pthread header dependency."""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class SceneRenderGetterPortability(unittest.TestCase):
    def test_scene_layout_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        self.assertIn(
            {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"},
            graph["targets"]["test_scene_item_render_getters"]["libraries"],
        )


if __name__ == "__main__":
    unittest.main()
