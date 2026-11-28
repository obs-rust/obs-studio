#!/usr/bin/env -S uv run --script
"""Linux preparation of the Cargo-native macOS contract (#5009, #5848)."""
from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import macos_tier2_prepare as prepare


class EnvironmentTest(unittest.TestCase):
    receipt = {"clang": "/t/bin/clang", "clangxx": "/t/bin/clang++", "ar": "/t/bin/llvm-ar", "swiftc": "/t/swiftc",
               "sdk": "/t/sdk", "c_compile_args": ["--target=arm64-apple-macosx11.0"], "c_link_args": ["-fuse-ld=lld"],
               "swift_compile_args": ["-target", "arm64-apple-macosx11.0"], "swift_link_args": ["-use-ld=lld"]}

    def test_toolchain_wrappers_carry_the_receipt_flags_and_sdk(self):
        with tempfile.TemporaryDirectory() as temporary:
            env = prepare.toolchain_environment(self.receipt, Path(temporary) / "w")
            self.assertIn("--target=arm64-apple-macosx11.0", Path(env["CC_aarch64_apple_darwin"]).read_text())
            self.assertIn("/t/bin/clang++", Path(env["CXX_aarch64_apple_darwin"]).read_text())
            self.assertIn("-use-ld=lld", Path(env["SWIFTC"]).read_text())
            self.assertEqual((env["SDKROOT"], env["OBS_BUILD_MACOS_NM"]), ("/t/sdk", "/t/bin/llvm-nm"))
            self.assertEqual(env["SOLDR_PORCELAIN"], "0")

    def test_preparation_and_assertions_share_every_path_but_the_mode(self):
        root, work, deps = Path("/r"), Path("/r/build"), Path("/r/.deps/d")
        prepared = prepare.contract_environment(root, work, deps, assert_only=False)
        asserted = prepare.contract_environment(root, work, deps, assert_only=True)
        self.assertIn("OBS_BUILD_MACOS_PREPARE_ONLY", prepared)
        self.assertIn("OBS_BUILD_MACOS_ASSERT_ONLY", asserted)
        shared = lambda env: {k: v for k, v in env.items() if k not in ("OBS_BUILD_MACOS_PREPARE_ONLY", "OBS_BUILD_MACOS_ASSERT_ONLY")}
        self.assertEqual(shared(prepared), shared(asserted))
        for name in ("REFERENCE", "OPENGL_REFERENCE", "METAL_REFERENCE", "MAC_CAPTURE_REFERENCE",
                     "MAC_AVCAPTURE_REFERENCE", "MAC_AVCAPTURE_LEGACY_REFERENCE"):
            self.assertTrue(asserted[f"OBS_BUILD_MACOS_{name}"].startswith("/r/build/"), name)
        self.assertTrue(asserted["OBS_BUILD_MACOS_MAC_CAPTURE_REFERENCE"].endswith("mac-capture.plugin/Contents/MacOS/mac-capture"))
        self.assertTrue(asserted["OBS_BUILD_MACOS_REFERENCE"].endswith("libobs-c-reference/libobs.framework/Versions/A/libobs"))


class ExecutableTest(unittest.TestCase):
    def run_with(self, stdout: str, code: int = 0):
        with mock.patch.object(prepare.subprocess, "run", return_value=mock.Mock(stdout=stdout, stderr="boom", returncode=code)):
            return prepare.test_executable(Path("/r"), {})

    def test_exactly_one_contract_executable_is_required(self):
        artifact = json.dumps({"reason": "compiler-artifact", "executable": "/t/obs_tests_app-1", "target": {"name": "obs-tests-app"}})
        other = json.dumps({"reason": "compiler-artifact", "executable": "/t/other", "target": {"name": "other"}})
        self.assertEqual(self.run_with(f"{other}\n{artifact}\n"), Path("/t/obs_tests_app-1"))
        with self.assertRaisesRegex(ValueError, "found 0"):
            self.run_with(f"{other}\n")
        with self.assertRaisesRegex(ValueError, "found 2"):
            self.run_with(f"{artifact}\n{artifact}\n")
        with self.assertRaisesRegex(RuntimeError, "boom"):
            self.run_with("", code=1)


class AssertTest(unittest.TestCase):
    def test_assertions_run_only_the_prebuilt_contract_and_fail_loudly(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / ".deps/obs-deps-2026-universal").mkdir(parents=True)
            work = root / "build"
            with self.assertRaises(FileNotFoundError):
                prepare.assertions(root, work)
            (work / prepare.CONTRACT_DIRECTORY).mkdir(parents=True)
            (work / prepare.CONTRACT_DIRECTORY / "obs-tests-app").write_bytes(b"x")
            with mock.patch.object(prepare.subprocess, "run") as run:
                prepare.assertions(root, work)
            args, kwargs = run.call_args
            self.assertEqual(args[0][1:], [prepare.TEST, "--exact", "--nocapture"])
            self.assertIn("OBS_BUILD_MACOS_ASSERT_ONLY", kwargs["env"])
            self.assertNotIn("OBS_BUILD_MACOS_PREPARE_ONLY", kwargs["env"])
            self.assertTrue(kwargs["check"])


if __name__ == "__main__":
    unittest.main()
