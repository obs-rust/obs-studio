#!/usr/bin/env -S uv run --script
"""Regression contract for Linux-built macOS Rust test artifacts (#5008)."""
from __future__ import annotations

import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import macos_test_artifact as artifact


class ArtifactTest(unittest.TestCase):
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
                {"package": "example", "binary": "target/test-example", "cwd": "rust/example", "env": {"CARGO_MANIFEST_DIR": "{root}/rust/example", "CUSTOM": "expected"}, "args": ["filter", "--exact"]}]}))
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
                    {"package": "example", "binary": binary, "cwd": ".", "env": {}, "args": []}]}))
                with self.subTest(binary=binary), self.assertRaises((ValueError, FileNotFoundError)):
                    artifact.execute(manifest, root)


class WorkflowTest(unittest.TestCase):
    def test_native_job_depends_on_artifact_build_and_never_compiles(self):
        workflow = (Path(__file__).resolve().parents[1] / ".github/workflows/build-project.yaml").read_text()
        job = re.search(r"^  rust-tests-macos:\n(.*?)(?=^  [a-z0-9-]+:)", workflow, re.S | re.M).group(1)
        self.assertIn("rust-tests-macos-build", job)
        self.assertIn("actions/download-artifact@", job)
        self.assertIn("ci/macos_test_artifact.py run", job)
        self.assertNotIn("soldr cargo", job)
        self.assertNotIn("setup-soldr@", job)


if __name__ == "__main__":
    unittest.main()
