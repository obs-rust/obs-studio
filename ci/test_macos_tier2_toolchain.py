#!/usr/bin/env -S uv run --script
"""Linux Darwin provisioning must retain sanitizers and Swift without fallback."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_tier2_toolchain as toolchain


class DarwinToolchainTest(unittest.TestCase):
    def receipt(self, root):
        fields = ("clang", "clangxx", "ar", "ranlib", "dsymutil", "swiftc",
                  "sdk", "clang_resource_dir", "compiler_rt", "swift_resource_dir",
                  "cmake_toolchain_file")
        result = {key: str(root / key) for key in fields}
        for key in fields:
            path = Path(result[key])
            if key in ("sdk", "compiler_rt", "clang_resource_dir", "swift_resource_dir"):
                path.mkdir()
            else:
                path.touch()
        for name in toolchain.RUNTIMES:
            (Path(result["compiler_rt"]) / name).touch()
        result.update(schema_version=1, target=toolchain.TARGET, sdk_version="26.5",
                      swift_version="6.3.2", identity="fixture",
                      c_compile_args=["--target=arm64-apple-macosx13.0"],
                      c_link_args=["-fuse-ld=lld"],
                      swift_compile_args=["-resource-dir", result["swift_resource_dir"]],
                      swift_link_args=["-use-ld=lld"])
        return result

    def test_managed_command_and_explicit_local_override(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipt = self.receipt(root)
            with patch.object(toolchain.subprocess, "run") as run:
                run.return_value.stdout = json.dumps(receipt)
                actual = toolchain.provision(root / "out", {"OBS_TIER2_SOLDR": "/proof/soldr"})
            self.assertEqual(run.call_args.args[0], ["/proof/soldr", "toolchain", "darwin",
                             "--target", toolchain.TARGET, "--sdk-version", "26.5", "--json"])
            self.assertEqual(actual["identity"], "fixture")
            self.assertTrue(run.call_args.kwargs["check"])

    def test_missing_runtime_fails_before_wrapper_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipt = self.receipt(root)
            (Path(receipt["compiler_rt"]) / "libclang_rt.osx.a").unlink()
            with self.assertRaisesRegex(ValueError, "libclang_rt.osx.a"):
                toolchain.validate(receipt)

    def test_wrong_sdk_or_target_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            receipt = self.receipt(Path(directory))
            for key, value in (("sdk_version", "14.5"), ("target", "x86_64-apple-darwin")):
                wrong = {**receipt, key: value}
                with self.assertRaises(ValueError):
                    toolchain.validate(wrong)

    def test_swift_wrapper_uses_managed_resources_linker_and_debug_tool(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            receipt = self.receipt(root)
            environment = toolchain.environment(receipt, root / "wrappers")
            swift = Path(environment["SWIFTC"]).read_text()
            self.assertIn(receipt["swiftc"], swift)
            self.assertIn(receipt["swift_resource_dir"], swift)
            self.assertIn("-use-ld=lld", swift)
            self.assertEqual(environment["DSYMUTIL"], receipt["dsymutil"])
            self.assertNotIn("xcrun", swift)


if __name__ == "__main__":
    unittest.main()
