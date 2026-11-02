#!/usr/bin/env -S uv run --script
"""ci/windows_sign.py lists every shipped Windows binary and repacks signed ones."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci/windows_sign.py"


def run(*args):
    return subprocess.run(
        [sys.executable, str(SCRIPT), *map(str, args)],
        capture_output=True,
        text=True,
    )


class WindowsSign(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.zip = self.tmp / "obs-studio-1-windows-x64.zip"
        with zipfile.ZipFile(self.zip, "w") as z:
            z.writestr("bin/64bit/obs64.exe", b"exe")
            z.writestr("bin/64bit/obs.dll", b"dll")
            z.writestr("data/obs-scripting/64bit/_obspython.PYD", b"pyd")
            z.writestr("obs-plugins/64bit/win-capture.dll", b"plug")
            z.writestr("data/locale/en-US.ini", b"ini")
        self.msi = self.tmp / "OBS.msi"
        self.msi.write_bytes(b"msi")

    def test_catalog_lists_every_binary_and_msi(self):
        work, cat = self.tmp / "w", self.tmp / "c.txt"
        r = run("unpack", "--work", work, "--catalog", cat, self.zip, self.msi)
        self.assertEqual(r.returncode, 0, r.stderr)
        names = sorted(Path(p).name for p in cat.read_text().splitlines())
        self.assertEqual(
            names,
            ["OBS.msi", "_obspython.PYD", "obs.dll", "obs64.exe", "win-capture.dll"],
        )

    def test_repack_writes_signed_files_in_original_order(self):
        work, cat = self.tmp / "w", self.tmp / "c.txt"
        self.assertEqual(run("unpack", "--work", work, "--catalog", cat, self.zip).returncode, 0)
        before = [i.filename for i in zipfile.ZipFile(self.zip).infolist()]
        for p in cat.read_text().splitlines():
            Path(p).write_bytes(Path(p).read_bytes() + b"+sig")
        r = run("repack", "--work", work, self.zip)
        self.assertEqual(r.returncode, 0, r.stderr)
        with zipfile.ZipFile(self.zip) as z:
            self.assertEqual([i.filename for i in z.infolist()], before)
            self.assertEqual(z.read("bin/64bit/obs.dll"), b"dll+sig")
            self.assertEqual(z.read("data/locale/en-US.ini"), b"ini")

    def test_no_binaries_fails(self):
        empty = self.tmp / "empty.zip"
        with zipfile.ZipFile(empty, "w") as z:
            z.writestr("readme.txt", b"x")
        r = run("unpack", "--work", self.tmp / "w", "--catalog", self.tmp / "c", empty)
        self.assertNotEqual(r.returncode, 0)

    def test_zip_slip_rejected(self):
        bad = self.tmp / "bad.zip"
        with zipfile.ZipFile(bad, "w") as z:
            z.writestr("../evil.dll", b"x")
        r = run("unpack", "--work", self.tmp / "w", "--catalog", self.tmp / "c", bad)
        self.assertNotEqual(r.returncode, 0)

    def test_release_workflow_signs_and_verifies_windows(self):
        wf = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        self.assertIn("azure/artifact-signing-action@208f8af4bf26cf2af8597424e3cb5582801523ba", wf)
        for name in ("AZURE_TENANT_ID", "AZURE_CLIENT_ID", "AZURE_CLIENT_SECRET",
                     "AZURE_SIGNING_ENDPOINT", "AZURE_SIGNING_ACCOUNT", "AZURE_SIGNING_PROFILE"):
            self.assertIn(f"secrets.{name}", wf)
        self.assertIn("Get-AuthenticodeSignature", wf)
        self.assertIn("startsWith(github.ref, 'refs/tags/v')", wf)
        sign = wf.index("Sign Windows binaries")
        self.assertLess(sign, wf.index("Repack signed Windows package"))
        self.assertLess(wf.index("Repack signed Windows package"), wf.index("- name: Collect and verify"))


if __name__ == "__main__":
    unittest.main()
