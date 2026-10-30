#!/usr/bin/env -S uv run --script
"""The port_comparison bench defines the libobs functions its oracles name (#3025).

    uv run python -m unittest ci/test_bench_libobs_stand_ins.py

The C ABI shims of obs-output and obs-hotkey, and the output interleave
oracle (rust/obs-c-oracle/oracle/output_interleave.c), call these libobs
functions by their libobs names; the cargo tests that link them define
stand-ins. The bench links the same shims and oracle. On Linux and macOS
the linker leaves the unused callers out, but lld-link on Windows reports
each name as an undefined symbol, so the bench did not build there. No bench
calls them: the bench's stand-ins only abort.

Linux CI cannot link for Windows, so this test checks the source; the
Windows build (`soldr cargo bench -p obs-c-oracle --bench port_comparison
--no-run`) is the check that counts.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "rust/obs-c-oracle/benches/port_comparison.rs"

# The undefined symbols lld-link reported for the bench with /ERRORLIMIT:0 (#3025).
SYMBOLS = (
    "obs_encoder_get_name",
    "obs_encoder_group_num_started",
    "obs_encoder_interleave_info",
    "obs_encoder_packet_create_instance",
    "obs_encoder_packet_release",
    "obs_hotkey_binding_fields",
    "obs_hotkey_binding_press",
    "obs_hotkey_binding_release",
    "obs_hotkey_key_is_pressed",
    "obs_hotkey_pair_call_func",
    "obs_hotkey_pair_fields",
    "obs_hotkeys_mutex_lock",
    "obs_hotkeys_mutex_unlock",
    "obs_hotkeys_store_key_translation",
    "obs_output_actual_start",
    "obs_output_actual_stop",
    "obs_output_begin_data_capture",
    "obs_output_can_begin_data_capture",
    "obs_output_caption_track_fields",
    "obs_output_delay_fields",
    "obs_output_delay_signal",
    "obs_output_get_name",
    "obs_output_initialize_encoders",
    "obs_output_interleave_active",
    "obs_output_interleave_add_caption",
    "obs_output_interleave_fields",
    "obs_output_interleave_first_audio_encoder_index",
    "obs_output_interleave_first_video_encoder_index",
)


class BenchStandInsTest(unittest.TestCase):
    def test_every_symbol_has_an_exported_definition(self):
        text = BENCH.read_text()
        for symbol in SYMBOLS:
            with self.subTest(symbol=symbol):
                pattern = rf"#\[unsafe\(no_mangle\)\]\s*(pub\s+)?(unsafe\s+)?extern \"C\" fn {symbol}\("
                self.assertRegex(text, pattern)

    def test_every_symbol_is_still_declared_by_a_shim(self):
        # A symbol a shim no longer declares needs no stand-in: drop it here
        # and in the bench, so the list does not grow stale.
        shims = "".join(path.read_text() for path in sorted(ROOT.glob("rust/*/src/ffi/**/*.rs")))
        for symbol in SYMBOLS:
            with self.subTest(symbol=symbol):
                self.assertRegex(shims, rf"\bfn {symbol}\(")


if __name__ == "__main__":
    unittest.main()
