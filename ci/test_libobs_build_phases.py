#!/usr/bin/env -S uv run --script
"""Tests for the libobs build-phase recorder (#3257).

    uv run python -m unittest ci/test_libobs_build_phases.py

rust/tools/benchmarks/libobs_build_phases.py records the module-cache
baseline (#3256). These tests cover its pure parts: reading the entries a
build appended to `.ninja_log`, attributing each one to a unity group or a
phase, and the edit cases the baseline has to cover.
"""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location(
    "libobs_build_phases", ROOT / "rust/tools/benchmarks/libobs_build_phases.py"
)
phases = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = phases
spec.loader.exec_module(phases)

LOG = """# ninja log v6
0\t100\t0\told.o\tabc
"""
APPENDED = (
    "10\t2010\t0\tlibobs/CMakeFiles/libobs-objects.dir/build/libobs.core+.h.c.o\th1\n"
    "20\t1520\t0\tlibobs/CMakeFiles/libobs-objects.dir/build/libobs.core+.h.c.o.d\th2\n"
    "30\t530\t0\tlibobs/CMakeFiles/libobs-objects.dir/build/libobs.util+.h.c.o\th3\n"
    "2100\t2600\t0\tlibobs/libobs.so.30\th4\n"
    "40\t9040\t0\tlibobs/libobs_rust-static.a\th5\n"
    "50\t60\t0\tlibobs/obsconfig.h\th6\n"
)


class NinjaLogTest(unittest.TestCase):
    def test_reads_only_the_entries_after_the_offset(self):
        with tempfile.TemporaryDirectory() as tmp:
            log = Path(tmp) / ".ninja_log"
            log.write_text(LOG, encoding="utf-8")
            offset = phases.log_offset(log)
            with log.open("a", encoding="utf-8") as handle:
                handle.write(APPENDED)
            entries = phases.read_entries(log, offset)
        self.assertEqual([e.output for e in entries][0], "libobs/CMakeFiles/libobs-objects.dir/build/libobs.core+.h.c.o")
        self.assertEqual(len(entries), 6)
        self.assertAlmostEqual(entries[0].seconds, 2.0)

    def test_missing_log_has_offset_zero(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(phases.log_offset(Path(tmp) / ".ninja_log"), 0)


class AttributionTest(unittest.TestCase):
    def test_groups_link_and_rust_are_separate_phases(self):
        entries = [phases.Entry(*line.split("\t")[:4]) for line in APPENDED.splitlines()]
        summary = phases.summarize(entries)
        self.assertEqual(summary["groups"], {"core": 2.0, "util": 0.5})
        self.assertAlmostEqual(summary["link_seconds"], 0.5)
        self.assertAlmostEqual(summary["rust_seconds"], 9.0)
        self.assertEqual(summary["compiled_groups"], 2)
        # Dependency files and generated headers are not compiles.
        self.assertAlmostEqual(summary["compile_seconds"], 2.5)


class CaseTest(unittest.TestCase):
    def test_every_edit_case_names_a_tracked_file(self):
        labels = [case.label for case in phases.cases(ROOT)]
        self.assertEqual(labels, ["no-change", "c-h", "pch", "ported-json", "rust-crate"])
        for case in phases.cases(ROOT):
            if case.path is not None:
                self.assertTrue((ROOT / case.path).is_file(), case.path)

    def test_edit_is_reverted_byte_for_byte(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "x.json"
            path.write_bytes(b'{"a": 1}\n')
            with phases.edited(path, "json"):
                self.assertNotEqual(path.read_bytes(), b'{"a": 1}\n')
            self.assertEqual(path.read_bytes(), b'{"a": 1}\n')


if __name__ == "__main__":
    unittest.main()
