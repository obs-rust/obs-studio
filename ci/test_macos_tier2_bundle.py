#!/usr/bin/env -S uv run --script
"""Exercise relocation and fail-closed execution of the Tier 2 bundle."""

from __future__ import annotations

import json
import importlib
import subprocess
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_tier2_bundle as bundle


class PacketTest(unittest.TestCase):
    def test_seal_rejects_extra_files_tampering_and_missing_empty_directories(self):
        packet = importlib.import_module("macos_tier2_packet")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "empty space").mkdir()
            (root / "probe").write_bytes(b"signed fixture")
            manifest = packet.seal(root, {"schema": 2, "complete": True})
            packet.verify_files(root, manifest)
            (root / "extra").touch()
            with self.assertRaisesRegex(ValueError, "inventory"):
                packet.verify_files(root, manifest)
            (root / "extra").unlink()
            (root / "probe").write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "inventory"):
                packet.verify_files(root, manifest)
            (root / "probe").write_bytes(b"signed fixture")
            (root / "empty space").rmdir()
            with self.assertRaisesRegex(ValueError, "inventory"):
                packet.verify_files(root, manifest)

    def test_scan_checks_binary_and_utf16_resource_paths(self):
        packet = importlib.import_module("macos_tier2_packet")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for encoding in ("utf-8", "utf-16le"):
                (root / "image").write_bytes("/producer/source/fixture".encode(encoding))
                with self.assertRaisesRegex(ValueError, "producer path"):
                    packet.scan(root, ["/producer/source"])

    def test_portable_names_and_escaping_links_fail_closed(self):
        packet = importlib.import_module("macos_tier2_packet")
        for name in ("/absolute", "../escape", "a/../b", "a\\b", "a//b"):
            with self.subTest(name=name), self.assertRaises(ValueError):
                packet.relative(name)

    def test_tool_identity_changes_only_known_executable_slots(self):
        packet = importlib.import_module("macos_tier2_packet")
        command = ["/producer/bin/cmake", "-E", "env", "LITERAL=/producer/bin/cmake",
                   "/producer/bin/cmake", "-E", "echo", "/producer/bin/cmake"]
        result = packet.command_tokens(command, {"cmake": "/producer/bin/cmake"})
        self.assertEqual(result[0], "@OBS_TIER2_TOOL_CMAKE@")
        self.assertEqual(result[4], result[0])
        self.assertEqual(result[3], command[3])
        self.assertEqual(result[7], command[7])
        self.assertEqual(packet.command_tokens(["/unknown/cmake"], {"cmake": command[0]}), ["/unknown/cmake"])

    def test_actual_ctest_roundtrip_keeps_empty_arguments_names_and_properties(self):
        packet = importlib.import_module("macos_tier2_packet")
        with tempfile.TemporaryDirectory(prefix="tier2 with space ") as temporary:
            root = Path(temporary)
            tests = [{"name": "module::case", "config": "RelWithDebInfo", "command": ["/bin/false", "", "C:/literal\\name", "x]==]y"],
                      "properties": [{"name": "WORKING_DIRECTORY", "value": str(root)},
                                     {"name": "TIMEOUT", "value": 17.0},
                                     {"name": "SKIP_RETURN_CODE", "value": 77},
                                     {"name": "RUN_SERIAL", "value": True},
                                     {"name": "ENVIRONMENT", "value": ["A=one;two", "B=yes"]}]}]
            (root / "CTestTestfile.cmake").write_text(packet.ctest_text(tests))
            self.assertEqual(packet.normalized_tests(packet.snapshot(root)), packet.normalized_tests({"tests": tests}))

    def test_hosted_proof_is_opt_in_and_preserves_required_native_job(self):
        from test_macos_run_only import jobs

        workflow = jobs()
        self.assertIn("rust-tier2-macos-proof", workflow)
        proof = workflow["rust-tier2-macos-proof"]
        self.assertIn("inputs.macos-tier2-proof-artifact-id != ''", proof)
        self.assertIn("ci/macos_tier2_packet.py consume", proof)
        self.assertIn("no compiler", proof)
        for command in ("setup-soldr@", "brew install", "cmake --build", "soldr cargo"):
            self.assertNotIn(command, proof)
        self.assertIn("bash rust/tools/tier2-validate/validate.sh", workflow["rust-tier2-macos"])

    def test_non_common_producer_cannot_certify_existing_absolute_binaries(self):
        packet = importlib.import_module("macos_tier2_packet")
        with patch.object(packet.host_platform, "is_linux", return_value=True):
            with self.assertRaisesRegex(ValueError, "common real checkout"):
                packet.stage(Path("/producer/source"), Path("/absent/output"), Path("receipt"), Path("contract"), [])

    def test_archive_rejects_traversal_before_creating_destination(self):
        import io
        import tarfile

        packet = importlib.import_module("macos_tier2_packet")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "packet.tar.zst"
            with tarfile.open(archive, "w:zst") as output:
                info = tarfile.TarInfo("../escape")
                info.size = 1
                output.addfile(info, io.BytesIO(b"x"))
            with self.assertRaisesRegex(ValueError, "nonportable"):
                packet.extract(archive, root / "output", bundle.digest(archive))
            self.assertFalse((root / "output").exists())

    def test_immutable_artifact_must_match_exact_source_before_downloading(self):
        packet = importlib.import_module("macos_tier2_packet")
        with patch.object(packet.subprocess, "check_output", return_value=json.dumps({
            "expired": False, "workflow_run": {"head_sha": "b" * 40}
        })), patch.object(packet.subprocess, "run") as execute:
            with self.assertRaisesRegex(ValueError, "another source"):
                packet.download("obs-rust/obs-studio", "12345", "a" * 40, Path("/unused"))
            execute.assert_not_called()


class BundleTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.build = self.root / "producer"
        self.build.mkdir()
        self.source = self.root / "source"
        self.source.mkdir()
        self.output = self.root / "bundle"
        self.binary = self.build / "test_probe"
        self.binary.write_bytes(b"fixture")
        self.registry = self.build / "CTestTestfile.cmake"
        self.registry.write_text(
            f'add_test(probe "{self.binary}")\n'
            "set_tests_properties(probe PROPERTIES TIMEOUT 17 RUN_SERIAL TRUE "
            'SKIP_RETURN_CODE 77 ENVIRONMENT "OBS_TEST=yes")\n'
        )

    def stage(self):
        return bundle.stage(
            self.build, self.source, self.output, [self.binary], lambda _: None
        )

    def test_runtime_inventory_follows_the_complete_tier2_ninja_closure(self):
        (self.build / "input.c").touch()
        (self.build / "build.ninja").write_text(
            "rule link\n  command = true\n"
            "build fixture: link input.c\n"
            "build helper: link input.c\n"
            "build unrelated-plugin: link input.c\n"
            "build tier2-cross: phony fixture helper\n"
        )
        paths = [self.build / name for name in ("fixture", "helper", "unrelated-plugin")]
        self.assertEqual(bundle.selected_runtime(self.build, paths), paths[:2])

    def test_empty_space_named_working_directory_survives_transport(self):
        directory = "test/cmocka/output runtime root/Release"
        (self.build / directory).mkdir(parents=True)
        manifest = self.stage()
        self.assertIn("build/" + directory, manifest["directories"])
        moved = self.root / "moved"
        self.output.rename(moved)
        bundle.prepare(moved, self.source)
        self.assertTrue((moved / "build" / directory).is_dir())

    def test_escaping_directory_inventory_is_rejected(self):
        self.stage()
        path = self.output / "manifest.json"
        manifest = json.loads(path.read_text())
        manifest["directories"] = ["../outside"]
        path.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, "escapes artifact root"):
            bundle.prepare(self.output, self.source)

    def test_prepared_fixture_evidence_and_products_are_transported(self):
        fixture = self.build / "test/cmocka/core_module_manifest_source"
        fixture.mkdir(parents=True)
        (fixture / "evidence.cmake").write_text("set(_build_result 0)\n")
        (fixture / "build-graph.txt").write_text("generated source selected\n")
        manifest = self.stage()
        relative = "build/test/cmocka/core_module_manifest_source/build-graph.txt"
        self.assertIn(relative, manifest["files"])
        self.assertEqual(
            (self.output / relative).read_text(), "generated source selected\n"
        )
        (self.output / relative).write_text("changed")
        with self.assertRaisesRegex(ValueError, "changed artifact"):
            bundle.prepare(self.output, self.source)

    def test_relocation_preserves_ctest_properties(self):
        self.stage()
        moved = self.root / "relocated"
        self.output.rename(moved)
        bundle.prepare(moved, self.source)
        registry = (moved / "build/CTestTestfile.cmake").read_text()
        self.assertIn(str(moved / "build/test_probe"), registry)
        self.assertNotIn(str(self.build), registry)
        for assertion in (
            "TIMEOUT 17",
            "RUN_SERIAL TRUE",
            "SKIP_RETURN_CODE 77",
            "OBS_TEST=yes",
        ):
            self.assertIn(assertion, registry)

    def test_missing_binary_or_registry_is_rejected(self):
        with self.assertRaises(FileNotFoundError):
            bundle.stage(
                self.build,
                self.source,
                self.output,
                [self.build / "absent"],
                lambda _: None,
            )
        self.registry.rename(self.build / "saved")
        with self.assertRaises(ValueError):
            self.stage()

    def test_tampering_is_rejected_before_any_process_runs(self):
        self.stage()
        (self.output / "build/test_probe").write_bytes(b"changed")
        with patch.object(subprocess, "run") as run:
            with self.assertRaises(ValueError):
                bundle.run(self.output, self.source, "shared", 1)
            run.assert_not_called()

    def test_source_root_and_escaping_symlink_are_rejected(self):
        self.stage()
        with self.assertRaises(ValueError):
            bundle.prepare(self.output, self.root / "different-source")
        outside = self.root / "outside"
        outside.write_bytes(b"outside")
        link = self.build / "escape"
        link.symlink_to(outside)
        with self.assertRaises(ValueError):
            bundle.stage(
                self.build, self.source, self.root / "bad", [link], lambda _: None
            )

    def test_nonzero_test_exit_is_propagated(self):
        self.stage()
        self.complete_fixture()
        with patch.object(
            subprocess, "run", return_value=subprocess.CompletedProcess([], 8)
        ) as run:
            self.assertEqual(bundle.run(self.output, self.source, "shared", 3), 8)
            args, kwargs = run.call_args
            self.assertIn("--output-on-failure", args[0])
            self.assertIn("3", args[0])
            self.assertEqual(
                kwargs["env"]["DYLD_FRAMEWORK_PATH"],
                str(self.output / "build/cores/shared"),
            )

    def test_failed_signature_verification_prevents_bundle_creation(self):
        def reject(_):
            raise ValueError("invalid signature")

        with self.assertRaisesRegex(ValueError, "invalid signature"):
            bundle.stage(self.build, self.source, self.output, [self.binary], reject)
        self.assertFalse((self.output / "manifest.json").exists())

    def test_manifest_records_incomplete_native_coverage(self):
        self.stage()
        manifest = json.loads((self.output / "manifest.json").read_text())
        self.assertFalse(manifest["native_contract_prebuilt"])
        self.assertEqual(manifest["target"], "aarch64-apple-darwin")

    def complete_fixture(self):
        path = self.output / "manifest.json"
        manifest = json.loads(path.read_text())
        # Only this synthetic compiler-free registry is complete.
        manifest["native_contract_prebuilt"] = True
        path.write_text(json.dumps(manifest))

    def test_incomplete_bundle_cannot_launch_a_runtime_compiler(self):
        self.stage()
        with patch.object(subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "incomplete prototype"):
                bundle.run(self.output, self.source, "shared", 1)
            run.assert_not_called()

    def test_actual_ctest_failure_is_propagated_after_relocation(self):
        producer_cmake = self.root / "producer-only/bin/cmake"
        (self.build / "CMakeCache.txt").write_text(
            f"CMAKE_COMMAND:INTERNAL={producer_cmake}\n"
        )
        self.registry.write_text(f'add_test(probe "{producer_cmake}" -E false)\n')
        self.stage()
        self.complete_fixture()
        self.assertEqual(bundle.run(self.output, self.source, "reference", 1), 8)

    def test_universal_dependencies_keep_exact_arm64_signed_bytes(self):
        arm64 = b"arm64 signed bytes"
        header = struct.pack(">7I", 0xCAFEBABE, 1, 0x100000C, 0, 32, len(arm64), 0)
        dependency = self.root / "libfixture.dylib"
        dependency.write_bytes(header.ljust(32, b"\x00") + arm64)
        bundle.stage(
            self.build,
            self.source,
            self.output,
            [self.binary],
            lambda _: None,
            [dependency],
        )
        self.assertEqual((self.output / "deps/libfixture.dylib").read_bytes(), arm64)
        self.assertEqual(bundle.arm64_slice(arm64), arm64)
        with self.assertRaises(ValueError):
            bundle.arm64_slice(header)

    def test_embedded_build_paths_require_a_common_runtime_binding(self):
        self.binary.write_bytes(str(self.build).encode())
        self.stage()
        with self.assertRaisesRegex(ValueError, "fixtures embed the build root"):
            bundle.prepare(self.output, self.source)
        self.build.rename(self.root / "original-build")
        self.build.symlink_to(self.output / "build", target_is_directory=True)
        bundle.prepare(self.output, self.source)


if __name__ == "__main__":
    unittest.main()
