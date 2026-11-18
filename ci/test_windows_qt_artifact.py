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
import windows_native_fixtures as native
from test_windows_test_artifact import executable


def dll(dependencies=("KERNEL32.dll",)):
    data = bytearray(executable(dependencies))
    struct.pack_into("<H", data, 0x56, 0x2022)
    return bytes(data)


def delayed_executable(name):
    data = bytearray(executable(["KERNEL32.dll"]))
    offset = len(data)
    rva = 0x1000 + offset - 0x200
    data.extend(struct.pack("<8I", 1, rva + 64, 0, 0, 0, 0, 0, 0))
    data.extend(bytes(32) + name.encode() + b"\0")
    struct.pack_into("<II", data, 0x40 + 24 + 112 + 13 * 8, rva, 64)
    section = 0x40 + 24 + 240
    struct.pack_into("<I", data, section + 8, len(data) - 0x200)
    struct.pack_into("<I", data, section + 16, len(data) - 0x200)
    return bytes(data)


class WindowsQtTest(unittest.TestCase):
    def test_cross_cpp_driver_preserves_msvc_exception_support(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sdk = root / "sdk"
            sdk.mkdir()
            (sdk / "soldr-case-overlay-test.yaml").write_text("overlay")
            tools = root / "tools"
            tools.mkdir()
            for directory in ("crt/lib/x86_64", "sdk/lib/um/x86_64", "sdk/lib/ucrt/x86_64"):
                (sdk / directory).mkdir(parents=True)

            def tool(option, *args, **kwargs):
                return str(sdk) if option == "--print-sysroot" else "/managed/driver"

            with mock.patch.object(native.shared, "tool", side_effect=tool), \
                    mock.patch.object(native.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "/managed/clang")):
                native.configure_tools(root, tools, {})
            self.assertIn("/EHsc", (tools / "cxx-clang-cl").read_text())
            self.assertNotIn("/EHsc", (tools / "cc-clang-cl").read_text())

    def test_matches_current_windows_job_and_linux_generators(self):
        workflow = (qt.ROOT / ".github/workflows/build-project.yaml").read_text()
        self.assertIn(qt.TARGET_ASSET, workflow)
        self.assertIn(qt.TARGET_SHA, workflow)
        self.assertEqual(qt.CONTRACTS, shared.CONTRACTS)
        self.assertEqual(qt.HOST_ASSET, shared.HOST_ASSET)
        # Exact OS imports observed in the pinned Qt Core/Gui/Widgets binaries.
        system = ["AUTHZ.dll", "MPR.dll", "D3D11.dll", "D3D12.dll", "DWrite.dll", "DwmApi.dll", "UxTheme.dll", "ICUUC.dll"]
        self.assertEqual(artifact.pe_imports(executable(system)), sorted(name.lower() for name in system))

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
        for name in ("VCRUNTIME140D.dll", "MSVCP140D.dll", "ucrtbased.dll"):
            with self.subTest(debug=name), self.assertRaisesRegex(ValueError, "debug.*runtime"):
                artifact.pe_imports(executable([name]), staged_dlls={name.lower()})
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "answer.exe").write_bytes(delayed_executable("Qt6Core.dll"))
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
            tools = root / "work/host-qt/libexec"
            tools.mkdir(parents=True)
            for name in ("moc", "uic", "rcc"):
                (tools / name).write_bytes(b"host generator")
            with mock.patch.object(qt.platform, "system", return_value="Linux"), mock.patch.object(qt, "fetch"), \
                    mock.patch.object(qt, "configure_tools", return_value=({"CMAKE_GENERATOR": "Unix Makefiles"}, {})), \
                    mock.patch.object(qt, "cmocka", return_value=root / "cmocka"), \
                    mock.patch.object(qt, "append_bundle"), \
                    mock.patch.object(qt.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "6.11.1", "")) as run:
                qt.build(root / "work", root / "bundle")
            command = run.call_args
            self.assertIn('env.CMAKE_GENERATOR.value="Ninja"', command.args[0])
            self.assertIn('env.CMAKE_GENERATOR.force=true', command.args[0])
            self.assertNotIn("CMAKE_GENERATOR", command.kwargs["env"])
            self.assertTrue(set(qt.CONTRACTS).issubset(command.args[0]))
            self.assertEqual(command.kwargs["env"]["OBS_QT_CONTRACT_TARGET"], artifact.TARGET)
            self.assertNotIn("STATIC_LIBRARY", (root / "work/producer-tools/windows.cmake").read_text())

    def test_incomplete_or_unclaimed_qt_payload_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for value in ([], [qt.CONTRACTS[0]]):
                with self.assertRaisesRegex(ValueError, "incomplete.*Qt"):
                    qt.validate({"qt_contracts_complete": value}, root)
            (root / "manifest.json").write_text(json.dumps({"qt_contracts_complete": list(qt.CONTRACTS)}))
            with self.assertRaises((ValueError, KeyError)):
                qt.validate({"qt_contracts_complete": list(qt.CONTRACTS), "tests": []}, root)

    def test_shared_bundle_preserves_every_runtime_entry_and_checks_dll_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle, work = root / "bundle", root / "work"
            contracts = work / "contracts"
            contracts.mkdir(parents=True)
            binary = contracts / "answer.exe"
            binary.write_bytes(executable(["Qt6Core.dll"]))
            entries = [{"contract": name, "name": "answer", "binary": str(binary), "cwd": str(contracts),
                        "env": {}, "args": [str(contracts / "resource")], "timeout": 19} for name in qt.CONTRACTS]
            (contracts / "records.jsonl").write_text("".join(json.dumps(entry) + "\n" for entry in entries))
            (work / "inputs.json").write_text(json.dumps({"pkg_config": {"version": "test"}}))
            bundle.mkdir()
            artifact.shared.finalize(bundle, {"version": 1, "target": qt.TARGET,
                "build_root": str(shared.ROOT), "coverage": artifact.PREPARED_COVERAGE,
                "doctests_complete": True, "tests": [], "images": {}, "runtime_dlls": []})

            def stage(sdk, prefix, destination, required):
                for name in ("bin/Qt6Core.dll", "plugins/platforms/qminimal.dll"):
                    path = destination / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(dll())

            with mock.patch.object(qt, "stage_runtime", side_effect=stage):
                qt.append_bundle(bundle, work, root / "sdk")
            data = artifact.verify(bundle / "manifest.json")
            self.assertEqual(len(data["tests"]), 4)
            self.assertEqual(data["tests"][0]["timeout"], 19)
            self.assertIn("{root}/target/windows-qt/contracts/resource", data["tests"][0]["args"])
            # An unrelated Rust/helper image has no Qt loader PATH.
            (bundle / "helper.exe").write_bytes(executable(["Qt6Core.dll"]))
            data["images"]["helper.exe"] = ["qt6core.dll"]
            artifact.shared.finalize(bundle, data)
            with self.assertRaisesRegex(ValueError, "runtime dependency"):
                artifact.verify(bundle / "manifest.json")
            (bundle / qt.RELATIVE / "qt/bin/Qt6Core.dll").write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "checksum"):
                artifact.verify(bundle / "manifest.json")


if __name__ == "__main__":
    unittest.main()
