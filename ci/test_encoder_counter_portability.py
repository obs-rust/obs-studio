#!/usr/bin/env -S uv run --script
"""Keep the encoder counter characterization portable across strict C hosts."""
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class EncoderCounterPortability(unittest.TestCase):
    def test_log_capture_declares_its_printf_contract(self):
        source = (ROOT / "test/cmocka/test_encoder_counter_getters.c").read_text()
        self.assertIn("__attribute__((format(printf, 2, 0)))", source)

    def test_private_encoder_layout_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        target = graph["targets"]["test_encoder_counter_getters"]
        self.assertIn({"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}, target["libraries"])
        self.assertEqual(target["sources"], ["test/cmocka/test_encoder_counter_getters.c"])


if __name__ == "__main__":
    unittest.main()
