#!/usr/bin/env -S uv run --script
"""Contracts for the opt-in Windows Tier 2 producer, with no Windows runner."""

import importlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from test_windows_zip_check import pe

ROOT = Path(__file__).resolve().parents[1]


class ProbeTests(unittest.TestCase):
    def helper(self):
        return importlib.import_module("windows_tier2_probe")

    def test_wrong_target_fails_before_toolchain_setup(self):
        with tempfile.TemporaryDirectory() as tmp:
            result = subprocess.run(
                ["cmake", "-S", str(ROOT / "cmake/cross"), "-B", tmp,
                 "-DOBS_CROSS_WINDOWS_TIER2=ON", "-DOBS_CROSS_TARGET=mac-arm64"],
                capture_output=True, text=True, check=False, timeout=10,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("OBS_CROSS_WINDOWS_TIER2 requires win-x64", result.stdout + result.stderr)

    def test_pe_architecture_and_imports(self):
        helper = self.helper()
        self.assertEqual(helper.image(pe(["kernel32.dll"])), ["kernel32.dll"])
        data = bytearray(pe([]))
        struct.pack_into("<H", data, 0x44, 0xAA64)
        with self.assertRaisesRegex(ValueError, "AMD64"):
            helper.image(data)
        with self.assertRaises(ValueError):
            helper.image(b"MZ")

    def test_symbol_parity_rejects_missing_extra_and_empty(self):
        helper = self.helper()
        self.assertEqual(helper.parity({"a", "b"}, {"b", "a"}), ["a", "b"])
        for left, right in [({"a"}, {"b"}), (set(), set())]:
            with self.assertRaises(ValueError):
                helper.parity(left, right)

    def test_transitive_dll_closure_and_missing_runtime(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / "test.exe"
            exe.write_bytes(pe(["one.dll", "kernel32.dll"]))
            (root / "one.dll").write_bytes(pe(["two.dll"]))
            (root / "two.dll").write_bytes(pe(["vcruntime140.dll"]))
            with self.assertRaisesRegex(ValueError, "vcruntime140.dll"):
                helper.closure([exe], [root])
            (root / "vcruntime140.dll").write_bytes(pe([]))
            self.assertEqual(len(helper.closure([exe], [root])), 4)

    def test_registry_relocation_and_corruption(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build, output = root / "producer", root / "bundle"
            build.mkdir()
            exe = build / "test.exe"
            exe.write_bytes(pe(["kernel32.dll"]))
            (build / "CTestTestfile.cmake").write_text(
                f'add_test(example "{exe}")\nset_tests_properties(example PROPERTIES TIMEOUT "12")\n'
            )
            with patch.object(helper, "exports", return_value={"api"}):
                manifest = helper.stage(build, output, root, [exe], [root], "llvm-readobj")
            self.assertFalse(manifest["complete"])
            helper.prepare(output)
            registry = (output / "build/CTestTestfile.cmake").read_text()
            self.assertNotIn(str(build), registry)
            self.assertIn('TIMEOUT "12"', registry)
            self.assertEqual(json.loads((output / "manifest.json").read_text())["coverage"], "probe-only")
            (output / "build/test.exe").write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "changed artifact"):
                helper.prepare(output)

    def test_manifest_cannot_escape_bundle(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "manifest.json").write_text(json.dumps({
                "schema": 1, "target": helper.TARGET, "files": {"../escape": "abc"},
                "ctest_templates": {},
            }))
            with self.assertRaisesRegex(ValueError, "escapes"):
                helper.prepare(root)


if __name__ == "__main__":
    unittest.main()
