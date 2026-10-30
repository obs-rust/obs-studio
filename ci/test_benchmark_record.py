#!/usr/bin/env -S uv run --script
"""Tests for the paired C/Rust benchmark recorder (#142, #3003).

    uv run python -m unittest ci/test_benchmark_record.py

record.py reads Criterion's output for every pair in the port_comparison
bench. Each port adds its own bench groups (#1997), so the inventory check
asks for the required categories and allows the others. The SIMD
candidates of #3009 (format conversion, volume meter, audio mix) are
required, so a baseline always has the numbers #3004 to #3006 compare with.
"""

from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Loaded by path: ci/ tests import only the standard library and ci/
# modules (test_ci_tests_stdlib_only.py).
spec = importlib.util.spec_from_file_location("record", ROOT / "rust/tools/benchmarks/record.py")
record = importlib.util.module_from_spec(spec)
spec.loader.exec_module(record)

BASELINE = "ports-test-1"
SIMD = {"format", "volmeter", "audio_mix"}


def write_pair(directory: Path, group: str, value: str | None = None) -> None:
    for backend in ("rust", "c"):
        parts = [group, backend] + ([value] if value else []) + [BASELINE]
        folder = directory.joinpath(*parts)
        folder.mkdir(parents=True)
        benchmark = {"group_id": group, "function_id": backend, "value_str": value}
        (folder / "benchmark.json").write_text(json.dumps(benchmark))
        median = {"point_estimate": 10.0, "confidence_interval": {"lower_bound": 9.0, "upper_bound": 11.0}}
        (folder / "estimates.json").write_text(json.dumps({"median": median}))


def inventory(directory: Path, categories: set[str], extra: list[str]) -> None:
    groups = [f"{category}/case" for category in sorted(categories)] + extra
    groups += [f"port_unit_{index}/case" for index in range(max(0, 80 - len(groups)))]
    for group in groups:
        write_pair(directory, group)


class InventoryTest(unittest.TestCase):
    def test_port_groups_next_to_the_required_categories_are_recorded(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            inventory(directory, record.CATEGORIES, ["source_render_route/filter", "canvas_rules/draw"])
            entries = record.read_run(directory, BASELINE)
        self.assertIn("source_render_route/filter", entries)
        self.assertTrue(record.CATEGORIES <= {key.split("/")[0] for key in entries})

    def test_the_simd_candidates_are_required(self):
        self.assertTrue(SIMD <= record.CATEGORIES)
        for missing in sorted(SIMD):
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as temp:
                directory = Path(temp)
                inventory(directory, record.CATEGORIES - {missing}, [])
                with self.assertRaisesRegex(ValueError, missing):
                    record.read_run(directory, BASELINE)

    def test_a_pair_without_its_c_side_is_refused(self):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            inventory(directory, record.CATEGORIES, [])
            shutil.rmtree(directory / "format/case" / "c")
            with self.assertRaisesRegex(ValueError, "missing comparison for format/case"):
                record.read_run(directory, BASELINE)


class SourceHashTest(unittest.TestCase):
    def test_every_rust_crate_is_in_the_fingerprint(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "Cargo.toml").write_text("[workspace]\n")
            (root / "Cargo.lock").write_text("")
            source = root / "rust/obs-media-io/src/format_conversion.rs"
            source.parent.mkdir(parents=True)
            source.write_text("fn before() {}\n")
            saved = record.ROOT
            record.ROOT = root
            try:
                before = record.source_hash()
                source.write_text("fn after() {}\n")
                after = record.source_hash()
            finally:
                record.ROOT = saved
        self.assertNotEqual(before, after)


class CpuNameTest(unittest.TestCase):
    def test_macos_names_the_chip(self):
        # platform.processor() is only "arm" on Apple Silicon; the baseline
        # names the CPU (#3003), so macOS asks sysctl.
        calls = []

        def run(command, **kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, stdout="Apple M1 Max\n", stderr="")

        saved = (record.platform.system, record.subprocess.run)
        record.platform.system, record.subprocess.run = (lambda: "Darwin"), run
        try:
            name = record.cpu_name()
        finally:
            record.platform.system, record.subprocess.run = saved
        self.assertEqual(name, "Apple M1 Max")
        self.assertEqual(calls, [["sysctl", "-n", "machdep.cpu.brand_string"]])


class WindowsCpuNameTest(unittest.TestCase):
    def test_windows_names_the_chip(self):
        # platform.processor() is "AMD64 Family 25 Model 33 ..." on Windows;
        # the baseline names the CPU (#3025), so Windows asks the registry.
        opened = []

        class Key:
            def __enter__(self):
                return self

            def __exit__(self, *exc):
                return False

        winreg = types.ModuleType("winreg")
        winreg.HKEY_LOCAL_MACHINE = "HKLM"

        def open_key(root, path):
            opened.append((root, path))
            return Key()

        winreg.OpenKey = open_key
        winreg.QueryValueEx = lambda key, name: ("AMD Ryzen 9 5950X 16-Core Processor  ", 1)
        saved = (record.platform.system, sys.modules.get("winreg"))
        record.platform.system = lambda: "Windows"
        sys.modules["winreg"] = winreg
        try:
            name = record.cpu_name()
        finally:
            record.platform.system = saved[0]
            if saved[1] is None:
                del sys.modules["winreg"]
            else:
                sys.modules["winreg"] = saved[1]
        self.assertEqual(name, "AMD Ryzen 9 5950X 16-Core Processor")
        self.assertEqual(opened, [("HKLM", r"HARDWARE\DESCRIPTION\System\CentralProcessor\0")])


class CommandTest(unittest.TestCase):
    def test_filter_selects_whole_categories(self):
        command = record.bench_command(["format", "audio_mix"])
        self.assertEqual(command[:7], ["soldr", "cargo", "bench", "-p", "obs-c-oracle", "--bench", "port_comparison"])
        self.assertEqual(command[-1], "^(format|audio_mix)/")

    def test_no_filter_runs_every_pair(self):
        self.assertEqual(record.bench_command([])[-1], "--")


if __name__ == "__main__":
    unittest.main()
