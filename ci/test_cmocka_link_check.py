#!/usr/bin/env -S uv run --script
"""Pre-queue Tier 2 cross-link selection and configured target discovery."""
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import ci_scope
import cmocka_link_check


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
                     mock.patch.object(cmocka_link_check.subprocess, "run") as run:
                    cmocka_link_check.build_targets(alias, work, 3, {})
                args = run.call_args.args[0]
                self.assertEqual(args[args.index("--parallel") + 1], "3")
                self.assertEqual(args[args.index("--target") + 1:args.index("--")], inventory)
                self.assertEqual(json.loads((work / "link-targets.json").read_text()), inventory)

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
