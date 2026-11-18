"""Fail-closed Linux contracts for the disabled Windows hosted proof."""
import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import macos_test_artifact as shared
import macos_test_workspace as archive
import windows_test_artifact as windows
import windows_test_proof as proof
from test_windows_test_artifact import executable


class RootsTest(unittest.TestCase):
    def test_drive_mapping_is_exact_and_rejects_other_roots(self):
        logical = "/opt/obs-win-42-3/src"
        self.assertEqual(proof.windows_root(logical), "C:/opt/obs-win-42-3/src")
        proof.check_root(logical, "C:\\opt\\obs-win-42-3\\src", "C:\\opt\\obs-win-42-3\\src\\rust")
        for root, cwd in [("D:/opt/obs-win-42-3/src", "D:/opt/obs-win-42-3/src"),
                          ("C:/opt/other/src", "C:/opt/other/src"),
                          ("C:/opt/obs-win-42-3/src", "D:/runner/work")]:
            with self.subTest(root=root, cwd=cwd), self.assertRaisesRegex(ValueError, "root|drive"):
                proof.check_root(logical, root, cwd)
        for bad in ("/tmp/src", "/opt/obs-win-42-3/../src", "C:/opt/obs-win-42-3/src", "/opt/obs-win-42-3/src/"):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                proof.windows_root(bad)

    def test_paths_reject_traversal_windows_aliases_and_case_collisions(self):
        for bad in ("../outside", "/root", "C:/outside", "a\\b", "a/../b", "a:stream", "a.", "AUX/x"):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                proof.relative_path(bad)
        with self.assertRaisesRegex(ValueError, "case"):
            proof.check_names(["target/A.exe", "target/a.exe"])


class ConsumerTest(unittest.TestCase):
    def bundle(self, parent):
        bundle = parent / "bundle"
        bundle.mkdir()
        binary = bundle / "suite.exe"
        binary.write_bytes(executable())
        data = {"version": 1, "target": windows.TARGET, "build_root": str(parent / "source"),
                "coverage": windows.PREPARED_COVERAGE, "doctests_complete": True,
                "images": {"suite.exe": windows.pe_imports(binary.read_bytes())},
                "runtime_dlls": ["vcruntime140.dll"], "tests": [{"package": "probe", "binary": "suite.exe",
                    "cwd": ".", "env": {}, "args": []}]}
        shared.finalize(bundle, data)
        return bundle

    def test_full_payload_verification_happens_before_install_or_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = self.bundle(root)
            proof.verify_payload(bundle, require_qt=False)
            (bundle / "unlisted.exe").write_bytes(executable())
            with self.assertRaisesRegex(ValueError, "inventory"):
                proof.verify_payload(bundle, require_qt=False)
            (bundle / "unlisted.exe").unlink()
            data = json.loads((bundle / "manifest.json").read_text())
            data["target"] = "aarch64-apple-darwin"
            (bundle / "manifest.json").write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                proof.verify_payload(bundle, require_qt=False)

    def test_archive_and_receipt_mismatches_fail_before_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = self.bundle(root)
            packet = root / "tests.tar.gz"
            archive.pack(bundle, packet)
            with patch.object(proof, "run_verified") as run:
                with self.assertRaisesRegex(ValueError, "checksum"):
                    proof.consume(packet, root / "source", root / "unpack", "0" * 64, {}, execute=True)
                run.assert_not_called()
            receipt = {"run_id": "42", "run_attempt": "3", "source_sha": "a" * 40,
                       "logical_root": "/opt/obs-win-42-3/src"}
            for field in receipt:
                expected = dict(receipt, **{field: "wrong"})
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, "receipt"):
                    proof.check_receipt(receipt, expected)

    def test_missing_qt_corruption_and_incomplete_payload_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            bundle = self.bundle(root)
            with self.assertRaisesRegex(ValueError, "Qt"):
                proof.verify_payload(bundle, require_qt=True)
            (bundle / "suite.exe").write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "checksum"):
                proof.verify_payload(bundle, require_qt=False)

    def test_runner_reuses_shared_execution_and_retains_logical_root_check(self):
        with patch.object(shared, "execute", return_value=7) as run:
            self.assertEqual(proof.run_verified(Path("manifest.json"), Path("/root")), 7)
            self.assertIs(run.call_args.kwargs["root_validator"], proof.validate_runtime_root)
            self.assertEqual(run.call_args.kwargs["target"], windows.TARGET)


if __name__ == "__main__":
    unittest.main()
