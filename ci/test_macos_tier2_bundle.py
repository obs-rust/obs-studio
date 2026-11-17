#!/usr/bin/env -S uv run --script
"""Exercise relocation and fail-closed execution of the Tier 2 bundle."""

from __future__ import annotations

import json
import subprocess
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from ci import macos_tier2_bundle as bundle


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
