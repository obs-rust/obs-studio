"""Fail-closed contracts for Linux-prepared Darwin native fixtures."""
import json
import tempfile
import unittest
from pathlib import Path

import macos_test_artifact as artifacts


class NativeFixtureTest(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
