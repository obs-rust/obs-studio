#!/usr/bin/env -S uv run --script
"""Execute the release collector against Windows package fixtures (#627)."""

from pathlib import Path
import os
import subprocess
import tempfile
import textwrap
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent


class WindowsSymbolsCollection(unittest.TestCase):
    def collect(self, root, target):
        workflow = (ROOT / ".github/workflows/release-rust.yaml").read_text()
        block = workflow.split("      - name: Collect and verify", 1)[1]
        source = block.split("        run: |\n", 1)[1].split(
            "          # Rust must actually", 1
        )[0]
        return subprocess.run(
            [
                "bash",
                "-c",
                textwrap.dedent(source).replace("${{ matrix.target }}", target),
            ],
            cwd=root,
            env=dict(
                os.environ, OS="windows", TARGET=target, VERSION="0.1.0-beta.test"
            ),
            capture_output=True,
            text=True,
            timeout=10,
        )

    def fixture(self, root, target, symbols=True):
        build = root / f"build_{target}"
        build.mkdir()
        stem = build / f"obs-studio-test-windows-{target}"
        with zipfile.ZipFile(f"{stem}.zip", "w") as archive:
            archive.writestr("bin/64bit/obs64.exe", b"application")
        if symbols:
            with zipfile.ZipFile(f"{stem}-symbols.zip", "w") as archive:
                archive.writestr("bin/64bit/obs64.pdb", b"matching debug symbols")

    def test_both_architectures_collect_matching_symbols_without_bloating_main_zip(
        self,
    ):
        for target in ("x64", "arm64"):
            with (
                self.subTest(target=target),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory)
                self.fixture(root, target)
                result = self.collect(root, target)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                stem = (
                    root / "pkg" / f"OBS-Studio-Rust-0.1.0-beta.test-windows-{target}"
                )
                self.assertTrue(Path(f"{stem}-symbols.zip").is_file(), result.stdout)
                with zipfile.ZipFile(f"{stem}-symbols.zip") as symbols:
                    self.assertEqual(
                        symbols.read("bin/64bit/obs64.pdb"), b"matching debug symbols"
                    )
                with zipfile.ZipFile(f"{stem}.zip") as main:
                    self.assertFalse(
                        any(name.endswith(".pdb") for name in main.namelist())
                    )

    def test_missing_symbols_fails_collection(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.fixture(root, "x64", symbols=False)
            result = self.collect(root, "x64")
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
