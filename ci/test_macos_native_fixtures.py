"""Fail-closed contracts for Linux-prepared Darwin native fixtures."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_test_artifact as artifacts


class NativeFixtureTest(unittest.TestCase):
    def test_complete_inventory_and_missing_per_test_manifest(self):
        from macos_native_fixtures import TESTS
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory = root / "native-fixtures"
            directory.mkdir()
            names = ["native_build::" + name for name in TESTS]
            (directory / "index.json").write_text(json.dumps({"version": 1, "target": artifacts.TARGET,
                "tests": names, "signed": ["fixture.dylib"], "complete": True}))
            for name in names:
                path = directory / name.replace("::", "--") / "manifest.json"
                path.parent.mkdir()
                path.write_text(json.dumps({"host": "x86_64-unknown-linux-gnu", "target": artifacts.TARGET,
                    "complete": True, "events": [{}]}))
            with patch.object(artifacts, "verify_macho") as verify:
                artifacts.validate_native_fixtures(root)
                verify.assert_called_once_with(directory / "fixture.dylib", allowed_filetypes=(2, 6, 8))
                path.unlink()
                with self.assertRaises(FileNotFoundError):
                    artifacts.validate_native_fixtures(root)

    def test_fixture_manifest_is_required_and_complete(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises((ValueError, FileNotFoundError)):
                artifacts.validate_native_fixtures(root)
            (root / "native-fixtures").mkdir()
            (root / "native-fixtures/index.json").write_text(json.dumps({
                "version": 1, "target": artifacts.TARGET, "tests": [], "complete": False,
            }))
            with self.assertRaises(ValueError):
                artifacts.validate_native_fixtures(root)

    def test_dylib_signature_uses_same_page_verifier(self):
        # Defaults still reject shared images in the executable collector.
        from test_macos_test_artifact import SignatureTest
        import struct
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "fixture.dylib"
            data = bytearray(SignatureTest.signed_image())
            struct.pack_into("<I", data, 12, 6)
            # Changing the header without re-signing must fail, even for a dylib.
            path.write_bytes(data)
            with self.assertRaises(ValueError):
                artifacts.verify_macho(path, allowed_filetypes=(2, 6, 8))


class FixtureInventoryTest(unittest.TestCase):
    def test_preparation_rejects_macos_before_starting_any_tool(self):
        import macos_native_fixtures as fixtures
        with patch.object(fixtures.platform, "system", return_value="Darwin"), patch.object(fixtures.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "Linux producer"):
                fixtures.prepare(Path("unused"), Path("unused"), {})
            run.assert_not_called()

    def test_cross_tool_settings_replace_both_target_spellings(self):
        import macos_native_fixtures as fixtures
        for name in ("CC", "CXX", "AR", "SDKROOT", "SWIFTC"):
            environment = {f"{name}_aarch64-apple-darwin": "inherited-hyphen",
                           f"{name}_aarch64_apple_darwin": "inherited-underscore"}
            fixtures.target_setting(environment, name, "managed")
            self.assertEqual(set(environment.values()), {"managed"})
        environment = {"SWIFTC_aarch64-apple-darwin": "/hyphen/swiftc",
                       "SWIFTC_aarch64_apple_darwin": "/underscore/swiftc"}
        self.assertEqual(fixtures.swift_compiler(Path("unused"), environment), Path("/hyphen/swiftc"))

    def test_every_default_darwin_compile_contract_uses_transport(self):
        import re
        from macos_native_fixtures import TESTS
        root = Path(__file__).resolve().parents[1]
        source = (root / "rust/obs-build/tests/native.rs").read_text()
        definitions = list(re.finditer(r"(?m)^fn (\w+)\(", source))
        blocks = {match[1]: source[match.start():definitions[index + 1].start() if index + 1 < len(definitions) else len(source)]
                  for index, match in enumerate(definitions)}
        for name in TESTS:
            if "::" in name:
                continue
            with self.subTest(name=name):
                block = blocks[name]
                self.assertIn("prebuilt::Scope::enter()", block)
                self.assertNotRegex(block, r"\.(shared_library|swift_shared_library|static_library|executable|compile)\(")
                self.assertNotIn("target: obs_build::BUILD_TARGET.into()", block)

    def test_build_bundles_fixtures_before_final_checksums(self):
        import inspect
        source = inspect.getsource(artifacts.build)
        self.assertLess(source.index("prepare(ROOT, output, env)"), source.index("finalize(output, manifest)"))
        self.assertIn('entry["env"]["OBS_NATIVE_FIXTURES"]', source)


if __name__ == "__main__":
    unittest.main()
