#!/usr/bin/env -S uv run --script
"""Windows executable slice shares Darwin transport and verifies real PE contracts."""
from __future__ import annotations

import json
import struct
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import macos_test_artifact as shared
import windows_test_artifact as artifact
from test_windows_zip_check import pe


def executable(dlls=("KERNEL32.dll", "VCRUNTIME140.dll")):
    data = bytearray(pe(list(dlls)))
    struct.pack_into("<H", data, 0x40 + 22, 0x22)  # executable, large-address-aware
    struct.pack_into("<I", data, 0x40 + 24 + 16, 0x1000)  # entry point
    struct.pack_into("<H", data, 0x40 + 24 + 68, 3)  # console subsystem
    struct.pack_into("<I", data, 0x40 + 24 + 240 + 36, 0x60000020)  # executable code section
    return bytes(data)


class WindowsArtifactTest(unittest.TestCase):
    def test_pe_rejects_wrong_architecture_dll_non_pe_and_truncation(self):
        good = executable()
        self.assertEqual(artifact.pe_imports(good), ["kernel32.dll", "vcruntime140.dll"])
        self.assertEqual(artifact.pe_imports(executable(["DXGI.dll", "pdh.dll", "avrt.dll"])),
                         ["avrt.dll", "dxgi.dll", "pdh.dll"])
        arm = bytearray(good)
        struct.pack_into("<H", arm, 0x44, 0xAA64)
        dll = bytearray(good)
        struct.pack_into("<H", dll, 0x56, 0x2022)
        truncated_body = bytearray(good)
        struct.pack_into("<I", truncated_body, 0x40 + 24 + 240 + 16, 1_000_000)
        no_code = bytearray(good)
        struct.pack_into("<I", no_code, 0x40 + 24 + 240 + 36, 0x40000040)
        for bad in (bytes(arm), bytes(dll), b"not PE", good[:100], bytes(truncated_body), bytes(no_code)):
            with self.subTest(size=len(bad)), self.assertRaises(ValueError):
                artifact.pe_imports(bad)

    def test_unknown_and_debug_runtime_dependencies_fail_closed(self):
        for name in ("missing-plugin.dll", "VCRUNTIME140D.dll", "ucrtbased.dll"):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "runtime dependency"):
                artifact.pe_imports(executable([name]))

    def test_bundle_keeps_helpers_target_out_dir_cwd_environment_and_partial_coverage(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            pkg = root / "rust/example"
            pkg.mkdir(parents=True)
            target = root / "target" / artifact.TARGET / "debug"
            target.mkdir(parents=True)
            harness, helper = target / "suite.exe", target / "helper.exe"
            for binary in (harness, helper):
                binary.write_bytes(executable())
                binary.with_suffix(".pdb").write_bytes(b"not needed")
            out = target / "build/example/out"
            host = root / "target/debug/build/example/out"
            for directory in (out, host):
                directory.mkdir(parents=True)
                (directory / "generated").write_text("fixture")
            package = {"id": "id", "name": "example", "version": "1.2.3", "authors": [],
                       "manifest_path": str(pkg / "Cargo.toml"),
                       "targets": [{"name": "suite", "test": True, "kind": ["test"]}]}
            metadata = {"packages": [package], "workspace_members": ["id"], "target_directory": str(root / "target")}
            events = [{"reason": "build-script-executed", "package_id": "id", "out_dir": str(directory),
                       "env": [["SELECTED", str(directory)]]} for directory in (out, host)]
            events += [{"reason": "compiler-artifact", "package_id": "id", "executable": str(binary),
                        "profile": {"test": binary == harness}, "target": {"name": "suite"}} for binary in (helper, harness)]
            events.append({"reason": "build-finished", "success": True})
            bundle = root / "bundle"
            data = artifact.collect(metadata, events, root, bundle, ["--workspace"])
            entry = data["tests"][0]
            self.assertEqual(data["target"], artifact.TARGET)
            self.assertEqual(data["coverage"], "rust-test-executables")
            self.assertNotIn("doctests_complete", data)
            self.assertEqual(entry["cwd"], "rust/example")
            self.assertEqual(entry["env"]["OUT_DIR"], "{root}/" + str(out.relative_to(root)))
            self.assertEqual(entry["env"]["SELECTED"], "{root}/" + str(out.relative_to(root)))
            self.assertIn(str(helper.relative_to(root)), data["files"])
            self.assertFalse(any(name.endswith(".pdb") for name in data["files"]))
            self.assertEqual(data["runtime_dlls"], ["vcruntime140.dll"])
            artifact.verify(bundle / "manifest.json")
            helper_in_bundle = bundle / helper.relative_to(root)
            helper_in_bundle.write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "checksum"):
                artifact.verify(bundle / "manifest.json")

    def test_missing_category_failed_build_and_stale_output_are_rejected(self):
        meta = {"workspace_members": ["id"], "packages": [{"id": "id", "name": "example",
                "targets": [{"name": "suite", "test": True, "kind": ["test"]}]}]}
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, "test target inventory"):
                artifact.collect(meta, [{"reason": "build-finished", "success": True}], root, root / "bundle", ["--workspace"])
            with self.assertRaises(ValueError):
                artifact.collect(meta, [], root, root / "bundle", ["--workspace"])
            (root / "old").write_text("stale")
            with mock.patch.object(artifact.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "empty"):
                    artifact.build(root, None)
                run.assert_not_called()

    def test_build_command_reuses_link_check_and_preserves_scope(self):
        self.assertEqual(artifact.build_command(["--workspace"]),
                         ["soldr", "cargo", "test", "--workspace", "--no-run", "--locked", "--target", artifact.TARGET, "--message-format=json"])
        command = artifact.build_command(["-p", "obs-data"])
        self.assertNotIn("--workspace", command)
        self.assertEqual(command[-3:], ["-p", "obs-data", "--message-format=json"])
        self.assertNotIn("--doc", command)

    def test_cargo_failure_cannot_publish_manifest(self):
        meta = {"packages": []}
        failed = subprocess.CompletedProcess([], 3, '{"reason":"build-finished","success":true}\n', "link failed\n")
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(artifact.local_gate, "add_target"), \
                mock.patch.object(artifact.subprocess, "run", side_effect=[subprocess.CompletedProcess([], 0, json.dumps(meta)), failed]):
            output = Path(temporary) / "bundle"
            with self.assertRaises(subprocess.CalledProcessError):
                artifact.build(output, None)
            self.assertFalse((output / "manifest.json").exists())

    def test_shared_darwin_collector_keeps_default_target_and_verifier(self):
        # A Windows producer must not mutate Darwin module globals.
        self.assertEqual(shared.TARGET, "aarch64-apple-darwin")
        self.assertNotEqual(shared.TARGET, artifact.TARGET)


if __name__ == "__main__":
    unittest.main()
