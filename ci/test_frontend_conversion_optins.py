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

Apple TUs keep a pin instead of the group, stated in an `if(OS_MACOS)` branch so
the flags are visible platform options. The branch is not a workaround for the
generator -- a nested `$<NOT:$<PLATFORM_ID:Darwin>>` genex expands in both the
Ninja and Xcode generators. #2979's genex pin reached the app TUs and the lane was
still red (#3096) because `shared/properties-view` adds its conversion groups as
INTERFACE options, which CMake appends after the consumer's own options, and
`-Wconversion` re-enables `-Wsign-conversion` diagnostics regardless of an earlier
`-Wno-sign-conversion`. The fix therefore has to be on that INTERFACE block: it
must contribute none of the three groups on Apple.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRONTEND = ROOT / "frontend" / "CMakeLists.txt"
PROPERTIES_VIEW = ROOT / "shared" / "properties-view" / "CMakeLists.txt"
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

    The macOS app lane keeps a `-Wno-sign-conversion` pin rather than the ratcheted
    group, because the group reached its TUs through `OBS::properties-view`'s
    INTERFACE options and stopped inside Qt 6.11's own headers (#3096). The branch
    shape keeps that pin a real, visible compile option instead of an expression
    whose contribution depends on where a linked interface sorts.
    """
    match = re.search(
        r"^if\(OS_MACOS\)$(.*?)^(?:else\(\)|endif\(\))$",
        without_comments(text),
        re.M | re.S,
    )
    return match.group(1) if match else None


def without_comments(text: str) -> str:
    """The CMake text with comment lines dropped, so a comment cannot satisfy a check."""
    return "\n".join(
        line for line in text.splitlines() if not line.lstrip().startswith("#")
    )


class FrontendConversionOptinsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = FRONTEND.read_text(encoding="utf-8")

    def test_one_opt_in_block_names_the_ratcheted_group(self) -> None:
        calls = target_compile_options(self.text, "obs-studio")
        ratcheted = [
            flags
            for flags in calls
            if any(not flag.startswith("-Wno-") for flag in flags)
        ]
        self.assertEqual(
            len(ratcheted),
            1,
            "one opt-in block per target: a second block silently widens the "
            "groups the target opts into (#2909, #2938)",
        )
        self.assertEqual(
            sorted(set(ratcheted[0])),
            sorted(APP_TARGET_GROUPS),
            "the app target opts into one conversion group; a second block widens it "
            "while the remaining files are still unconverted (#2938)",
        )
        pins = [
            flags for flags in calls if all(flag.startswith("-Wno-") for flag in flags)
        ]
        self.assertEqual(len(pins), 1, "the Apple pin is one block (#3096)")
        self.assertIn(
            pins[0][0],
            darwin_branch(self.text) or "",
            "the pin names its group on the Darwin branch (#3096)",
        )

    def test_darwin_states_the_pin_in_a_branch(self) -> None:
        self.assertNotIn(
            "$<NOT:$<PLATFORM_ID:Darwin>>",
            without_comments(self.text),
            "the Apple pin is stated as an `if(OS_MACOS)` branch, not a genex: the "
            "genex expands in both generators, so it was never the reason the lane "
            "stayed red -- the ratcheted group arrived from an INTERFACE library "
            "(#3096) -- but a branch keeps the pin a visible compile option",
        )
        branch = darwin_branch(self.text)
        self.assertIsNotNone(
            branch, "an `if(OS_MACOS)` branch states the Apple pin (#3096)"
        )
        self.assertEqual(
            set(re.findall(r"-W(?:no-)?[a-z-]*conversion", branch or "")),
            {"-Wno-sign-conversion"},
            "Darwin keeps the global pin as a target option, so Apple TUs never see "
            "a positive conversion group (#2950, #3096)",
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


class PropertiesViewConversionOptinsTest(unittest.TestCase):
    """The interface group must not leak into Apple consumers' TUs (#3096)."""

    def setUp(self) -> None:
        self.text = PROPERTIES_VIEW.read_text(encoding="utf-8")

    def test_apple_keeps_the_sign_conversion_pin(self) -> None:
        branch = darwin_branch(self.text)
        self.assertIsNotNone(
            branch, "an `if(OS_MACOS)` branch states the Apple pin (#3096)"
        )
        self.assertEqual(
            set(re.findall(r"-W(?:no-)?[a-z-]*conversion", branch or "")),
            {"-Wno-sign-conversion"},
            "properties-view is INTERFACE: its group reaches the app target's Apple "
            "TUs, where Qt 6.11's headers are not converted (#3096)",
        )

    def test_other_platforms_keep_the_ratcheted_groups(self) -> None:
        calls = target_compile_options(self.text, "properties-view")
        groups = [
            flags
            for flags in calls
            if any(not flag.startswith("-Wno-") for flag in flags)
        ]
        self.assertEqual(len(groups), 1, "one ratcheted opt-in block (#3096)")
        self.assertEqual(
            set(groups[0]),
            {"-Wconversion", "-Wsign-conversion", "-Wfloat-conversion"},
            "the non-Apple consumers keep the groups this target ratcheted (#2625)",
        )


if __name__ == "__main__":
    unittest.main()
