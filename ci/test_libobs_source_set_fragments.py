#!/usr/bin/env -S uv run --script
"""libobs source sets live in one file per set (#3235).

`libobs/build/source-sets.json` held all fifteen lists in one file, so every
port edited it and parallel PRs conflicted, and its Windows and macOS lists
were invisible to path-based CI scoping. Each set is now
`libobs/build/source-sets.d/<SET>.json`, a JSON list; the set name in the file
name carries the OS token ci/platform_paths.py reads (WINDOWS, MACOS, LINUX,
NIX, PULSE, WAYLAND), so a host list edit selects its OS and a shared list
edit selects what libobs/ always did.
"""

import importlib.util
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FRAGMENTS = ROOT / "libobs/build/source-sets.d"

from ci_scope import scope

SPEC = importlib.util.spec_from_file_location("ported_sources", ROOT / "rust/tools/ported_sources.py")
PORTED = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PORTED)

# Set name -> (windows, macos, linux tier2) a fragment edit selects.
SETS = {
    "LIBOBS_SOURCES": (False, False, True),
    "LIBOBS_PORTED_SOURCES": (False, False, True),
    "LIBOBS_RUST_SOURCES": (False, False, True),
    "LIBOBS_HEVC_PORTED_SOURCES": (False, False, True),
    "LIBOBS_WINDOWS_SOURCES": (True, False, False),
    "LIBOBS_WINDOWS_OBFUSCATE_SOURCES": (True, False, False),
    "LIBOBS_WINDOWS_PORTED_SOURCES": (True, False, False),
    "LIBOBS_MACOS_SOURCES": (False, True, False),
    "LIBOBS_MACOS_PORTED_SOURCES": (False, True, False),
    "LIBOBS_LINUX_SOURCES": (False, False, True),
    "LIBOBS_LINUX_PORTED_SOURCES": (False, False, True),
    "LIBOBS_PULSE_PORTED_SOURCES": (False, False, True),
    "LIBOBS_NIX_GIO_PORTED_SOURCES": (False, False, True),
    "LIBOBS_WAYLAND_PORTED_SOURCES": (False, False, True),
    "LIBOBS_WAYLAND_GIO_PORTED_SOURCES": (False, False, True),
}


class SourceSetFragmentsTest(unittest.TestCase):
    def test_no_monolithic_manifest(self):
        self.assertFalse((ROOT / "libobs/build/source-sets.json").exists())

    def test_one_list_file_per_set(self):
        names = sorted(p.stem for p in FRAGMENTS.glob("*.json"))
        self.assertEqual(names, sorted(SETS))
        for name in names:
            members = json.loads((FRAGMENTS / f"{name}.json").read_text())
            self.assertIsInstance(members, list, name)
            self.assertTrue(all(isinstance(m, str) for m in members), name)
            self.assertEqual(len(members), len(set(members)), f"{name} has duplicates")

    def test_loader_returns_each_fragment_in_file_order(self):
        sets = PORTED.source_sets(ROOT)
        self.assertEqual(sorted(sets), sorted(SETS))
        for name, members in sets.items():
            listed = json.loads((FRAGMENTS / f"{name}.json").read_text())
            if name.endswith("_PORTED_SOURCES"):
                # Per-unit ported-source-sets/ fragments add members; sorted.
                self.assertTrue(set(listed) <= set(members), name)
            else:
                self.assertEqual(members, listed, name)

    def test_fragment_edits_select_their_os(self):
        for name, (windows, macos, tier2) in SETS.items():
            got = scope([f"libobs/build/source-sets.d/{name}.json"], lambda _p: False)
            self.assertEqual((got.windows, got.macos, got.tier2, got.core), (windows, macos, tier2, True), name)


if __name__ == "__main__":
    unittest.main()
