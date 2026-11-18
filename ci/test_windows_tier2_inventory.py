#!/usr/bin/env -S uv run --script
"""Extensionless Windows commands retain their entire CTest records on Linux."""

import json
from pathlib import Path
import shutil
import struct
import tempfile
import unittest

from test_windows_zip_check import pe
import windows_tier2_inventory as helper


@unittest.skipUnless(shutil.which("ctest") and shutil.which("cmake"), "needs CMake/CTest")
class InventoryTests(unittest.TestCase):
    def fixture(self, root, data):
        tests = root / "test/cmocka"
        tests.mkdir(parents=True)
        (tests / "legacy.exe").write_bytes(data)
        (root / "CTestTestfile.cmake").write_text('subdirs("test/cmocka")\n')
        (tests / "CTestTestfile.cmake").write_text(
            f'add_test(legacy "{tests}/legacy" "two words" "a;b")\n'
            'set_tests_properties(legacy PROPERTIES TIMEOUT 17 RUN_SERIAL TRUE RESOURCE_LOCK graphics)\n'
            f'add_test(missing "{tests}/missing")\n'
        )

    def test_resolves_only_existing_pe_and_preserves_args_and_properties(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, pe([]))
            result = helper.inventory(root, {"names": ["legacy", "missing"], "revision": "sha", "job": 1})
            self.assertEqual(["missing"], result["unbuilt_tests"])
            legacy = next(t for t in result["tests"] if t["name"] == "legacy")
            self.assertEqual([str(root / "test/cmocka/legacy.exe"), "two words", "a;b"], legacy["command"])
            props = {p["name"]: p["value"] for p in legacy["properties"]}
            self.assertEqual(17, props["TIMEOUT"])
            self.assertTrue(props["RUN_SERIAL"])
            self.assertEqual(["graphics"], props["RESOURCE_LOCK"])
            self.assertFalse(result["complete"])

    def test_rejects_wrong_architecture_or_non_pe_command(self):
        bad = bytearray(pe([]))
        struct.pack_into("<H", bad, 0x44, 0xAA64)
        for data in (bytes(bad), b"not a PE"):
            with self.subTest(data=data[:2]), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.fixture(root, data)
                with self.assertRaisesRegex(ValueError, "AMD64 PE"):
                    helper.inventory(root, {"names": ["legacy"], "revision": "sha", "job": 1})

    def test_rejects_resolution_outside_the_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, pe([]))
            outside = root / "outside.exe"
            outside.write_bytes(pe([]))
            build = root / "test/cmocka"
            (build / "CTestTestfile.cmake").write_text(f'add_test(escape "{root}/outside")\n')
            with self.assertRaisesRegex(ValueError, "outside"):
                helper.inventory(build, {"names": ["escape"], "revision": "sha", "job": 1})


if __name__ == "__main__":
    unittest.main()
