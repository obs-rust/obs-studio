#!/usr/bin/env -S uv run --script
"""A libobs port edits only its own unit file (#3236).

Ports used to add a `set_property(SOURCE build/libobs.ported-...)` block to
libobs/CMakeLists.txt for a group's compile options, and the ported headers
and "have Rust ports" comments to both cmake/os-linux.cmake and
cmake/os-freebsd.cmake, so parallel ports conflicted on the same hunks.
Those now live in the unit's libobs/build/ported-source-sets/<unit>.json:

    {
      "LIBOBS_<...>PORTED_SOURCES": ["build/libobs.ported-<unit>+.h.c"],
      "compile_options": {"gnu": [...], "msvc": [...]},   # optional
      "headers": ["<header>.h"]                           # optional
    }

cmake/sources.cmake applies them (libobs_add_ported_sources), wherever the
group's set is built.
"""

import importlib.util
import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIBOBS = ROOT / "libobs"
UNITS = LIBOBS / "build/ported-source-sets"
CENTRAL = [LIBOBS / "CMakeLists.txt", *sorted((LIBOBS / "cmake").glob("os-*.cmake"))]
FIELDS = {"compile_options", "headers"}
FAMILIES = {"gnu", "msvc"}

SPEC = importlib.util.spec_from_file_location("ported_sources", ROOT / "rust/tools/ported_sources.py")
PORTED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PORTED)


def units():
    return {path.stem: json.loads(path.read_text()) for path in sorted(UNITS.glob("*.json"))}


def ported_headers():
    """The header next to each implementation file a ported group compiles."""
    headers = set()
    for name, groups in PORTED.source_sets(ROOT).items():
        if name.endswith("_PORTED_SOURCES"):
            for group in groups:
                for member in PORTED.group_members(ROOT, group):
                    headers.add(re.sub(r"\.(c|m)\.h$", ".h", member))
    return headers


class PortUnitMetadataTest(unittest.TestCase):
    def test_central_files_name_no_ported_group(self):
        for path in CENTRAL:
            with self.subTest(path=path.name):
                self.assertNotIn("build/libobs.ported-", path.read_text(),
                                 "per-group options go in the unit's compile_options")

    def test_central_files_list_no_ported_header(self):
        """libobs-objects source lists; OBS_PUBLIC_HEADERS (install) stays global."""
        headers = ported_headers()
        self.assertTrue(headers)
        for path in CENTRAL:
            self.assertNotRegex(path.read_text(), r"have Rust ports?:", f"{path.name}: port comment")
            text = "\n".join(re.findall(r"target_sources\(\s*libobs-objects[^)]*\)", path.read_text()))
            for header in headers:
                with self.subTest(path=path.name, header=header):
                    self.assertNotRegex(text, rf"(?<![\w/.-]){re.escape(header)}(?![\w.])",
                                        "a ported header goes in the unit's headers")

    def test_unit_schema(self):
        for unit, data in units().items():
            with self.subTest(unit=unit):
                sets = [key for key in data if key.endswith("_PORTED_SOURCES")]
                self.assertTrue(sets, "a unit registers at least one ported group")
                self.assertLessEqual(set(data) - set(sets), FIELDS)
                options = data.get("compile_options", {})
                self.assertLessEqual(set(options), FAMILIES)
                for flags in options.values():
                    self.assertTrue(flags and all(isinstance(f, str) and f for f in flags))
                for header in data.get("headers", []):
                    self.assertTrue((LIBOBS / header).is_file(), header)

    def test_migrated_options_stay_on_their_groups(self):
        got = units()
        self.assertEqual(got["audio-resampler-drift"]["compile_options"], {"gnu": ["-ffp-contract=off"]})
        self.assertEqual(got["audio-controls"]["compile_options"], {"msvc": ["/wd4056", "/wd4756"]})
        self.assertEqual(got["nsgif"]["compile_options"]["gnu"], ["-Wno-conversion", "-Wno-sign-conversion"])
        self.assertEqual(got["nsgif"]["compile_options"]["msvc"], ["/wd4244", "/wd4267"])
        self.assertEqual(got["data"]["compile_options"],
                         {"gnu": ["-Wno-deprecated-declarations"], "msvc": ["/wd4996"]})
        self.assertEqual(got["macos-cocoa"]["compile_options"], {"gnu": ["-fobjc-arc"]})

    def test_sources_cmake_applies_the_fields(self):
        text = (LIBOBS / "cmake/sources.cmake").read_text()
        for needle in ('"compile_options"', '"headers"', "function(libobs_add_ported_sources",
                       "C_COMPILER_ID:GNU,Clang,AppleClang", "C_COMPILER_ID:MSVC"):
            self.assertIn(needle, text)


if __name__ == "__main__":
    unittest.main()
