#!/usr/bin/env -S uv run --script
"""Windows Qt preparation retains the shared transport and loader closure."""
import json
import struct
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import macos_qt_artifact as shared
import windows_qt_artifact as qt
import windows_test_artifact as artifact
from test_windows_test_artifact import executable


def dll(dependencies=("KERNEL32.dll",)):
    data = bytearray(executable(dependencies))
    struct.pack_into("<H", data, 0x56, 0x2022)
    return bytes(data)


class WindowsQtTest(unittest.TestCase):
    def test_matches_current_windows_job_and_linux_generators(self):
        workflow = (qt.ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertIn(qt.TARGET_ASSET, workflow)
        self.assertIn(qt.TARGET_SHA, workflow)
        self.assertEqual(qt.CONTRACTS, shared.CONTRACTS)
        self.assertEqual(qt.HOST_ASSET, shared.HOST_ASSET)

    def test_stage_follows_transitive_imports_and_requires_minimal_plugin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sdk, prefix, destination = (root / name for name in ("sdk", "cmocka", "runtime"))
            files = {"bin/Qt6Core.dll": dll(["KERNEL32.dll", "helper.dll"]),
                     "bin/helper.dll": dll(), "bin/Qt6Gui.dll": dll(["Qt6Core.dll"]),
                     "bin/Qt6Widgets.dll": dll(["Qt6Gui.dll"]),
                     "plugins/platforms/qminimal.dll": dll(["Qt6Gui.dll"])}
            for name, data in files.items():
                path = sdk / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            (prefix / "bin").mkdir(parents=True)
            (prefix / "bin/cmocka.dll").write_bytes(dll())
            qt.stage_runtime(sdk, prefix, destination, {"qt6widgets.dll", "cmocka.dll"})
            self.assertTrue((destination / "bin/helper.dll").is_file())
            self.assertTrue((destination / "plugins/platforms/qminimal.dll").is_file())
            (sdk / "bin/helper.dll").write_bytes(dll(["missing.dll"]))
            with self.assertRaisesRegex(ValueError, "missing.*missing.dll"):
                qt.stage_runtime(sdk, prefix, root / "bad", {"qt6widgets.dll"})

    def test_closure_verification_rejects_unstaged_delay_import(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "answer.exe").write_bytes(executable(["Qt6Core.dll"]))
            with self.assertRaisesRegex(ValueError, "runtime dependency"):
                qt.image_inventory(root)
            (root / "Qt6Core.dll").write_bytes(dll())
            self.assertEqual(len(qt.image_inventory(root)), 2)
            (root / "Qt6Core.dll").write_bytes(dll(["absent.dll"]))
            with self.assertRaisesRegex(ValueError, "runtime dependency"):
                qt.image_inventory(root)

    def test_preparation_forces_ninja_after_managed_tool_setup(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with mock.patch.object(qt, "fetch"), \
                    mock.patch.object(qt, "configure_tools", return_value=({"CMAKE_GENERATOR": "Unix Makefiles"}, {})), \
                    mock.patch.object(qt, "cmocka", return_value=root / "cmocka"), \
                    mock.patch.object(qt, "append_bundle"), \
                    mock.patch.object(qt.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "6.11.1")) as run:
                qt.build(root / "work", root / "bundle")
            command = run.call_args
            self.assertIn('env.CMAKE_GENERATOR.value="Ninja"', command.args[0])
            self.assertIn('env.CMAKE_GENERATOR.force=true', command.args[0])
            self.assertNotIn("CMAKE_GENERATOR", command.kwargs["env"])
            self.assertTrue(set(qt.CONTRACTS).issubset(command.args[0]))
            self.assertEqual(command.kwargs["env"]["OBS_QT_CONTRACT_TARGET"], artifact.TARGET)

    def test_incomplete_or_unclaimed_qt_payload_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for value in ([], [qt.CONTRACTS[0]]):
                with self.assertRaisesRegex(ValueError, "incomplete.*Qt"):
                    qt.validate({"qt_contracts_complete": value}, root)
            (root / "manifest.json").write_text(json.dumps({"qt_contracts_complete": list(qt.CONTRACTS)}))
            with self.assertRaises((ValueError, KeyError)):
                qt.validate({"qt_contracts_complete": list(qt.CONTRACTS), "tests": []}, root)


if __name__ == "__main__":
    unittest.main()
