#!/usr/bin/env -S uv run --script
"""Run-only Qt bundle regressions; no SDK download in self-tests."""
import json
import hashlib
import os
import struct
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import macos_qt_artifact as qt
import test_macos_test_artifact as artifact_tests


class QtArtifactTest(unittest.TestCase):
    def test_producer_pins_ninja_for_colon_bearing_contract_paths(self):
        import local_gate

        for inherited in (None, "Unix Makefiles"):
            with self.subTest(inherited=inherited), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                environment = {} if inherited is None else {"CMAKE_GENERATOR": inherited}
                with mock.patch.object(local_gate, "soldr_env", return_value=environment), \
                        mock.patch.object(qt, "fetch"), mock.patch.object(qt, "toolchain"), \
                        mock.patch.object(qt, "cmocka", return_value=root / "cmocka"), \
                        mock.patch.object(qt, "append_bundle"), \
                        mock.patch.object(qt.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "/soldr/cc\n")) as run:
                    qt.build(root / "work", root / "bundle")
                command = run.call_args
                self.assertIn("--config", command.args[0])
                self.assertIn('env.CMAKE_GENERATOR.value="Ninja"', command.args[0])
                self.assertIn('env.CMAKE_GENERATOR.force=true', command.args[0])
                self.assertNotIn("CMAKE_GENERATOR", command.kwargs["env"])
                self.assertIn("--exact", command.args[0])
                self.assertTrue(set(qt.CONTRACTS).issubset(command.args[0]))

    def test_runtime_stages_minimal_and_offscreen_for_unchanged_darwin_tests(self):
        with tempfile.TemporaryDirectory() as temporary:
            source, destination = Path(temporary) / "sdk", Path(temporary) / "runtime"
            names = [f"lib/Qt{module}.framework/Versions/A/Qt{module}" for module in ("Core", "Gui", "Widgets")]
            names += ["plugins/platforms/libqminimal.dylib", "plugins/platforms/libqoffscreen.dylib"]
            for name in names:
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(artifact_tests.SignatureTest.signed_image())
            with mock.patch.object(qt.artifact, "verify_macho") as verify, mock.patch.object(qt, "audit_linkage") as audit:
                qt.stage_qt(source, destination)
            self.assertEqual(verify.call_count, 5)
            self.assertEqual(audit.call_count, 5)
            for name in names:
                self.assertEqual((destination / name).read_bytes(), (source / name).read_bytes())
                self.assertTrue(os.access(destination / name, os.X_OK))

    def test_apple_linker_zero_padding_and_dylib_signatures(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "QtCore"
            image = bytearray(artifact_tests.SignatureTest.signed_image())
            image.extend(b"\0" * 8)
            struct.pack_into("<I", image, 12, 6)
            struct.pack_into("<I", image, 44, len(image) - 4096)
            image[4096 + 20 + 44:4096 + 20 + 76] = hashlib.sha256(image[:4096]).digest()
            path.write_bytes(image)
            qt.artifact.verify_macho(path, allowed_filetypes=(6,))
            with self.assertRaisesRegex(ValueError, "not an arm64"):
                qt.artifact.verify_macho(path)
            image[-1] = 1
            path.write_bytes(image)
            with self.assertRaisesRegex(ValueError, "invalid signature superblob"):
                qt.artifact.verify_macho(path, allowed_filetypes=(6,))

    def test_ctest_records_preserve_runtime_contract(self):
        inventory = {"tests": [{"name": "answer", "command": ["/work/answer", "42"],
                                "properties": [{"name": "WORKING_DIRECTORY", "value": "/work"},
                                               {"name": "ENVIRONMENT", "value": ["QT_QPA_PLATFORM=minimal"]},
                                               {"name": "TIMEOUT", "value": 60.0}]}]}
        entries = qt.ctest_entries(inventory)
        self.assertEqual(entries, [{"name": "answer", "binary": "/work/answer", "args": ["42"],
                                    "cwd": "/work", "env": {"QT_QPA_PLATFORM": "minimal"}, "timeout": 60.0}])
        inventory["tests"][0]["properties"].append({"name": "ENVIRONMENT_MODIFICATION", "value": [
            "DYLD_FRAMEWORK_PATH=path_list_prepend:/qt/lib"]})
        self.assertEqual(qt.ctest_entries(inventory)[0]["env_path_prepend"], {"DYLD_FRAMEWORK_PATH": ["/qt/lib"]})
        inventory["tests"][0]["properties"].append({"name": "WILL_FAIL", "value": True})
        with self.assertRaisesRegex(ValueError, "unsupported execution property"):
            qt.ctest_entries(inventory)
        with self.assertRaisesRegex(ValueError, "ENVIRONMENT_MODIFICATION"):
            qt.ctest_path_prepend(["PATH=set:/other"])

    def test_incomplete_contract_inventory_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            (work / "records.jsonl").write_text(json.dumps({"contract": qt.CONTRACTS[0], "binary": "/missing"}) + "\n")
            with self.assertRaisesRegex(ValueError, "incomplete Qt contracts"):
                qt.read_records(work)

    def test_runtime_path_translation_rejects_unmapped_producer_paths(self):
        mappings = {"/work": "{root}/target/macos-qt", "/checkout": "{root}"}
        self.assertEqual(qt.portable("/work/answer", mappings), "{root}/target/macos-qt/answer")
        self.assertEqual(qt.portable("/checkout/frontend", mappings), "{root}/frontend")
        with self.assertRaisesRegex(ValueError, "unmapped producer path"):
            qt.portable("/tmp/other/answer", mappings)

    def test_bundle_appends_all_contracts_with_cwd_env_args_and_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "checkout"
            root.mkdir()
            work = Path(temporary) / "work"
            contracts = work / "contracts"
            contracts.mkdir(parents=True)
            binary = contracts / "answer"
            binary.write_bytes(artifact_tests.SignatureTest.signed_image())
            entries = [{"name": "answer", "binary": str(binary), "cwd": str(contracts),
                        "env": {"RESOURCE": str(contracts / "data")}, "args": [str(contracts / "data")],
                        "timeout": 17, "contract": name} for name in qt.CONTRACTS]
            (contracts / "records.jsonl").write_text("".join(json.dumps(item) + "\n" for item in entries))
            bundle = Path(temporary) / "bundle"
            bundle.mkdir()
            (bundle / "manifest.json").write_text(json.dumps({"build_root": str(root), "tests": [],
                                                            "version": 1, "target": qt.TARGET,
                                                            "doctests_complete": True, "files": {}}))
            with mock.patch.object(qt, "ROOT", root), mock.patch.object(qt, "stage_qt"):
                qt.append_bundle(bundle, work, work / "qt")
            manifest = json.loads((bundle / "manifest.json").read_text())
            self.assertEqual(manifest["qt_contracts_complete"], list(qt.CONTRACTS))
            self.assertEqual(len(manifest["tests"]), 4)
            entry = manifest["tests"][0]
            self.assertEqual(entry["binary"], qt.RELATIVE + "/contracts/answer")
            self.assertEqual(entry["cwd"], qt.RELATIVE + "/contracts")
            self.assertEqual(entry["env"]["RESOURCE"], "{root}/" + qt.RELATIVE + "/contracts/data")
            self.assertEqual(entry["args"], [entry["env"]["RESOURCE"]])
            self.assertEqual(entry["timeout"], 17)
            self.assertEqual(manifest["files"][entry["binary"]], hashlib.sha256(binary.read_bytes()).hexdigest())
            with mock.patch.object(qt, "ROOT", root), self.assertRaisesRegex(ValueError, "already complete"):
                qt.append_bundle(bundle, work, work / "qt")

    def test_unbundled_load_paths_are_rejected(self):
        with mock.patch.object(qt, "load_paths", return_value=[(0x8000001C, "/producer/qt/lib")]):
            with self.assertRaisesRegex(ValueError, "unbundled Mach-O"):
                qt.audit_linkage(Path("answer"), Path("runtime"))

    def test_common_runner_preserves_timeout_and_inherited_loader_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "answer"
            binary.write_bytes(artifact_tests.SignatureTest.signed_image())
            manifest = root / "manifest.json"
            qt.artifact.finalize(root, {"version": 1, "target": qt.TARGET, "build_root": str(root),
                "doctests_complete": True, "tests": [{"binary": "answer", "cwd": ".", "package": "qt",
                    "args": ["{root}/resource"], "timeout": 17, "env": {},
                    "env_path_prepend": {"DYLD_FRAMEWORK_PATH": ["{root}/qt/lib"]}}]})
            with mock.patch.dict(os.environ, {"DYLD_FRAMEWORK_PATH": "/inherited"}), mock.patch.object(
                qt.artifact.subprocess, "run", return_value=subprocess.CompletedProcess([], 19)
            ) as run:
                self.assertEqual(qt.artifact.execute(manifest, root), 19)
            args, kwargs = run.call_args
            self.assertEqual(args[0], [str(binary), str(root / "resource")])
            self.assertEqual(kwargs["timeout"], 17)
            self.assertEqual(kwargs["env"]["DYLD_FRAMEWORK_PATH"], str(root / "qt/lib") + os.pathsep + "/inherited")


if __name__ == "__main__":
    unittest.main()
