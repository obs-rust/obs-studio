#!/usr/bin/env -S uv run --script
"""Contracts for the opt-in Windows Tier 2 producer, with no Windows runner."""

import importlib
import io
import json
from pathlib import Path
import struct
import subprocess
import tempfile
import tarfile
import unittest
from unittest.mock import patch

from test_windows_zip_check import pe

ROOT = Path(__file__).resolve().parents[1]


class ProbeTests(unittest.TestCase):
    def helper(self):
        return importlib.import_module("windows_tier2_probe")

    def test_full_renderer_and_qt_system_imports_keep_unknown_dlls_fatal(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "renderer.dll"
            path.write_bytes(
                pe(
                    [
                        "authz.dll",
                        "coremessaging.dll",
                        "d3d11.dll",
                        "d3d12.dll",
                        "d3d9.dll",
                        "d3dcompiler_47.dll",
                        "dwrite.dll",
                        "icuuc.dll",
                    ]
                )
            )
            self.assertEqual([path], helper.closure([path], []))
            path.write_bytes(pe(["unprovided-renderer.dll"]))
            with self.assertRaisesRegex(ValueError, "unresolved"):
                helper.closure([path], [])

    def test_wrong_target_fails_before_toolchain_setup(self):
        with tempfile.TemporaryDirectory() as tmp:
            result = subprocess.run(
                [
                    "cmake",
                    "-S",
                    str(ROOT / "cmake/cross"),
                    "-B",
                    tmp,
                    "-DOBS_CROSS_WINDOWS_TIER2=ON",
                    "-DOBS_CROSS_TARGET=mac-arm64",
                ],
                capture_output=True,
                text=True,
                check=False,
                timeout=10,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "OBS_CROSS_WINDOWS_TIER2 requires win-x64", result.stdout + result.stderr
        )

    def test_pe_architecture_and_imports(self):
        helper = self.helper()
        self.assertEqual(helper.image(pe(["kernel32.dll"])), ["kernel32.dll"])
        data = bytearray(pe([]))
        struct.pack_into("<H", data, 0x44, 0xAA64)
        with self.assertRaisesRegex(ValueError, "AMD64"):
            helper.image(data)
        with self.assertRaises(ValueError):
            helper.image(b"MZ")

    def test_symbol_parity_rejects_missing_extra_and_empty(self):
        helper = self.helper()
        self.assertEqual(helper.parity({"a", "b"}, {"b", "a"}), ["a", "b"])
        for left, right in [({"a"}, {"b"}), (set(), set())]:
            with self.assertRaises(ValueError):
                helper.parity(left, right)

    def test_export_comparison_requires_both_core_variants(self):
        helper = self.helper()
        core = Path("shared/obs.dll")
        with self.assertRaisesRegex(ValueError, "distinct shared/reference"):
            helper.core_symbols([core, core], "llvm-readobj")

    def test_transitive_dll_closure_and_missing_runtime(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / "test.exe"
            exe.write_bytes(pe(["one.dll", "kernel32.dll"]))
            (root / "one.dll").write_bytes(pe(["two.dll"]))
            (root / "two.dll").write_bytes(pe(["vcruntime140.dll"]))
            with self.assertRaisesRegex(ValueError, "vcruntime140.dll"):
                helper.closure([exe], [root])
            (root / "vcruntime140.dll").write_bytes(pe([]))
            self.assertEqual(len(helper.closure([exe], [root])), 4)

    def test_registry_relocation_and_corruption(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build, output = root / "producer", root / "bundle"
            build.mkdir()
            exe = build / "test.exe"
            exe.write_bytes(pe(["kernel32.dll"]))
            (build / "CTestTestfile.cmake").write_text(
                f'add_test(example "{exe}")\nset_tests_properties(example PROPERTIES TIMEOUT "12")\n'
            )
            cores = []
            for name in ["shared", "reference"]:
                core = build / name / "obs.dll"
                core.parent.mkdir()
                core.write_bytes(pe([]))
                cores.append(core)
            with patch.object(helper, "exports", return_value={"api"}):
                manifest = helper.stage(
                    build, output, root, [exe, *cores], [root], "llvm-readobj"
                )
            self.assertFalse(manifest["complete"])
            helper.prepare(output)
            registry = (output / "build/CTestTestfile.cmake").read_text()
            self.assertNotIn(str(build), registry)
            self.assertIn('TIMEOUT "12"', registry)
            self.assertEqual(
                json.loads((output / "manifest.json").read_text())["coverage"],
                "probe-only",
            )
            (output / "build/test.exe").write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "changed artifact"):
                helper.prepare(output)

    def test_copy_race_cannot_publish_a_missing_dependency(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build, output = root / "producer", root / "bundle"
            build.mkdir()
            artifacts = []
            for variant in ["shared", "reference"]:
                core = build / variant / "obs.dll"
                core.parent.mkdir()
                core.write_bytes(pe([]))
                artifacts.append(core)
            (build / "CTestTestfile.cmake").write_text("# registry\n")

            def racing_copy(source, destination):
                Path(destination).write_bytes(pe(["unavailable.dll"]))

            with (
                patch.object(helper, "exports", return_value={"api"}),
                patch.object(helper.shutil, "copyfile", side_effect=racing_copy),
            ):
                with self.assertRaisesRegex(ValueError, "unavailable.dll"):
                    helper.stage(build, output, root, artifacts, [], "llvm-readobj")
            self.assertFalse((output / "manifest.json").exists())

    def test_core_variants_are_not_ambiguous_dependencies(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            artifacts = []
            for variant, dlls in [("shared", ["kernel32.dll"]), ("reference", [])]:
                core = root / variant / "obs.dll"
                core.parent.mkdir()
                core.write_bytes(pe(dlls))
                artifacts.append(core)
            self.assertEqual(len(helper.closure(artifacts, [root])), 2)

    def test_manifest_cannot_escape_bundle(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "manifest.json").write_text(
                json.dumps(
                    {
                        "schema": 1,
                        "target": helper.TARGET,
                        "files": {"../escape": "abc"},
                        "ctest_templates": {},
                    }
                )
            )
            with self.assertRaisesRegex(ValueError, "escapes"):
                helper.prepare(root)

    def test_hosted_runtime_is_explicit_and_still_requires_consumer_validation(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            exe = root / "test.exe"
            exe.write_bytes(pe(["vcruntime140.dll", "kernel32.dll"]))
            self.assertEqual(helper.closure([exe], [], hosted_runtime=True), [exe])
            with self.assertRaisesRegex(ValueError, "vcruntime140.dll"):
                helper.closure([exe], [])
            exe.write_bytes(pe(["vcruntime140d.dll"]))
            with self.assertRaisesRegex(ValueError, "vcruntime140d.dll"):
                helper.closure([exe], [], hosted_runtime=True)

    def test_pinned_builtins_overlay_keeps_managed_resources(self):
        from compression import zstd

        helper = importlib.import_module("windows_tier2_toolchain")
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            resource = root / "managed"
            (resource / "include").mkdir(parents=True)
            (resource / "include/xmmintrin.h").write_text("managed intrinsic\n")
            (resource / "share").mkdir()
            payload = b"compiler-rt-test"
            archive = io.BytesIO()
            with tarfile.open(fileobj=archive, mode="w") as tar:
                for name, data in [
                    ("./package/" + helper.LIBRARY, payload),
                    ("./package/LICENSE.TXT", b"license"),
                ]:
                    entry = tarfile.TarInfo(name)
                    entry.size = len(data)
                    tar.addfile(entry, io.BytesIO(data))
            compressed = zstd.compress(archive.getvalue())
            with (
                patch.object(helper, "ARCHIVE_SHA256", helper.sha256(compressed)),
                patch.object(helper, "LIBRARY_SHA256", helper.sha256(payload)),
                patch.object(
                    helper.subprocess, "check_output", return_value=str(resource)
                ),
                patch.object(
                    helper.urllib.request,
                    "urlopen",
                    return_value=io.BytesIO(compressed),
                ),
            ):
                config = helper.provision("soldr-driver", root / "overlay")
                self.assertIn("--rtlib=compiler-rt", config.read_text())
                self.assertIn("-isystem", config.read_text())
                overlay = config.parent / "resource"
                self.assertEqual(
                    (overlay / "include/xmmintrin.h").read_text(), "managed intrinsic\n"
                )
                self.assertEqual(
                    (overlay / "lib/windows/clang_rt.builtins-x86_64.lib").read_bytes(),
                    payload,
                )
                self.assertFalse((resource / "lib").exists())
                (overlay / "lib/windows/clang_rt.builtins-x86_64.lib").write_bytes(
                    b"corrupt"
                )
                with self.assertRaisesRegex(ValueError, "builtins checksum"):
                    helper.provision("soldr-driver", root / "overlay")

    def test_hosted_runtime_requires_real_release_pe_and_recursive_imports(self):
        helper = self.helper()
        manifest = {"hosted_runtime": {"dlls": ["vcruntime140_1.dll"]}}
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaises(FileNotFoundError):
                helper.verify_runtime(manifest, root)
            (root / "vcruntime140_1.dll").write_bytes(pe(["vcruntime140.dll"]))
            with self.assertRaises(FileNotFoundError):
                helper.verify_runtime(manifest, root)
            (root / "vcruntime140.dll").write_bytes(pe(["kernel32.dll"]))
            self.assertEqual(
                set(helper.verify_runtime(manifest, root)),
                {"vcruntime140.dll", "vcruntime140_1.dll"},
            )
            data = bytearray(pe([]))
            struct.pack_into("<H", data, 0x44, 0xAA64)
            (root / "vcruntime140.dll").write_bytes(data)
            with self.assertRaisesRegex(ValueError, "AMD64"):
                helper.verify_runtime(manifest, root)
            (root / "vcruntime140.dll").write_bytes(pe(["arbitrary.dll"]))
            with self.assertRaisesRegex(ValueError, "unexpected installed runtime"):
                helper.verify_runtime(manifest, root)

    def test_hosted_bundle_records_exact_external_imports_and_rechecks_them(self):
        helper = self.helper()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build, output = root / "producer", root / "bundle"
            build.mkdir()
            artifacts = []
            for variant in ["shared", "reference"]:
                core = build / variant / "obs.dll"
                core.parent.mkdir()
                core.write_bytes(pe(["vcruntime140.dll"]))
                artifacts.append(core)
            (build / "CTestTestfile.cmake").write_text("# registry\n")
            with patch.object(helper, "exports", return_value={"api"}):
                manifest = helper.stage(
                    build,
                    output,
                    root,
                    artifacts,
                    [],
                    "llvm-readobj",
                    hosted_runtime=True,
                )
            self.assertEqual(manifest["hosted_runtime"]["dlls"], ["vcruntime140.dll"])
            if helper.os.name != "nt":
                self.assertIsNone(helper.prepare(output)["verified_hosted_runtime"])
            with self.assertRaises(FileNotFoundError):
                helper.prepare(output, runtime_root=root)
            (root / "vcruntime140.dll").write_bytes(pe([]))
            self.assertIn(
                "vcruntime140.dll",
                helper.prepare(output, runtime_root=root)["verified_hosted_runtime"],
            )
            manifest["hosted_runtime"]["dlls"] = []
            (output / "manifest.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "does not match imports"):
                helper.prepare(output)

    def test_builtins_archive_checksum_fails_before_installing(self):
        helper = importlib.import_module("windows_tier2_toolchain")
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "overlay"
            with patch.object(
                helper.urllib.request, "urlopen", return_value=io.BytesIO(b"corrupt")
            ):
                with self.assertRaisesRegex(ValueError, "archive checksum"):
                    helper.install_builtins(output, output / "builtins.lib")
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
