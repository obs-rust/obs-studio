#!/usr/bin/env -S uv run --script
"""The cross-build proof must account for every non-system Mach-O load."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_tier2_audit as audit


class ClosureTest(unittest.TestCase):
    def test_transitive_runtime_is_verified_and_missing_runtime_is_fatal(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable, first, second = [root / name for name in ("probe", "first.dylib", "second.dylib")]
            for image in (executable, first, second):
                image.touch()
            commands = {
                executable: [(0xC, "@rpath/first.dylib"), (0xC, "/usr/lib/libSystem.B.dylib")],
                first: [(0xC, "@loader_path/second.dylib")],
                second: [],
            }
            checked = []
            with patch.object(audit, "loads", side_effect=lambda image: commands[image]):
                result = audit.closure([executable], [root], checked.append)
                self.assertEqual(set(checked), {executable, first, second})
                self.assertEqual(len(result), 3)
                second.rename(root / "absent")
                with self.assertRaisesRegex(ValueError, "second.dylib"):
                    audit.closure([executable], [root], checked.append)

    def test_ambient_absolute_libraries_and_bad_signatures_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            image = Path(directory) / "probe"
            image.touch()
            with patch.object(audit, "loads", return_value=[(0xC, "/host/libnative.dylib")]):
                with self.assertRaisesRegex(ValueError, "unresolved Darwin load"):
                    audit.closure([image], [], lambda _: None)
            def bad_signature(_):
                raise ValueError("changed CodeDirectory page")
            with self.assertRaisesRegex(ValueError, "CodeDirectory"):
                audit.closure([image], [], bad_signature)


if __name__ == "__main__":
    unittest.main()
