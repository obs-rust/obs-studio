#!/usr/bin/env -S uv run --script
"""The cross toolchain comes from `soldr cc --target ... --print-*` (#3059).

`cmake/common/soldr.cmake` used to find the cross compiler and SDK by walking
soldr's install layout (`bin/llvm-*/hardlinked/bin/clang`,
`bin/apple-sdk/*/darwin-<arch>/package/sdk`, `sdk/x86_64-pc-windows-msvc/xwin/*`),
because soldr did not expose them. soldr 0.9.45 does, through `soldr cc
--target <triple> --print-cc/--print-cxx/--print-linker/--print-sysroot/
--print-ar/--print-rc`, so the layout is no longer a supported interface and a
soldr release that moves a directory silently breaks every cross compile.

    uv run python -m unittest ci/test_soldr_cross_toolchain.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOLDR_CMAKE = ROOT / "cmake/common/soldr.cmake"

# Printers the toolchain file needs. `rc` is Windows-only: `soldr cc` refuses
# it for the Apple targets, which have no resource compiler.
PRINTERS = ("cc", "cxx", "linker", "sysroot", "ar", "rc")

# Any of these means the function is still deriving paths from soldr's root
# instead of asking soldr for them.
LAYOUT_GUESSES = (
    "/hardlinked/",
    "apple-sdk/",
    "xwin/",
    "_obs_soldr_locate",
    "soldr status",
    "rustup target add",
)


def function_body(name: str) -> str:
    """The text of `function(<name> ...) ... endfunction()`, without the header."""
    text = SOLDR_CMAKE.read_text()
    start = re.search(rf"function\(\s*{re.escape(name)}\b", text)
    assert start, f"function({name} ...) not found"
    end = text.index("endfunction()", start.start())
    return text[start.start() : end]


class SoldrCrossToolchainTest(unittest.TestCase):
    def test_toolchain_is_resolved_through_soldr_cc_printers(self):
        body = function_body("obs_soldr_cross_toolchain")
        helper = " ".join(function_body("_obs_soldr_cc_print").split())
        self.assertIn(
            "cc --target ${triple} --print-${what}",
            helper,
            "the tool must come from `soldr cc --target <triple> --print-<what>`",
        )
        for printer in PRINTERS:
            with self.subTest(printer=printer):
                self.assertRegex(
                    body,
                    rf"_obs_soldr_cc_print\(\s*{printer}\b",
                    f"the toolchain file must ask soldr for `--print-{printer}` "
                    "(soldr#2335)",
                )

    def test_no_soldr_layout_guessing_remains(self):
        text = SOLDR_CMAKE.read_text()
        for guess in LAYOUT_GUESSES:
            with self.subTest(guess=guess):
                self.assertNotIn(
                    guess,
                    text,
                    "resolve the toolchain with `soldr cc --target <triple> --print-*`, "
                    "never from soldr's install layout",
                )

    def test_every_alias_has_a_printer_triple(self):
        text = SOLDR_CMAKE.read_text()
        aliases = re.search(r"^set\(OBS_CROSS_ALIASES ([^)]+)\)$", text, re.M)
        assert aliases, "OBS_CROSS_ALIASES not found"
        for alias in aliases.group(1).split():
            with self.subTest(alias=alias):
                entry = re.search(
                    rf"^set\(_obs_cross_{re.escape(alias)} (\S+)", text, re.M
                )
                self.assertIsNotNone(entry, f"no triple for {alias}")
                assert entry is not None
                self.assertRegex(entry.group(1), r"^(x86_64|aarch64)-\S+$")


if __name__ == "__main__":
    unittest.main()
