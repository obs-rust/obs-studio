#!/usr/bin/env -S uv run --script
"""The Rust release profile keeps whole-staticlib optimization (#3187, #3176).

    uv run python -m unittest ci/test_cargo_release_profile.py

libobs links one Rust staticlib built from ~25 workspace crates. Without LTO,
small hot helpers in one crate (obs-util, obs-callback, obs-media-io shims) are
not inlined into callers in another, so every per-frame and per-tick call pays
a real function call. Thin, not fat: fat LTO with one codegen unit slowed the
audio_mix scene_mix_add and source_volume benches 1.8-2.7x, thin did not
(PR #3225). The bench profile inherits release, so
`rust/tools/benchmarks/record.py` measures these settings.
panic = "abort" keeps panics from unwinding across the C ABI.
"""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EXPECTED = {"panic": "abort", "lto": "thin"}


class CargoReleaseProfileTest(unittest.TestCase):
    def test_release_profile_optimizes_the_whole_staticlib(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        release = manifest.get("profile", {}).get("release", {})
        errors = [
            f"[profile.release] {key} = {value!r} expected, found {release.get(key)!r}"
            for key, value in EXPECTED.items()
            if release.get(key) != value
        ]
        self.assertEqual(errors, [], "\n".join(errors))


if __name__ == "__main__":
    unittest.main()
