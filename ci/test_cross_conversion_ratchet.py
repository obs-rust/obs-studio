#!/usr/bin/env -S uv run --script
"""The cross check can host a conversion-ratchet slice (#3150, #2625).

    uv run python -m unittest ci/test_cross_conversion_ratchet.py

A Windows- or macOS-only target is compiled by no Linux lane, so the per-plugin
opt-in `#2625` uses (`-Wconversion;-Wsign-conversion;-Wfloat-conversion` in the
target's own `CMakeLists.txt`) is never measured. The one compiler on a Linux
host that sees such a target is the cross check's clang, and it inherits the
global pins from `_obs_clang_*_options`, so the cross check needs its own opt-in
and its own pin: `ci/conversion_ratchet.py` reads `target_compile_options()` in
the plugin files, while the cross targets are declared in
`cmake/cross/CMakeLists.txt`.

The set of opted-in cross targets is read from that file, never a hand-kept
copy, so the PR that opts one in adds the marker and the pin together.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

CROSS = Path(__file__).resolve().parent.parent / "cmake" / "cross" / "CMakeLists.txt"
GROUPS = ("-Wconversion", "-Wsign-conversion", "-Wfloat-conversion")
HELPER = "_obs_cross_conversion_ratchet"
# The helper's body, up to the next top-level construct.
HELPER_BODY = re.compile(rf"^function\({HELPER}\b(?P<body>.*?)^endfunction\(\)$", re.M | re.S)
# `_obs_cross_plugin(<target> ...)` declares a cross target; the call may wrap.
PLUGIN = re.compile(r"_obs_cross_plugin\(\s*(?P<target>[A-Za-z0-9_.+-]+)", re.S)
# A call of the helper at the start of a line, `_obs_cross_conversion_ratchet(<target>)`.
OPT_IN = re.compile(rf"^[ \t]*{HELPER}\(\s*(?P<target>[A-Za-z0-9_.+-]+)", re.M)


def source() -> str:
    return CROSS.read_text(encoding="utf-8")


def helper_body(text: str) -> str:
    match = HELPER_BODY.search(text)
    assert match, f"cmake/cross/CMakeLists.txt defines no {HELPER}()"
    return match.group("body")


def opted_in(text: str) -> set[str]:
    """The cross targets that call the helper."""
    return {match.group("target") for match in OPT_IN.finditer(text)}


class CrossConversionRatchet(unittest.TestCase):
    def test_the_helper_names_the_three_groups(self) -> None:
        body = helper_body(source())
        for group in GROUPS:
            self.assertIn(group, body)

    def test_the_helper_is_gated_on_clang(self) -> None:
        """MSVC has no such flags, so the alias must stay inert there."""
        body = helper_body(source())
        self.assertIn("COMPILE_LANG_AND_ID", body)
        self.assertIn("Clang", body)

    def test_the_helper_follows_the_cross_plugin_helper(self) -> None:
        """It appends to a target `_obs_cross_plugin()` already configured with
        the shared option lists, which carry the `-Wno-conversion` pins."""
        text = source()
        self.assertLess(
            text.index("function(_obs_cross_plugin"),
            text.index(f"function({HELPER}"),
            f"{HELPER}() belongs after _obs_cross_plugin()",
        )

    def test_every_opted_in_target_is_a_cross_target(self) -> None:
        text = source()
        declared = {match.group("target") for match in PLUGIN.finditer(text)}
        chosen = opted_in(text)
        self.assertTrue(chosen, f"no target calls {HELPER}()")
        self.assertLessEqual(chosen, declared, f"not declared by _obs_cross_plugin(): {chosen - declared}")

    def test_the_wiring_proof_is_opted_in(self) -> None:
        """`obs-amf-test-cross` is the no-op slice: it carries no diagnostics, so
        its RED (before this marker) is "no group named", not a compiler error."""
        self.assertIn("obs-amf-test-cross", opted_in(source()))


if __name__ == "__main__":
    unittest.main()
