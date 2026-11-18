#!/usr/bin/env -S uv run --script
"""The complete Darwin bundle is built on Linux and only executed on macOS."""
from __future__ import annotations

import errno
import json
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import gate_jobs
import macos_test_artifact as artifact
from ci_scope import scope

ROOT = Path(__file__).resolve().parents[1]


def jobs():
    return dict(re.findall(r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)",
                          (ROOT / ".github/workflows/build-project.yaml").read_text(), re.M | re.S))


class WorkflowTest(unittest.TestCase):
    def test_consumer_has_no_compilation_or_toolchain_setup(self):
        consumer = jobs()["rust-tests-macos"]
        self.assertIn("needs: [check-event, ci-mode, rust-tests-macos-build]", consumer)
        self.assertIn("needs.rust-tests-macos-build.result == 'success'", consumer)
        self.assertIn("name: Rust Tests 🦀 (macos-26)", consumer)
        self.assertIn("actions/download-artifact@", consumer)
        self.assertIn("ci/macos_test_artifact.py run", consumer)
        for command in ("setup-soldr@", "soldr cargo", "local_gate.py", "brew install", "cmake --build",
                        "rustc", "rustfmt", "clippy", "clang", "swiftc", "moc", "uic", "rcc"):
            with self.subTest(command=command):
                self.assertNotIn(command, consumer)
        self.assertRegex(consumer, r"python-version: '3\.14'")
        self.assertIn("ci/macos_test_workspace.py unpack", consumer)

    def test_producer_keeps_scope_and_uses_existing_heavy_routing(self):
        producer = jobs()["rust-tests-macos-build"]
        self.assertIn("runs-on: ${{ inputs.mq-heavy-labels != '' && fromJSON(inputs.mq-heavy-labels) || 'ubuntu-26.04' }}", producer)
        for text in ("needs.check-event.outputs.macos == 'true'", "skip_rust-tests-macos != 'true'",
                     "github.event_name != 'merge_group'", "needs.check-event.outputs.deferred == 'true'",
                     "--base", "needs.check-event.outputs.qt_sdk", "ci/macos_qt_artifact.py build",
                     "ci/macos_test_artifact.py build", "ci/macos_test_workspace.py pack",
                     "actions/upload-artifact@", "if-no-files-found: error"):
            self.assertIn(text, producer)
        self.assertNotIn("rust-tests-linux", producer.split("    steps:")[0])

    def test_both_hosts_use_one_real_checkout_and_restore_it(self):
        workflow = jobs()
        for name in ("rust-tests-macos-build", "rust-tests-macos"):
            job = workflow[name]
            self.assertIn("ci/macos_test_workspace.py place", job)
            self.assertIn("ci/macos_test_workspace.py restore", job)
            self.assertIn("if: always()", job)
            self.assertLess(job.index("ci/macos_test_workspace.py place"),
                            job.index("ci/macos_test_artifact.py"))

    def test_consumer_rerun_uses_original_producer_receipts(self):
        workflow = jobs()
        producer, consumer = (workflow[name] for name in ("rust-tests-macos-build", "rust-tests-macos"))
        self.assertIn("/opt/obs-ci-${{ github.run_id }}-${{ github.run_attempt }}", producer)
        self.assertIn("common-parent: ${{ steps.workspace.outputs.common-parent }}", producer)
        self.assertIn("artifact-id: ${{ steps.bundle.outputs.artifact-id }}", producer)
        self.assertIn("COMMON_PARENT: ${{ needs.rust-tests-macos-build.outputs.common-parent }}", consumer)
        self.assertIn("artifact-ids: ${{ needs.rust-tests-macos-build.outputs.artifact-id }}", consumer)
        # A consumer attempt 2 still uses producer attempt 1's root and artifact.
        self.assertNotIn("github.run_attempt", consumer)

    def test_linux_prerequisites_precede_cooking_and_analysis(self):
        producer = jobs()["rust-tests-macos-build"]
        self.assertLess(producer.index("Install Linux preparation dependencies"), producer.index("Set Up soldr"))
        self.assertLess(producer.index("ci/macos_test_artifact.py prepare-target"), producer.index("Run Clippy"))
        for package in ("uuid-dev", "libglib2.0-dev", "libxkbcommon-dev", "libncurses6"):
            self.assertIn(package, producer)

    def test_gate_requires_both_selected_jobs_and_rejects_missing_producer(self):
        selected = scope(["rust/obs-util/src/dstr.rs"], event="merge_group")
        expected = gate_jobs.expected_jobs(selected)
        producer = "Build 🧱 / macOS Rust Artifacts 🐧"
        self.assertIn(producer, expected)
        self.assertIn("Build 🧱 / Rust Tests 🦀 (macos-26)", expected)
        records = [{"name": name, "status": "completed", "conclusion": "success"} for name in expected]
        for conclusion in ("skipped", "failure", "cancelled", None):
            with self.subTest(conclusion=conclusion):
                broken = [record if record["name"] != producer else record | {"conclusion": conclusion,
                          "status": "completed" if conclusion else "queued"} for record in records]
                self.assertEqual(gate_jobs.blocking(broken, expected=expected), [producer])
        self.assertNotIn(producer, gate_jobs.expected_jobs(scope(["docs/features.md"], event="merge_group")))


class WorkspaceTest(unittest.TestCase):
    def test_restore_removes_privileged_common_parent_and_propagates_real_errors(self):
        import macos_test_workspace as workspace

        for failure in (None, PermissionError("sudo denied")):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temporary:
                source = Path(temporary) / "runner/checkout"
                source.mkdir(parents=True)
                (source / "Cargo.toml").write_text("workspace")
                destination = Path(temporary) / "common/src"
                workspace.place(source, destination)
                original = Path.rmdir

                def remove(path):
                    if path == destination.parent:
                        raise PermissionError(errno.EACCES, "root-owned /opt", str(path))
                    return original(path)

                def privileged(command, **kwargs):
                    self.assertEqual(command, ["sudo", "-n", "rmdir", "--", str(destination.parent)])
                    self.assertEqual(kwargs, {"check": True})
                    if failure:
                        raise failure
                    original(destination.parent)

                with mock.patch.object(Path, "rmdir", remove), mock.patch("subprocess.run", side_effect=privileged) as run:
                    if failure:
                        with self.assertRaisesRegex(PermissionError, "sudo denied"):
                            workspace.restore(source, destination)
                    else:
                        workspace.restore(source, destination)
                    run.assert_called_once()
                self.assertFalse(source.is_symlink())
                self.assertEqual((source / "Cargo.toml").read_text(), "workspace")

    def test_restore_does_not_hide_nonempty_common_parent(self):
        import macos_test_workspace as workspace

        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "checkout"
            source.mkdir()
            destination = Path(temporary) / "common/src"
            workspace.place(source, destination)
            (destination.parent / "unexpected").write_text("preserve")
            with mock.patch("subprocess.run") as run, self.assertRaises(OSError) as error:
                workspace.restore(source, destination)
            self.assertEqual(error.exception.errno, errno.ENOTEMPTY)
            run.assert_not_called()

    def test_checkout_placement_across_runner_mounts(self):
        import macos_test_workspace as workspace

        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "runner/checkout"
            source.mkdir(parents=True)
            (source / "Cargo.toml").write_text("workspace")
            destination = Path(temporary) / "common/src"
            with mock.patch("os.rename", side_effect=OSError(errno.EXDEV, "different mounts")):
                workspace.place(source, destination)
                workspace.restore(source, destination)
            self.assertEqual((source / "Cargo.toml").read_text(), "workspace")

    def test_real_checkout_moves_and_restores_without_a_second_copy(self):
        import macos_test_workspace as workspace

        with tempfile.TemporaryDirectory() as temporary:
            source = Path(temporary) / "runner/checkout"
            source.mkdir(parents=True)
            (source / "Cargo.toml").write_text("workspace")
            destination = Path(temporary) / "common/src"
            workspace.place(source, destination)
            self.assertTrue(source.is_symlink())
            self.assertEqual(source.resolve(), destination)
            self.assertEqual((source / "Cargo.toml").read_text(), "workspace")
            with self.assertRaises(ValueError):
                workspace.place(source, destination)
            workspace.restore(source, destination)
            self.assertFalse(source.is_symlink())
            self.assertEqual((source / "Cargo.toml").read_text(), "workspace")
            self.assertFalse(destination.exists())

    def test_symlinked_common_parent_is_rejected(self):
        import macos_test_workspace as workspace

        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            source = parent / "checkout"
            source.mkdir()
            (parent / "alias").symlink_to(parent, target_is_directory=True)
            with self.assertRaises(ValueError):
                workspace.place(source, parent / "alias/src")
            self.assertTrue(source.is_dir())

    def test_archive_round_trip_and_corruption_fail_closed(self):
        import macos_test_workspace as workspace

        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            bundle = parent / "bundle"
            bundle.mkdir()
            (bundle / "manifest.json").write_text('{"tests": []}')
            (bundle / ".hidden").write_bytes(b"build-script data")
            archive = parent / "bundle.tar.gz"
            workspace.pack(bundle, archive)
            destination = parent / "checkout"
            destination.mkdir()
            workspace.unpack(archive, destination)
            self.assertEqual((destination / ".hidden").read_bytes(), b"build-script data")
            archive.write_bytes(archive.read_bytes() + b"corrupt")
            with self.assertRaisesRegex(ValueError, "checksum"):
                workspace.unpack(archive, destination)

    def test_missing_qt_payload_fails_before_running_any_test(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "manifest.json"
            manifest.write_text(json.dumps({"version": 1, "target": artifact.TARGET, "build_root": str(root),
                                           "tests": [], "scope_no_tests": True, "doctests_complete": True, "files": {}}))
            with mock.patch.object(artifact.subprocess, "run") as run, self.assertRaisesRegex(ValueError, "Qt"):
                artifact.execute(manifest, root, require_qt=True)
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
