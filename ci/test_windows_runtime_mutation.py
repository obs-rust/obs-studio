#!/usr/bin/env -S uv run --script
"""Execute the native workflow's ZIP mutation and package rejection (#627)."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import textwrap
import unittest
import zipfile

from test_windows_zip_check import pe

ROOT = Path(__file__).resolve().parent.parent


class WindowsRuntimeMutation(unittest.TestCase):
    def test_real_powershell_mutation_rejects_runtime_without_modifying_original(self):
        if shutil.which("pwsh"):
            engine = ["pwsh"]
        elif shutil.which("nix"):
            engine = ["nix", "shell", "nixpkgs#powershell", "--command", "pwsh"]
        else:
            self.skipTest("PowerShell is unavailable")
        workflow = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        source = (
            "$BrokenZip ="
            + workflow.split("          $BrokenZip =", 1)[1].split(
                "          $PackageRoot =", 1
            )[0]
        )
        source = textwrap.dedent(source)
        checker = "'" + str(ROOT / "ci/windows_zip_check.py").replace("'", "''") + "'"
        source = source.replace("ci/windows_zip_check.py", checker)
        for target in ("x64", "arm64"):
            with (
                self.subTest(target=target),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory)
                original = root / "original.zip"
                with zipfile.ZipFile(original, "w") as archive:
                    archive.writestr("bin/64bit/obs64.exe", pe(["vcruntime140.dll"]))
                    archive.writestr("bin/64bit/vcruntime140.dll", b"runtime fixture")
                    archive.writestr("core/camera/win-dshow.dll", pe([]))
                before = original.read_bytes()
                script = root / "mutation.ps1"
                script.write_text(
                    "$ErrorActionPreference = 'Stop'\n$Zip = Get-Item './original.zip'\n"
                    + source
                )
                result = subprocess.run(
                    [*engine, "-NoLogo", "-NoProfile", "-File", str(script)],
                    cwd=root,
                    env=dict(
                        os.environ,
                        RUNNER_TEMP=directory,
                        TARGET=target,
                        VERSION="fixture",
                    ),
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn("imports vcruntime140.dll", result.stdout)
                self.assertEqual(original.read_bytes(), before)
                with zipfile.ZipFile(
                    root / f"obs-{target}-missing-runtime.zip"
                ) as broken:
                    self.assertNotIn("bin/64bit/vcruntime140.dll", broken.namelist())
                    self.assertIn("core/camera/win-dshow.dll", broken.namelist())


if __name__ == "__main__":
    unittest.main()
