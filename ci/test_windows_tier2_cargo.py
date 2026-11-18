#!/usr/bin/env -S uv run --script
"""The Cargo producer must select the real soldr MSVC frontend explicitly."""

import importlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch


class CargoProducerTests(unittest.TestCase):
    def test_timeout_is_recorded_and_old_success_is_not_reused(self):
        producer = importlib.import_module("windows_tier2_cargo")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.object(
                    producer.subprocess,
                    "run",
                    side_effect=subprocess.TimeoutExpired("probe", 3600),
                ),
                self.assertRaisesRegex(RuntimeError, "124"),
            ):
                producer.run(["probe"], root, root, {}, "timeout")
            receipt = json.loads((root / "timeout.json").read_text())
            self.assertEqual(124, receipt["exit_code"])
            self.assertTrue(receipt["timed_out"])
            (root / "cargo-receipt.json").write_text('{"passed": true}')
            with self.assertRaisesRegex(ValueError, "refusing to reuse"):
                producer.produce(root, root, root, root)

    def test_failed_preparation_records_status_and_stops(self):
        producer = importlib.import_module("windows_tier2_cargo")
        cmake = shutil.which("cmake")
        if not cmake:
            self.skipTest("CMake is unavailable")
        with tempfile.TemporaryDirectory(prefix="cargo proof ") as directory:
            root = Path(directory)
            with self.assertRaisesRegex(RuntimeError, "probe failed"):
                producer.run([cmake, "-E", "false"], root, root, {}, "probe")
            receipt = json.loads((root / "probe.json").read_text())
            self.assertNotEqual(0, receipt["exit_code"])
            self.assertTrue((root / "probe.log").is_file())

    def test_selected_soldr_provisions_target_qualified_cl_tools(self):
        producer = importlib.import_module("windows_tier2_cargo")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            soldr = root / "soldr"
            soldr.write_bytes(b"selected soldr")
            soldr.chmod(0o755)
            driver = root / "x86_64-pc-windows-msvc-clang-cl"
            tools = {
                "cc": driver,
                "ar": root / "llvm-lib",
                "rc": root / "llvm-rc",
                "sysroot": root / "sdk",
            }
            for path in tools.values():
                path.write_bytes(b"tool")
            calls = []

            def query(command, **kwargs):
                calls.append(command)
                return str(tools[command[-1].removeprefix("--print-")]) + "\n"

            with patch.object(producer.subprocess, "check_output", side_effect=query):
                env, receipt = producer.toolchain(str(soldr), root / "output")
            self.assertEqual(str(driver), env["CC_x86_64_pc_windows_msvc"])
            self.assertEqual(str(driver), env["CXX_x86_64_pc_windows_msvc"])
            self.assertEqual(str(tools["ar"]), env["AR_x86_64_pc_windows_msvc"])
            self.assertNotIn("CC", env)
            self.assertEqual("clang-cl", receipt["frontend"])
            self.assertEqual(producer.digest(soldr), receipt["soldr_sha256"])
            self.assertTrue(all(command[0] == str(soldr) for command in calls))
            self.assertIn("--driver-mode=cl", calls[0])


if __name__ == "__main__":
    unittest.main()
