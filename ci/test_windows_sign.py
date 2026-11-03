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

    def test_azure_catalog_is_relative_to_its_own_directory(self):
        work, cat = self.tmp / "work", self.tmp / "all.txt"
        self.assertEqual(run("unpack", "--work", work, "--catalog", cat,
                             self.zip, self.msi).returncode, 0)
        original = cat.read_bytes()
        # The unsigned catalog and Azure catalog may live in different folders.
        azure = self.tmp / "azure" / "unsigned.txt"
        azure.parent.mkdir()
        r = run("azure-catalog", "--catalog", cat, "--output", azure)
        self.assertEqual(r.returncode, 0, r.stderr)
        entries = azure.read_text().splitlines()
        self.assertEqual(len(entries), 5)
        for entry, absolute in zip(entries, cat.read_text().splitlines()):
            self.assertFalse(Path(entry).is_absolute())
            self.assertEqual((azure.parent / entry).resolve(), Path(absolute))
            self.assertTrue((azure.parent / entry).is_file())
        self.assertEqual(cat.read_bytes(), original)

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
        for name in ("AZURE_TENANT_ID", "AZURE_CLIENT_ID",
                     "AZURE_SIGNING_ENDPOINT", "AZURE_SIGNING_ACCOUNT", "AZURE_SIGNING_PROFILE"):
            self.assertIn(f"secrets.{name}", wf)
        # Signing authenticates by federated OIDC: no client secret exists.
        self.assertNotIn("AZURE_CLIENT_SECRET", wf)
        self.assertNotIn("azure-client-secret", wf)
        # The build job that signs must be allowed to mint an OIDC token.
        build = wf[wf.index("\n  build:\n"):]
        build = build[:build.index("\n    steps:")]
        self.assertRegex(build, r"\n    permissions:\n(      .*\n)*      id-token: write\n")
        # azure/login (pinned) logs in with client and tenant ids only,
        # before the signing step, under the same condition.
        login = wf.index("- name: Azure login (federated OIDC)")
        sign = wf.index("Sign Windows binaries")
        self.assertLess(login, sign)
        step = wf[login:sign]
        self.assertIn("uses: azure/login@f5d393ae46f8fde4be8b75f32e3fc50e654ad0ca", step)
        self.assertIn("client-id: ${{ secrets.AZURE_CLIENT_ID }}", step)
        self.assertIn("tenant-id: ${{ secrets.AZURE_TENANT_ID }}", step)
        self.assertNotIn("client-secret", step)
        self.assertNotIn("creds:", step)
        cond = "if: matrix.os == 'windows' && steps.wincatalog.outputs.sign == 'true'"
        self.assertIn(cond, step)
        self.assertIn(cond, wf[sign:sign + 300])
        self.assertIn("Get-AuthenticodeSignature", wf)
        self.assertIn('run: uv run --no-project ci/windows_sign.py azure-catalog', wf)
        self.assertIn('files-catalog: ${{ runner.temp }}/obs-azure-${{ matrix.target }}.txt', wf)
        # Fail closed on every run, and sign in an environment the Azure
        # federated credential trusts (it does not trust refs/heads/master).
        self.assertNotIn("::warning::$Msg", wf)
        self.assertIn("\n    environment: bouf\n", build)
        self.assertIn("exclude-azure-cli-credential: false", wf)
        self.assertLess(sign, wf.index("Repack signed Windows package"))
        self.assertLess(wf.index("Repack signed Windows package"), wf.index("- name: Collect and verify"))


if __name__ == "__main__":
    unittest.main()
