#!/usr/bin/env -S uv run --script
"""The app target opts into exactly one conversion group at a time (#2625, #2938).

    uv run python -m unittest ci/test_frontend_conversion_optins.py

`cmake/linux/compilerconfig.cmake` pins `-Wno-conversion`, `-Wno-sign-conversion`
and `-Wno-float-conversion` globally, and each target drops a pin by naming the
group in its own `target_compile_options` once *every* file in that target is
converted. #2909 added a second block to `frontend/CMakeLists.txt` that named all
three groups while 63 files were still unconverted, so the app lanes (which PR CI
does not build) stopped on the first of them. Two blocks naming different groups
is the shape that made the target unbuildable: the earlier block is the contract,
the later one silently widens it.

The expected group here moves with the ratchet: when the remaining frontend files
are converted, add the group to `APP_TARGET_GROUPS` in the same PR that adds the
flag, and the next group is the one still pinned globally.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRONTEND = ROOT / "frontend" / "CMakeLists.txt"
LINUX = ROOT / "cmake" / "linux" / "compilerconfig.cmake"

# The groups the app target states at its call sites today.
APP_TARGET_GROUPS = {"-Wsign-conversion"}
# The three groups the global pins hide across every target.
GLOBAL_PINS = {"-Wno-conversion", "-Wno-sign-conversion", "-Wno-float-conversion"}


def target_compile_options(text: str, target: str) -> list[list[str]]:
    """The conversion flags of each `target_compile_options(<target> ...)` call."""
    calls: list[list[str]] = []
    for body in target_compile_option_bodies(text, target):
        flags = re.findall(r"-W(?:no-)?[a-z-]*conversion", body)
        if flags:
            calls.append(flags)
    return calls


def target_compile_option_bodies(text: str, target: str) -> list[str]:
    """The body of each `target_compile_options(<target> ...)` call."""
    bodies: list[str] = []
    for body in re.findall(r"target_compile_options\(([^)]*)\)", text, re.S):
        tokens = body.split()
        if tokens and tokens[0] == target:
            bodies.append(body)
    return bodies


def darwin_branch(text: str) -> str | None:
    """The body of the `if(OS_MACOS)` branch that states the Apple compile options.

    The macOS lane builds with the Xcode generator, which does not apply the
    Darwin half of a `$<NOT:$<PLATFORM_ID:Darwin>>` genex in
    `target_compile_options` (#3096), so the pin has to be a branch the
    generator cannot drop.
    """
    match = re.search(r"^if\(OS_MACOS\)$(.*?)^endif\(\)$", text, re.M | re.S)
    return match.group(1) if match else None


class FrontendConversionOptinsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = FRONTEND.read_text(encoding="utf-8")

    def test_one_opt_in_block_names_the_ratcheted_group(self) -> None:
        calls = target_compile_options(self.text, "obs-studio")
        self.assertEqual(
            len(calls),
            1,
            "one opt-in block per target: a second block silently widens the "
            "groups the target opts into (#2909, #2938)",
        )
        self.assertEqual(
            sorted(set(calls[0])),
            sorted(APP_TARGET_GROUPS),
            "the app target opts into one conversion group; a second block widens it "
            "while the remaining files are still unconverted (#2938)",
        )

    def test_darwin_pins_the_group_in_a_branch_the_xcode_generator_honours(self) -> None:
        self.assertNotIn(
            "$<NOT:$<PLATFORM_ID:Darwin>>",
            self.text,
            "the macOS app lane was still handed the ratcheted group and stopped "
            "inside Qt 6.11's own headers (#3096): the Xcode generator does not "
            "apply the Darwin half of that genex, so state the pin in an if() branch",
        )
        branch = darwin_branch(self.text)
        self.assertIsNotNone(branch, "an `if(OS_MACOS)` branch states the Apple pin (#3096)")
        self.assertEqual(
            set(re.findall(r"-W(?:no-)?[a-z-]*conversion", branch or "")),
            {"-Wno-sign-conversion"},
            "Darwin keeps the global pin as a target option, so Apple TUs never see "
            "the group the app target ratchets (#2950, #3096)",
        )

    def test_global_pins_are_still_in_place(self) -> None:
        pins = set(
            re.findall(r"-Wno-[a-z-]*conversion", LINUX.read_text(encoding="utf-8"))
        )
        self.assertEqual(
            pins,
            GLOBAL_PINS,
            "the global conversion pins carry the groups the app target has not ratcheted",
        )


if __name__ == "__main__":
    unittest.main()
