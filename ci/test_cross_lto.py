"""OBS_CROSS_LTO wires cross-language ThinLTO into libobs (#3217).

Usage:
    uv run python -m unittest ci/test_cross_lto.py

Opt-in (default OFF): Clang on Linux, rustc's LLVM major equal to Clang's
(read from Corrosion's Rust_LLVM_VERSION_MAJOR; a regex capture is cleared
by the next match), the staticlib built with -Clinker-plugin-lto, the C
with -flto=thin, libobs linked by lld with --undefined-version (lld rejects
the version script's Windows-only names), and the module-cache key records
the switch so a non-LTO cached archive is never linked into an LTO build.
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST = ROOT / "libobs/cmake/rust.cmake"
NIX = ROOT / "libobs/cmake/rust-os-nix.cmake"


class CrossLtoTest(unittest.TestCase):
    def setUp(self):
        self.text = RUST.read_text(encoding="utf-8")
        self.nix = NIX.read_text(encoding="utf-8")

    def block(self) -> str:
        return self.nix.split("if(OBS_CROSS_LTO)", 1)[1].split("\nendif()", 1)[0]

    def test_option_defaults_off(self):
        self.assertRegex(self.text, r'option\(OBS_CROSS_LTO "[^"]+" OFF\)')

    def test_requires_clang_and_matching_llvm(self):
        block = self.block()
        self.assertIn('CMAKE_C_COMPILER_ID STREQUAL "Clang"', block)
        self.assertIn("Rust_LLVM_VERSION_MAJOR", block)
        self.assertNotIn("CMAKE_MATCH_1 STREQUAL", block)

    def test_flags(self):
        block = self.block()
        self.assertIn("corrosion_add_target_rustflags(libobs_rust -Clinker-plugin-lto)", block)
        self.assertIn("-flto=thin", block)
        self.assertIn("-fuse-ld=lld", block)
        self.assertIn("LINKER:--undefined-version", block)

    def test_module_cache_key_records_it(self):
        self.assertIn('\\"cross_lto\\": \\"${OBS_CROSS_LTO}\\"', self.text)


if __name__ == "__main__":
    unittest.main()
