#!/usr/bin/env -S uv run --script
"""Regression contract for Linux-built macOS Rust test artifacts (#5008)."""
from __future__ import annotations

import json
import os
import re
import subprocess
import hashlib
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import macos_test_artifact as artifact


class ArtifactTest(unittest.TestCase):
    def test_build_script_outputs_keep_both_hosts_and_select_target_env(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            host = root / "target/debug/build/example/out"
            target = root / "target" / artifact.TARGET / "debug/build/example/out"
            for path in (host, target):
                path.mkdir(parents=True)
                (path / "generated").write_text("output")
            events = [{"reason": "build-script-executed", "package_id": "example-id",
                       "out_dir": str(path), "env": [["WHERE", str(path)]]} for path in (target, host)]
            bundle = root / "bundle"
            scripts = artifact.collect_build_scripts(events, root, bundle, root / "target")
            self.assertEqual(scripts["example-id"]["out_dir"], str(target))
            for path in (host, target):
                self.assertTrue((bundle / path.relative_to(root) / "generated").is_file())

    def test_doctest_capture_keeps_arguments_environment_and_signature(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "temporary/rust_out"
            binary.parent.mkdir()
            binary.write_bytes(SignatureTest.signed_image())
            package = root / "rust/example"
            package.mkdir(parents=True)
            env = {"CARGO_PKG_NAME": "example", "CARGO_MANIFEST_DIR": str(package),
                   "RUSTDOC_DOCTEST_BIN_PATH": str(binary)}
            bundle = root / "bundle"
            entry = artifact.capture_doctest(binary, ["--test-threads=2"], root, bundle, package, env)
            self.assertEqual(entry["cwd"], "rust/example")
            self.assertEqual(entry["args"], ["--test-threads=2"])
            self.assertEqual(entry["env"]["CARGO_MANIFEST_DIR"], "{root}/rust/example")
            self.assertEqual(entry["env"]["RUSTDOC_DOCTEST_BIN_PATH"], "{root}/" + entry["binary"])
            self.assertEqual((bundle / entry["binary"]).read_bytes(), binary.read_bytes())
            binary.write_bytes(b"unsigned")
            with self.assertRaises((ValueError, struct.error)):
                artifact.capture_doctest(binary, [], root, bundle, package, env)

    def test_doctest_command_uses_stable_runtool_and_soldr_darwin_linker(self):
        with mock.patch.object(artifact.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "/soldr/darwin-clang\n")) as run:
            env = artifact.doctest_environment(Path("/src"), Path("/bundle"), {})
        self.assertEqual(run.call_args.args[0], ["soldr", "cc", "--target", artifact.TARGET, "--print-linker"])
        flags = env["RUSTDOCFLAGS"]
        self.assertIn("linker=/soldr/darwin-clang", flags)
        self.assertIn("--test-runtool", flags)
        self.assertIn("capture-doctest", flags)
        self.assertNotIn("unstable", flags)

    def test_compiled_root_mismatch_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({"version": 1, "target": artifact.TARGET,
                                           "build_root": "/another/checkout", "tests": []}))
            with self.assertRaisesRegex(ValueError, "source root mismatch"):
                artifact.execute(manifest, root)

    def test_incomplete_doctests_and_tampered_helper_fail_before_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "suite"
            binary.write_bytes(b"suite")
            helper = root / "helper"
            helper.write_bytes(b"helper")
            data = {"version": 1, "target": artifact.TARGET, "build_root": str(root),
                    "tests": [{"package": "example", "binary": "suite", "cwd": ".", "env": {}, "args": []}]}
            artifact.finalize(root, data)
            with mock.patch.object(artifact.subprocess, "run") as run:
                with self.assertRaisesRegex(ValueError, "doctest compilation"):
                    artifact.execute(root / "manifest.json", root)
                data["doctests_complete"] = True
                artifact.finalize(root, data)
                helper.write_bytes(b"corrupted helper")
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    artifact.execute(root / "manifest.json", root)
                run.assert_not_called()

    def test_preexisting_bundle_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "stale").write_bytes(b"old doctest")
            with self.assertRaisesRegex(ValueError, "stale artifacts"):
                artifact.build(root, None)

    def test_inventory_rejects_missing_category_and_accepts_scoped_no_harness(self):
        meta = {"workspace_members": ["id"], "packages": [
            {"id": "id", "name": "example", "targets": [{"name": "suite", "kind": ["test"], "test": True}]},
            {"id": "empty", "name": "adapter", "targets": [{"name": "adapter", "kind": ["lib"], "test": False}]}]}
        with self.assertRaisesRegex(ValueError, "test target inventory"):
            artifact.check_test_inventory(meta, [], ["--workspace"])
        events = [{"reason": "compiler-artifact", "package_id": "id", "target": {"name": "suite"},
                   "profile": {"test": True}, "executable": "/src/suite"}]
        self.assertTrue(artifact.check_test_inventory(meta, events, ["--workspace"]))
        self.assertFalse(artifact.check_test_inventory(meta, [], ["-p", "adapter"]))

    def test_manifest_keeps_package_cwd_env_out_dir_and_helper(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "rust/example"
            package.mkdir(parents=True)
            binary = root / "target/test-example"
            binary.parent.mkdir()
            binary.write_bytes(b"test")
            helper = binary.with_name("example")
            helper.write_bytes(b"helper")
            output = root / "target/out"
            output.mkdir()
            (output / "fixture").write_text("generated")
            meta = {"packages": [{"id": "example-id", "name": "example", "version": "1.2.3",
                    "manifest_path": str(package / "Cargo.toml"), "authors": ["An Author"],
                    "description": "description", "homepage": None, "repository": None,
                    "license": "MIT", "license_file": None, "readme": None, "rust_version": "1.98"}]}
            events = [
                {"reason": "build-script-executed", "package_id": "example-id", "out_dir": str(output), "env": [["CUSTOM", "value"]]},
                {"reason": "compiler-artifact", "package_id": "example-id", "executable": str(helper), "profile": {"test": False}, "target": {"name": "example", "kind": ["bin"], "crate_types": ["bin"]}},
                {"reason": "compiler-artifact", "package_id": "example-id", "executable": str(binary), "profile": {"test": True}, "target": {"name": "suite", "kind": ["test"], "crate_types": ["bin"]}},
                {"reason": "build-finished", "success": True},
            ]
            bundle = root / "bundle"
            with mock.patch.object(artifact, "verify_macho"):
                result = artifact.collect(meta, events, root, bundle, ["--test-threads=2"])
            entry = result["tests"][0]
            self.assertEqual(entry["package"], "example")
            self.assertEqual(entry["cwd"], "rust/example")
            self.assertEqual(entry["env"]["CARGO_PKG_VERSION"], "1.2.3")
            self.assertEqual(entry["env"]["CUSTOM"], "value")
            self.assertEqual(entry["env"]["CARGO_MANIFEST_DIR"], "{root}/rust/example")
            self.assertEqual(entry["env"]["OUT_DIR"], "{root}/target/out")
            self.assertTrue((bundle / "target/example").is_file())
            self.assertEqual(entry["args"], ["--test-threads=2"])
            self.assertTrue((bundle / "target/out/fixture").is_file())

    def test_failed_or_incomplete_build_is_rejected(self):
        for events in ([], [{"reason": "build-finished", "success": False}]):
            with self.subTest(events=events), self.assertRaises(ValueError):
                artifact.collect({"packages": []}, events, Path("/tmp"), Path("/tmp/unused"), [])

    def test_scope_reuses_affected_selection(self):
        meta = {"packages": []}
        with mock.patch.object(artifact, "affected", return_value=("crates", ["obs-data"])) as affected:
            self.assertEqual(artifact.packages_for(["rust/obs-data/src/lib.rs"], meta), ["-p", "obs-data"])
            affected.assert_called_once_with(["rust/obs-data/src/lib.rs"], {})
        with mock.patch.object(artifact, "affected", return_value=("skip", [])):
            self.assertEqual(artifact.packages_for(["docs/a.md"], meta), [])
        self.assertEqual(artifact.packages_for(None, meta), ["--workspace"])

    def test_runner_preserves_cwd_env_arguments_and_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "rust/example").mkdir(parents=True)
            binary = root / "target/test-example"
            binary.parent.mkdir()
            binary.write_text("executable")
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({"version": 1, "target": artifact.TARGET, "build_root": str(root), "tests": [
                {"package": "example", "binary": "target/test-example", "cwd": "rust/example", "env": {"CARGO_MANIFEST_DIR": "{root}/rust/example", "CUSTOM": "expected"}, "args": ["filter", "--exact"]}],
                "doctests_complete": True, "files": {"target/test-example": hashlib.sha256(binary.read_bytes()).hexdigest()}}))
            with mock.patch.object(artifact.subprocess, "run", return_value=subprocess.CompletedProcess([], 19)) as run:
                self.assertEqual(artifact.execute(manifest, root), 19)
            args, kwargs = run.call_args
            self.assertEqual(args[0], [str(binary), "filter", "--exact"])
            self.assertEqual(kwargs["cwd"], root / "rust/example")
            self.assertEqual(kwargs["env"]["CUSTOM"], "expected")
            self.assertEqual(kwargs["env"]["CARGO_MANIFEST_DIR"], str(root / "rust/example"))
            self.assertTrue(os.access(binary, os.X_OK))

    def test_missing_and_escaping_entries_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(FileNotFoundError):
                artifact.execute(root / "missing.json", root)
            manifest = root / "manifest.json"
            for binary in ("../escape", "target/missing"):
                manifest.write_text(json.dumps({"version": 1, "target": artifact.TARGET, "build_root": str(root), "tests": [
                    {"package": "example", "binary": binary, "cwd": ".", "env": {}, "args": []}],
                    "doctests_complete": True, "files": {binary: "unused"}}))
                with self.subTest(binary=binary), self.assertRaises((ValueError, FileNotFoundError)):
                    artifact.execute(manifest, root)


class SignatureTest(unittest.TestCase):
    @staticmethod
    def signed_image():
        image = bytearray(4096)
        struct.pack_into("<8I", image, 0, 0xFEEDFACF, 0x100000C, 0, 2, 1, 16, 0, 0)
        directory_length = 44 + 32
        signature_length = 20 + directory_length
        struct.pack_into("<4I", image, 32, 0x1D, 16, len(image), signature_length)
        directory = bytearray(directory_length)
        struct.pack_into(">9I", directory, 0, 0xFADE0C02, directory_length, 0x20400, 2, 44, 0, 0, 1, len(image))
        struct.pack_into("4B", directory, 36, 32, 2, 0, 12)
        directory[44:] = hashlib.sha256(image).digest()
        return image + struct.pack(">5I", 0xFADE0CC0, signature_length, 1, 0, 20) + directory

    def test_signature_requires_arm64_execute_and_valid_page_hashes(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "test"
            data = self.signed_image()
            path.write_bytes(data)
            artifact.verify_macho(path)
            data[100] ^= 1
            path.write_bytes(data)
            with self.assertRaisesRegex(ValueError, "invalid code signature page"):
                artifact.verify_macho(path)
            struct.pack_into("<I", data, 4, 0x1000007)
            path.write_bytes(data)
            with self.assertRaisesRegex(ValueError, "not an arm64"):
                artifact.verify_macho(path)

    def test_unsigned_image_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "test"
            data = self.signed_image()
            struct.pack_into("<I", data, 16, 0)
            path.write_bytes(data)
            with self.assertRaisesRegex(ValueError, "missing arm64 code signature"):
                artifact.verify_macho(path)


class WorkflowTest(unittest.TestCase):
    def assert_run_only_job(self, job):
        self.assertTrue("rust-tests-macos-build" in job, "macOS job has no Linux artifact producer dependency")
        self.assertIn("actions/download-artifact@", job)
        self.assertIn("ci/macos_test_artifact.py run", job)
        self.assertNotIn("soldr cargo", job)
        self.assertNotIn("setup-soldr@", job)

    def test_run_only_contract_rejects_missing_dependency_or_compilation(self):
        job = "needs: [rust-tests-macos-build]\nactions/download-artifact@pin\nci/macos_test_artifact.py run"
        self.assert_run_only_job(job)
        for invalid in (job.replace("rust-tests-macos-build", "other"), job + "\nsoldr cargo test",
                        job + "\nsetup-soldr@pin", job.replace("actions/download-artifact@", ""),
                        job.replace("ci/macos_test_artifact.py run", "")):
            with self.subTest(job=invalid), self.assertRaises(AssertionError):
                self.assert_run_only_job(invalid)

    def test_workflow_activation_is_atomic(self):
        workflow = (Path(__file__).resolve().parents[1] / ".github/workflows/build-project.yaml").read_text()
        job = re.search(r"^  rust-tests-macos:\n(.*?)(?=^  [a-z0-9-]+:)", workflow, re.S | re.M).group(1)
        if re.search(r"^  rust-tests-macos-build:", workflow, re.M):
            self.assert_run_only_job(job)
        else:
            # Infrastructure-only slice: activating just the executable runner
            # would lose native-build/Qt fixtures. Keep the switch together.
            self.assertNotIn("ci/macos_test_artifact.py run", job)
            self.assertNotIn("rust-tests-macos-build", job)


if __name__ == "__main__":
    unittest.main()
