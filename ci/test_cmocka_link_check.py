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


class DiscoveryTest(unittest.TestCase):
    def test_uses_generated_target_types_and_names_not_source_regexes(self):
        with tempfile.TemporaryDirectory() as temporary:
            reply = Path(temporary)
            refs = []
            for name, kind in (("test_existing_rust", "EXECUTABLE"),
                               ("test_new_steps_rust", "EXECUTABLE"),
                               ("test_plain", "EXECUTABLE"), ("support_rust", "STATIC_LIBRARY"),
                               ("libobs-cross-shared", "SHARED_LIBRARY")):
                filename = name + ".json"
                (reply / filename).write_text(json.dumps({"name": name, "type": kind}))
                refs.append({"name": name, "jsonFile": filename})
            (reply / "codemodel.json").write_text(json.dumps({"configurations": [{"targets": refs}]}))
            (reply / "index-1.json").write_text(json.dumps({"reply": {"codemodel-v2": {"jsonFile": "codemodel.json"}}}))
            self.assertEqual(cmocka_link_check.discover_targets(reply),
                             ["libobs-cross-shared", "test_existing_rust", "test_new_steps_rust"])

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
