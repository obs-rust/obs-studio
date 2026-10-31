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

import contextlib
import importlib.util
import io
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


class CFlagsTest(unittest.TestCase):
    """#3127: the bench C is built at the level the shipped libobs C gets."""

    def test_the_oracle_c_is_clamped_to_the_shipped_level(self):
        build_rs = (ROOT / "rust/obs-c-oracle/build.rs").read_text()
        self.assertIn("const SHIPPED_C_OPT_LEVEL: u32 = 2;", build_rs)
        self.assertIn("oracle.opt_level(SHIPPED_C_OPT_LEVEL)", build_rs)
        # The bench profile is opt-level 3; the shipped C is -O2. A -O3 oracle
        # vectorizes audio loops the shipped C leaves scalar (#3127).
        helper = build_rs.split("fn shipped_c_opt_level", 1)[1].split("\n}", 1)[0]
        self.assertNotIn("-O3", helper)

    def test_the_recorded_flags_name_the_shipped_level(self):
        self.assertTrue(record.C_FLAGS.startswith("-O2"))
        self.assertIn("-ffp-contract=off", record.C_FLAGS)
        self.assertNotIn("-O3", record.C_FLAGS)

    def test_the_report_carries_the_c_flags(self):
        report = record.report_metadata(
            timestamp="20260101T000000Z",
            fingerprint="0" * 64,
            cpu="test cpu",
            required={"format"},
            command=["true"],
            options=[],
            baselines=["ports-1"],
        )
        self.assertEqual(report["c_flags"], record.C_FLAGS)
        self.assertNotIn("-O3", report["c_flags"])


class CommandTest(unittest.TestCase):
    def test_filter_selects_whole_categories(self):
        command = record.bench_command(["format", "audio_mix"])
        self.assertEqual(command[:7], ["soldr", "cargo", "bench", "-p", "obs-c-oracle", "--bench", "port_comparison"])
        self.assertEqual(command[-1], "^(format|audio_mix)/")

    def test_no_filter_runs_every_pair(self):
        self.assertEqual(record.bench_command([])[-1], "--")


class BoundGateTest(unittest.TestCase):
    """`--max-ratio` turns a recorded over-bound pair into a failed run (#3138).

    The recording is a local check (the box is shared), so the gate belongs in
    the recorder, not in PR CI: it lets a worker or maintainer reproduce the
    bound breach with one command instead of reading the report by eye.
    """

    def test_a_pair_over_the_bound_is_reported(self):
        rows = [
            {"benchmark": "audio_mix/mix_whole_tick", "rust_over_c": 1.26,
             "status": "over 10% bound"},
        ]
        self.assertEqual(
            [row["benchmark"] for row in record.over_bound(rows)],
            ["audio_mix/mix_whole_tick"],
        )

    def test_a_pair_within_the_bound_passes(self):
        rows = [{"benchmark": "audio_mix/mix_whole_tick", "rust_over_c": 1.09,
                 "status": "within 10% bound"}]
        self.assertEqual(record.over_bound(rows), [])

    def test_the_c_printf_control_is_never_a_breach(self):
        rows = [{"benchmark": "dstr/printf_control/128", "rust_over_c": 12.0,
                 "status": "C control in both builds"}]
        self.assertEqual(record.over_bound(rows), [])


class BoundExemptionTest(unittest.TestCase):
    """#3138 decided: `audio_mix/buffering_add_one_tick` is exempt by name.

    The pair is one `extern "C"` shim on a once-per-audio-tick call at ~46 ns,
    with no hot spot left after #3141, so the ratio bound does not apply to it
    while the 5% full-app bound of #142 still does. The exemption must be a
    named entry that keeps the pair in the report.
    """

    PAIR = "audio_mix/buffering_add_one_tick"

    def test_the_exempt_pair_is_named_with_its_cost_and_issue(self):
        entry = record.BOUND_EXEMPTIONS[self.PAIR]
        self.assertIn("#3138", entry)
        self.assertIn("ns", entry)
        self.assertIn("Ryzen 7 3700X", entry)

    def test_an_exempt_pair_is_not_a_breach(self):
        rows = [{"benchmark": self.PAIR, "rust_over_c": 1.26, "status": "over 10% bound"}]
        self.assertEqual(record.over_bound(rows), [])

    def test_the_gate_passes_an_exempt_breach_and_names_it(self):
        rows = [{"benchmark": self.PAIR, "rust_over_c": 1.21,
                 "status": "exempt from 10% bound (#3138)"}]
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            record.enforce_bound(rows, 1.10)
        self.assertIn(self.PAIR, output.getvalue())
        self.assertIn("exempt", output.getvalue())

    def test_the_gate_still_fails_every_other_breach(self):
        rows = [{"benchmark": "audio_mix/mix_whole_tick", "rust_over_c": 1.21,
                 "status": "over 10% bound"}]
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit):
            record.enforce_bound(rows, 1.10)

    def test_an_exempt_pair_stays_in_the_report(self):
        runs = [
            {"format/case": {"rust": {"median_ns": 1.0}, "c": {"median_ns": 1.0}},
             self.PAIR: {"rust": {"median_ns": 46.0}, "c": {"median_ns": 39.0}}},
            {"format/case": {"rust": {"median_ns": 1.0}, "c": {"median_ns": 1.0}},
             self.PAIR: {"rust": {"median_ns": 47.0}, "c": {"median_ns": 39.0}}},
        ]
        row = {row["benchmark"]: row for row in record.summarize(runs)}[self.PAIR]
        self.assertAlmostEqual(row["rust_over_c"], 46.5 / 39.0)
        self.assertIn("exempt", row["status"])
        self.assertIn("#3138", row["status"])


if __name__ == "__main__":
    unittest.main()
