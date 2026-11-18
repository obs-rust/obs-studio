"""Windows doctest and native preparation contracts; no hosted activation."""
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_test_artifact as shared
import windows_test_artifact as windows
from test_windows_test_artifact import executable


class WindowsRunOnlyTest(unittest.TestCase):
    def test_doctest_capture_uses_pe_exe_and_preserves_harness_arguments(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = root / "rust_out.exe"
            binary.write_bytes(executable())
            entry = shared.capture_doctest(binary, ["--test-threads=1"], root,
                root / "bundle", root, {"CARGO_PKG_NAME": "probe"},
                target=windows.TARGET, verify_executable=windows.verify_pe)
            self.assertTrue(entry["binary"].endswith("/rust_out.exe"))
            self.assertIn("windows-doctests/", entry["binary"])
            self.assertEqual(entry["args"], ["--test-threads=1"])

    def test_doctest_linker_is_msvc_and_capture_target_is_explicit(self):
        with patch.object(shared.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "/soldr/lld-link\n")) as run:
            env = shared.doctest_environment(Path("/src"), Path("/bundle"), {}, target=windows.TARGET)
        self.assertEqual(run.call_args.args[0], ["soldr", "cc", "--target", windows.TARGET, "--print-linker"])
        self.assertIn("linker=/soldr/lld-link", env["RUSTDOCFLAGS"])
        self.assertEqual(env["OBS_CROSS_DOCTEST_TARGET"], windows.TARGET)

    def test_windows_native_inventory_and_missing_manifest_fail_closed(self):
        import windows_native_fixtures as fixtures
        self.assertIn("windows_resource_payload_tracks_headers_definitions_and_unity", fixtures.TESTS)
        self.assertIn("msvc_pch_links_its_object_and_preserves_language_header_and_flag_changes", fixtures.TESTS)
        with tempfile.TemporaryDirectory() as temporary:
            with self.assertRaises((ValueError, FileNotFoundError)):
                fixtures.validate(Path(temporary))

    def test_windows_contracts_use_shared_transport(self):
        import re
        root = Path(__file__).resolve().parents[1]
        source = (root / "rust/obs-build/tests/native.rs").read_text()
        for name in ("windows_resource_payload_tracks_headers_definitions_and_unity",
                     "msvc_pch_links_its_object_and_preserves_language_header_and_flag_changes"):
            body = source.split("fn " + name + "()", 1)[1].split("\nfn ", 1)[0]
            self.assertIn("prebuilt::Scope::enter()", body)
            self.assertNotRegex(body, r"\.(shared_library|static_library|executable)\(")
            self.assertIsNotNone(re.search(r"fixture_shared_library", body))

    def test_fixture_dll_imports_allow_only_staged_dependencies(self):
        data = bytearray(executable(["fixture-core.dll", "KERNEL32.dll"]))
        import struct
        struct.pack_into("<H", data, 0x56, 0x2022)
        self.assertEqual(windows.pe_imports(bytes(data), allow_dll=True, staged_dlls={"fixture-core.dll"}),
                         ["fixture-core.dll", "kernel32.dll"])
        with self.assertRaises(ValueError):
            windows.pe_imports(bytes(data), allow_dll=True)


if __name__ == "__main__":
    unittest.main()
