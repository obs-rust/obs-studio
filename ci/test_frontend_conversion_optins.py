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
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRONTEND = ROOT / "frontend" / "CMakeLists.txt"
PROPERTIES_VIEW = ROOT / "shared" / "properties-view" / "CMakeLists.txt"
LINUX = ROOT / "cmake" / "linux" / "compilerconfig.cmake"

# A `target_compile_options(<target> INTERFACE ...)` call: the usage requirement a
# linked target hands every consumer's TUs.
INTERFACE_CALL = re.compile(
    r"target_compile_options\(\s*(?P<target>[^\s)]+)\s+INTERFACE\s+(?P<body>[^)]*)\)",
    re.S,
)

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


def tracked_cmake_lists(root: Path) -> list[Path]:
    """Every tracked `CMakeLists.txt` below `root`, vendored trees excluded.

    Vendored upstream trees (`deps/`) are never edited here, so they never carry an
    obs-rust conversion group to check.
    """
    listing = subprocess.run(
        ["git", "ls-files", "-z", "*CMakeLists.txt"],
        cwd=root,
        check=True,
        capture_output=True,
    ).stdout.decode()
    return [
        root / path
        for path in listing.replace("\0", "\n").splitlines()
        if path and not path.startswith("deps/")
    ]


def interface_conversion_groups(text: str) -> list[tuple[str, list[str]]]:
    """`(target, flags)` for every `target_compile_options(<t> INTERFACE ...)` group.

    The pins (`-Wno-*`) are per-target exceptions, not groups: only the positive
    spelling opts a target in.
    """
    groups: list[tuple[str, list[str]]] = []
    for call in INTERFACE_CALL.finditer(without_comments(text)):
        flags = [
            flag
            for flag in re.findall(r"-W(?:no-)?[a-z-]*conversion", call.group("body"))
            if not flag.startswith("-Wno-")
        ]
        if flags:
            groups.append((call.group("target"), flags))
    return groups


class LinkedInterfaceConversionGroupTest(unittest.TestCase):
    """A linked INTERFACE target must not state a conversion group (#3164).

    CMake appends a linked target's INTERFACE options *after* the consumer's own
    options, so a consumer that still carries the global pins cannot pin the group
    back off: `-Wconversion` in that position re-enables its diagnostics, and the
    consumer's `-Wno-*` is already spent. #3096 hit the Apple half of this and
    excluded Darwin in the call, which only moved the leak to the other platforms:
    under Qt 6.8 the frontend stopped on a `long double` passed to `QString::arg()`
    that the pins exist to tolerate (#3164). The group belongs to the target whose
    sources were converted, so it is stated `PRIVATE` there, never as an INTERFACE
    usage requirement.
    """

    def test_no_interface_target_states_a_conversion_group(self) -> None:
        violations = [
            f"{path.relative_to(ROOT).as_posix()}: {target}"
            for path in tracked_cmake_lists(ROOT)
            for target, _flags in interface_conversion_groups(
                path.read_text(encoding="utf-8", errors="replace")
            )
        ]
        self.assertEqual(
            violations,
            [],
            "an INTERFACE conversion group lands after the consumer's own pins, so "
            "a consumer still holding the global pins cannot turn it off (#3096, "
            "#3164); state the group PRIVATE on the target that compiles the "
            "converted sources",
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


def compiles_the_unity_tu_in_an_object_library(text: str) -> bool:
    """True when a target of its own compiles the conversion ratchet's unity TU.

    A group on an INTERFACE target is a usage requirement that reaches every
    consumer's TUs after their own options (#3164); an OBJECT library compiles the
    TU once and states the group PRIVATE, so only the converted sources see it.
    """
    return (
        re.search(r"add_library\(\s*properties-view-code\s+OBJECT\b", text) is not None
    )


class PropertiesViewConversionOptinsTest(unittest.TestCase):
    """The groups are PRIVATE on the target that compiles the TU (#3164)."""

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
            "the Apple TUs of the compiled TU keep the pin: with the group on the "
            "lane stops inside Qt 6.11's headers (#3096)",
        )

    def test_other_platforms_keep_the_ratcheted_groups(self) -> None:
        calls = target_compile_options(self.text, "properties-view-code")
        groups = [
            flags
            for flags in calls
            if any(not flag.startswith("-Wno-") for flag in flags)
        ]
        self.assertEqual(len(groups), 1, "one ratcheted opt-in block (#2625)")
        self.assertEqual(
            set(groups[0]),
            {"-Wconversion", "-Wsign-conversion", "-Wfloat-conversion"},
            "the target that compiles the converted TU keeps its groups (#2625)",
        )

    def test_the_groups_are_private_on_the_compiling_target(self) -> None:
        self.assertEqual(
            target_compile_options(self.text, "properties-view"),
            [],
            "the interface target states no conversion group: CMake appends a "
            "linked target's INTERFACE options after the consumer's own, so the "
            "frontend's global pins never applied to its TUs (#3164)",
        )
        self.assertTrue(
            compiles_the_unity_tu_in_an_object_library(self.text),
            "`properties-view-code` is the OBJECT library that compiles the unity "
            "TU, so the group is PRIVATE there rather than a usage requirement",
        )


if __name__ == "__main__":
    unittest.main()
