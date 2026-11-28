#!/usr/bin/env -S uv run --script
"""Pre-queue Tier 2 cross-link selection and configured target discovery."""
from __future__ import annotations

import hashlib
import json
import struct
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import ci_scope
import cmocka_link_check
import test_macos_test_artifact


class ScopeTest(unittest.TestCase):
    def test_registration_and_support_select_link_checks(self):
        for path in ("libobs/obs-source.c.h", "libobs/cmake/rust-hidden.d/new.txt",
                     "rust/libobs-rust/src/lib.rs", "rust/obs-build/src/native.rs",
                     "rust/obs-build/test-graph.json", "test/cmocka/tests.d/new_link_support.cmake",
                     "ci/cmocka_link_check.py", "cmake/cross/tier2.cmake", "CMakeLists.txt",
                     "libobs/build/source-sets.d/LIBOBS_WINDOWS_SOURCES.json"):
            with self.subTest(path=path):
                for event in ("pull_request", "merge_group"):
                    self.assertTrue(ci_scope.scope([path], event=event).cmocka_link)

    def test_linked_rust_crates_follow_the_real_dependency_closure(self):
        root = Path(__file__).resolve().parent.parent

        def read(path):
            source = root / path
            return source.read_text() if source.is_file() else ""

        self.assertTrue(ci_scope.scope(["rust/obs-util/src/ffi/bmem.rs"], read=read).cmocka_link)

    def test_unrelated_changes_keep_the_cheaper_category_check(self):
        for path in ("docs/ci-tooling.md", "frontend/widgets/foo.cpp", "ci/backpressure.py"):
            with self.subTest(path=path):
                self.assertFalse(ci_scope.scope([path], event="pull_request").cmocka_link)


class ToolchainTest(unittest.TestCase):
    def test_windows_uses_managed_clang_cl_and_the_target_sdk(self):
        with tempfile.TemporaryDirectory(prefix="tier2 sdk ") as temporary:
            work = Path(temporary)
            (work / "soldr-tools").mkdir()
            llvm = work / "llvm"
            llvm.mkdir()
            (llvm / "clang-cl").touch()
            sdk = work / "sdk"
            values = {"--print-ar": str(llvm / "llvm-lib"),
                      "--print-rc": str(llvm / "llvm-rc"), "--print-sysroot": str(sdk)}
            with mock.patch.object(cmocka_link_check.subprocess, "run",
                                   side_effect=lambda args, **kw: mock.Mock(stdout=values[args[-1]])):
                toolchain = cmocka_link_check.windows_toolchain(work, "x86_64-pc-windows-msvc")
            content = toolchain.read_text()
            self.assertIn("MultiThreadedDLL", content)
            self.assertIn("lld-link", content)
            self.assertIn(f'/libpath:"{sdk}/sdk/lib/um/x86_64"', content)
            wrapper = (work / "soldr-tools/clang-cl").read_text()
            self.assertIn("-imsvc", wrapper)
            self.assertNotIn("-isystem", wrapper)
            self.assertIn("--target=x86_64-pc-windows-msvc", wrapper)


class BuildTest(unittest.TestCase):
    def test_each_alias_builds_the_full_inventory_at_the_cpu_budget(self):
        inventory = ["libobs-cross-shared", "test_existing_rust", "test_new_steps_rust"]
        for alias in cmocka_link_check.TRIPLES:
            with self.subTest(alias=alias), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                with mock.patch.object(cmocka_link_check, "discover_targets", return_value=inventory), \
                     mock.patch.object(cmocka_link_check, "validate_msvc_providers"), \
                     mock.patch.object(cmocka_link_check, "verify_signed_images"), \
                     mock.patch.object(cmocka_link_check, "record_registry"), \
                     mock.patch.object(cmocka_link_check.subprocess, "run") as run:
                    cmocka_link_check.build_targets(alias, work, 3, {})
                expected = inventory + (cmocka_link_check.MAC_CLOSURE_TARGETS if alias == "mac-arm64" else [])
                args = run.call_args.args[0]
                self.assertEqual(args[args.index("--parallel") + 1], "3")
                self.assertEqual(args[args.index("--target") + 1:args.index("--")], expected)
                self.assertEqual(json.loads((work / "link-targets.json").read_text()), expected)

    def test_alias_link_failures_propagate(self):
        import subprocess

        for alias in cmocka_link_check.TRIPLES:
            with self.subTest(alias=alias), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                failure = subprocess.CalledProcessError(1, ["cmake", "--build"])
                with mock.patch.object(cmocka_link_check, "discover_targets", return_value=["libobs-cross-shared", "test_existing_rust"]), \
                     mock.patch.object(cmocka_link_check, "validate_msvc_providers"), \
                     mock.patch.object(cmocka_link_check.subprocess, "run", side_effect=failure):
                    with self.assertRaises(subprocess.CalledProcessError):
                        cmocka_link_check.build_targets(alias, work, 2, {})

    def test_kept_tree_uses_a_stable_verified_launcher_with_a_fresh_cache(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            tools = work / "build" / "compiler-cache"
            with mock.patch.object(cmocka_link_check.tier2_front_cache, "install", return_value=tools / "ccache") as install:
                env = cmocka_link_check.cache_env(work, tools)
            install.assert_called_once_with(tools)
            self.assertEqual(env["CMAKE_C_COMPILER_LAUNCHER"], str(tools / "ccache"))
            self.assertEqual(env["CCACHE_DIR"], str(work / "objects"))

    def test_cache_is_fresh_and_cannot_inherit_relaxed_or_remote_policy(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            with mock.patch.object(cmocka_link_check.tier2_front_cache, "install", return_value=work / "ccache"), \
                 mock.patch.dict("os.environ", {"CCACHE_SLOPPINESS": "time_macros", "CCACHE_REMOTE_STORAGE": "remote", "CCACHE_PREFIX": "unsafe"}):
                env = cmocka_link_check.cache_env(work)
            self.assertNotIn("CCACHE_SLOPPINESS", env)
            self.assertNotIn("CCACHE_REMOTE_STORAGE", env)
            self.assertNotIn("CCACHE_PREFIX", env)
            self.assertEqual(Path(env["CCACHE_CONFIGPATH"]).read_text(), "")
            self.assertEqual(env["CCACHE_MAXSIZE"], "512M")
            self.assertEqual(env["CCACHE_COMPILERCHECK"], "content")


class MacClosureTest(unittest.TestCase):
    """mac-arm64 builds and signs the whole CTest closure on Linux (#5009)."""

    def test_mac_builds_every_cmocka_test_but_windows_keeps_the_rust_inventory(self):
        inventory = ["libobs-cross-shared", "test_existing_rust"]
        for alias, closure in (("mac-arm64", True), ("win-x64", False)):
            with self.subTest(alias=alias), tempfile.TemporaryDirectory() as temporary:
                work = Path(temporary)
                with mock.patch.object(cmocka_link_check, "discover_targets", return_value=inventory), \
                     mock.patch.object(cmocka_link_check, "validate_msvc_providers"), \
                     mock.patch.object(cmocka_link_check, "verify_signed_images") as verify, \
                     mock.patch.object(cmocka_link_check, "record_registry"), \
                     mock.patch.object(cmocka_link_check.subprocess, "run") as run:
                    cmocka_link_check.build_targets(alias, work, 2, {})
                args = run.call_args_list[0].args[0]
                targets = args[args.index("--target") + 1:args.index("--")]
                self.assertEqual("cmocka-tests" in targets, closure)
                self.assertEqual(verify.called, closure)

    def test_every_linked_image_must_carry_a_valid_signature(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            (work / "test").mkdir()
            image = test_macos_test_artifact.SignatureTest.signed_image()
            (work / "test/test_a").write_bytes(image)
            dylib = bytearray(image)
            struct.pack_into("<I", dylib, 12, 6)
            # Re-seal the single signed page: hash sits after the 20-byte superblob and 44-byte header.
            dylib[4096 + 64:4096 + 96] = hashlib.sha256(bytes(dylib[:4096])).digest()
            (work / "test/libx.dylib").write_bytes(bytes(dylib))
            obj = bytearray(4096)
            struct.pack_into("<8I", obj, 0, 0xFEEDFACF, 0x100000C, 0, 1, 0, 0, 0, 0)
            (work / "test/x.o").write_bytes(bytes(obj))
            (work / "test/notes.txt").write_text("text")
            self.assertEqual(cmocka_link_check.verify_signed_images(work), 2)
            unsigned = bytearray(image)
            struct.pack_into("<I", unsigned, 16, 0)
            (work / "test/test_b").write_bytes(bytes(unsigned))
            with self.assertRaisesRegex(ValueError, "missing arm64 code signature"):
                cmocka_link_check.verify_signed_images(work)

    def test_no_images_is_a_failure_not_a_pass(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaisesRegex(ValueError, "no linked Mach-O"):
                cmocka_link_check.verify_signed_images(Path(temporary))

    def test_darwin_runtime_comes_from_the_managed_receipt_and_must_exist(self):
        with tempfile.TemporaryDirectory() as temporary:
            runtime = Path(temporary) / "lib/darwin"
            runtime.mkdir(parents=True)
            names = ("libclang_rt.osx.a", "libclang_rt.asan_osx_dynamic.dylib", "libclang_rt.ubsan_osx_dynamic.dylib")
            for name in names:
                (runtime / name).touch()
            receipt = json.dumps({"compiler_rt": str(runtime), "clang_resource_dir": str(Path(temporary))})
            with mock.patch.object(cmocka_link_check.subprocess, "run", return_value=mock.Mock(stdout=receipt)) as run:
                flags = cmocka_link_check.darwin_runtime_flags()
            self.assertEqual(run.call_args.args[0][:3], ["soldr", "toolchain", "darwin"])
            self.assertEqual(flags, {"CMAKE_EXE_LINKER_FLAGS": f"{runtime / 'libclang_rt.osx.a'} -resource-dir {temporary}"})
            self.assertEqual(sorted(p.name for p in cmocka_link_check.sanitizer_runtimes(Path(temporary) / "lib/darwin")),
                             sorted(names[1:]))
            for name in names:
                (runtime / name).unlink()
                with mock.patch.object(cmocka_link_check.subprocess, "run", return_value=mock.Mock(stdout=receipt)):
                    with self.assertRaisesRegex(FileNotFoundError, name):
                        cmocka_link_check.darwin_runtime_flags()
                (runtime / name).touch()

    def test_sanitizers_stay_off_only_for_windows(self):
        self.assertEqual(cmocka_link_check.sanitizer_option("win-x64"), "-DOBS_TEST_SANITIZE_PER_TARGET=OFF")
        self.assertEqual(cmocka_link_check.sanitizer_option("mac-arm64"), "-DOBS_TEST_SANITIZE_PER_TARGET=ON")

    def test_registry_must_contain_every_native_macos_test(self):
        listing = "Test project x\n" + "".join(f"  Test #{i}: {name}\n" for i, name in enumerate(["a", "b", "c"], 1))
        with tempfile.TemporaryDirectory() as temporary:
            snapshot = Path(temporary) / "names.txt"
            snapshot.write_text("a\nb\n")
            cmocka_link_check.check_registry_parity(listing, snapshot)
            snapshot.write_text("a\nb\nmissing_owner_test\n")
            with self.assertRaisesRegex(ValueError, "missing_owner_test"):
                cmocka_link_check.check_registry_parity(listing, snapshot)


class DiscoveryTest(unittest.TestCase):
    def test_msvc_rejects_full_dll_imports_beside_the_rust_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            reply = Path(temporary)
            target = {"name": "fixture", "type": "EXECUTABLE", "link": {"commandFragments": [
                {"fragment": "libobs_rust.lib", "role": "libraries"},
                {"fragment": '"/build/obs.lib"', "role": "libraries"},
            ]}}
            (reply / "fixture.json").write_text(json.dumps(target))
            (reply / "codemodel.json").write_text(json.dumps({"configurations": [{
                "targets": [{"name": "fixture", "jsonFile": "fixture.json"}]}]}))
            (reply / "index-1.json").write_text(json.dumps({"reply": {
                "codemodel-v2": {"jsonFile": "codemodel.json"}}}))
            with self.assertRaisesRegex(ValueError, "must not mix"):
                cmocka_link_check.validate_msvc_providers(reply)
            target["link"]["commandFragments"].pop()
            (reply / "fixture.json").write_text(json.dumps(target))
            cmocka_link_check.validate_msvc_providers(reply)

    def test_uses_generated_target_types_and_names_not_source_regexes(self):
        with tempfile.TemporaryDirectory() as temporary:
            reply = Path(temporary)
            refs = []
            for name, kind in (("test_existing_rust", "EXECUTABLE"),
                               ("test_new_steps_rust", "EXECUTABLE"),
                                ("test_plain", "EXECUTABLE"), ("support_rust", "STATIC_LIBRARY"),
                                ("test_hotkey_background_enum", "EXECUTABLE"),
                                ("test_video_level_queries", "EXECUTABLE"),
                                ("libobs_rust-static", "STATIC_LIBRARY"),
                               ("libobs-cross-shared", "SHARED_LIBRARY")):
                filename = name + ".json"
                target = {"id": name, "name": name, "type": kind}
                if name == "test_hotkey_background_enum":
                    target["dependencies"] = [{"id": "libobs_rust-static"}]
                if name == "test_video_level_queries":
                    target["link"] = {"commandFragments": [{"fragment": '"/build/libobs_rust.lib"', "role": "libraries"}]}
                (reply / filename).write_text(json.dumps(target))
                refs.append({"id": name, "name": name, "jsonFile": filename})
            (reply / "codemodel.json").write_text(json.dumps({"configurations": [{"targets": refs}]}))
            (reply / "index-1.json").write_text(json.dumps({"reply": {"codemodel-v2": {"jsonFile": "codemodel.json"}}}))
            self.assertEqual(cmocka_link_check.discover_targets(reply),
                              ["libobs-cross-shared", "test_existing_rust", "test_hotkey_background_enum", "test_new_steps_rust", "test_video_level_queries"])

    def test_existing_reply_without_rust_executables_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            reply = Path(temporary)
            (reply / "codemodel.json").write_text(json.dumps({"configurations": [{"targets": []}]}))
            (reply / "index-1.json").write_text(json.dumps({"reply": {"codemodel-v2": {"jsonFile": "codemodel.json"}}}))
            with self.assertRaises(ValueError):
                cmocka_link_check.discover_targets(reply)

    def test_missing_reply_or_empty_rust_inventory_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises((ValueError, FileNotFoundError)):
                cmocka_link_check.discover_targets(Path(temporary))


if __name__ == "__main__":
    unittest.main()
