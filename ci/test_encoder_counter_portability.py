#!/usr/bin/env -S uv run --script
"""Keep the encoder counter characterization portable across strict C hosts."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class EncoderCounterPortability(unittest.TestCase):
    def test_log_capture_declares_its_printf_contract(self):
        source = (ROOT / "test/cmocka/test_encoder_counter_getters.c").read_text()
        self.assertIn("__attribute__((format(printf, 2, 0)))", source)

    def test_private_encoder_layout_imports_windows_pthreads(self):
        fragment = (ROOT / "test/cmocka/tests.d/test_encoder_counter_getters.cmake").read_text()
        self.assertIn("$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>", fragment)


if __name__ == "__main__":
    unittest.main()
