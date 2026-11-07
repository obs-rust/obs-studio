#!/usr/bin/env -S uv run --script
"""Field adapters must not add a per-port oracle build.rs append point (#3910)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "rust/obs-c-oracle/build.rs"


def explicit_field_adapters(source: str) -> list[str]:
    """Reject concrete adapter paths, including ones assigned before extraction.

    The discovery pattern/suffix is allowed; a concrete obs-*-fields.c.h
    filename belongs in the filesystem, never in a hand-maintained build list.
    """
    return re.findall(r'"(obs-[\w-]+-fields\.c\.h)"', source)


class OracleFieldDiscoveryTest(unittest.TestCase):
    def test_no_per_port_extract_blocks(self):
        self.assertEqual(
            explicit_field_adapters(BUILD.read_text()),
            [],
            "discover field adapters instead of adding per-port build.rs blocks",
        )

    def test_rejects_hand_written_extract_fixture(self):
        source = '''std::fs::write(
            extract_dir.join("sibling_fields.inc"),
            extract_after_libobs_pch(&libobs.join("obs-sibling-fields.c.h")),
        ).expect("write sibling adapter extract");'''
        self.assertEqual(explicit_field_adapters(source), ["obs-sibling-fields.c.h"])

    def test_rejects_adapter_assigned_before_extract(self):
        self.assertEqual(
            explicit_field_adapters('let src = libobs.join("obs-other-fields.c.h");'),
            ["obs-other-fields.c.h"],
        )

    def test_allows_discovery_pattern_and_suffix(self):
        self.assertEqual(explicit_field_adapters('"obs-*-fields.c.h" "-fields.c.h"'), [])


if __name__ == "__main__":
    unittest.main()
