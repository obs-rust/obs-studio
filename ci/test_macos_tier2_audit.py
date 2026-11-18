#!/usr/bin/env -S uv run --script
"""The cross-build proof must account for every non-system Mach-O load."""

import tempfile
import struct
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_tier2_audit as audit


class ClosureTest(unittest.TestCase):
    def test_executable_path_stays_anchored_to_the_entry_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "bin/probe"
            library = root / "lib/main.dylib"
            decoy = root / "lib/private/dep.dylib"
            for path in (executable, library, decoy):
                path.parent.mkdir(parents=True, exist_ok=True)
                path.touch()
            executable.write_bytes(struct.pack("<4I", 0xFEEDFACF, 0x100000C, 0, 2))
            commands = {executable: [(0xC, "@rpath/main.dylib")],
                        library: [(0xC, "@executable_path/private/dep.dylib")], decoy: []}
            with patch.object(audit, "loads", side_effect=lambda image: commands[image]):
                with self.assertRaisesRegex(ValueError, "executable_path"):
                    audit.closure([executable], [root / "lib"], lambda _: None)
                actual = root / "bin/private/dep.dylib"
                actual.parent.mkdir()
                actual.touch()
                commands[actual] = []
                result = audit.closure([executable], [root / "lib", actual.parent], lambda _: None)
                self.assertIn(str(actual), result)
                self.assertNotIn(str(decoy), result)

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
