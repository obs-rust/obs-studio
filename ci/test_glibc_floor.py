#!/usr/bin/env -S uv run --script
"""The AppImage refuses to ship a binary newer than its glibc floor (#926).

    uv run python -m unittest ci/test_glibc_floor.py

build-aux/appimage/check-glibc-floor.py runs over the assembled AppDir and fails
when an ELF file requires a GLIBC_/GLIBCXX_/CXXABI_ symbol version newer than
the floor (glibc 2.28, GLIBCXX_3.4.25, CXXABI_1.3.11). Only the versions a
file *requires* count, so these tests stub `objdump -p` and check the parsing,
the floor comparison and the messages.
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHECK = ROOT / "build-aux" / "appimage" / "check-glibc-floor.py"
ELF = b"\x7fELF\x02\x01\x01\x00"

OBJDUMP_STUB = """
[[ -f "$STUBS/$(basename "${!#}")" ]] && cat "$STUBS/$(basename "${!#}")"
exit 0
"""


def references(*versions: str, defined: str = "") -> str:
    """objdump -p output: a Version References section, then unrelated text."""
    required = "".join(f"    0x1 0x00 0{i} {version}\n" for i, version in enumerate(versions, 1))
    return ("Dynamic Section:\n  NEEDED libc.so.6\n\n"
            "Version References:\n  required from libc.so.6:\n" + required + "\n"
            + defined)


class GlibcFloorTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.appdir = self.root / "AppDir"
        (self.appdir / "usr" / "lib").mkdir(parents=True)
        self.stubs = self.root / "objdump"
        self.stubs.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        objdump = self.bin / "objdump"
        objdump.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + OBJDUMP_STUB)
        objdump.chmod(0o755)

    def elf(self, name: str, *versions: str, defined: str = "") -> Path:
        path = self.appdir / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(ELF + b"payload")
        self.stubs.joinpath(path.name).write_text(references(*versions, defined=defined))
        return path

    def run_check(self, *args: str) -> subprocess.CompletedProcess:
        environment = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
        environment.update(PATH=f"{self.bin}{os.pathsep}{environment['PATH']}", STUBS=str(self.stubs))
        return subprocess.run([sys.executable, str(CHECK), *args], capture_output=True, text=True, env=environment)

    def test_a_directory_inside_the_floor_passes(self) -> None:
        self.elf("usr/lib/libobs.so.0", "GLIBC_2.28", "GLIBCXX_3.4.25", "CXXABI_1.3.11")
        result = self.run_check(str(self.appdir))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("all 1 ELF files run on glibc 2.28 with GLIBCXX_3.4.25", result.stdout)

    def test_newer_requirements_are_named_per_file(self) -> None:
        self.elf("usr/lib/libobs.so.0", "GLIBC_2.34", "GLIBCXX_3.4.29", "CXXABI_1.3.12")
        result = self.run_check(str(self.appdir))
        self.assertEqual(result.returncode, 1)
        self.assertIn("::error::usr/lib/libobs.so.0 needs GLIBC_2.34 GLIBCXX_3.4.29 CXXABI_1.3.12", result.stderr)
        self.assertIn("1 of 1 ELF files need a newer glibc", result.stderr)

    def test_the_custom_floor_moves_the_boundary(self) -> None:
        self.elf("bin/obs", "GLIBC_2.28")
        self.assertEqual(self.run_check(str(self.appdir)).returncode, 0)
        result = self.run_check(str(self.appdir), "2.17")
        self.assertEqual(result.returncode, 1)
        self.assertIn("needs GLIBC_2.28", result.stderr)

    def test_versions_a_file_defines_are_not_requirements(self) -> None:
        """The bundled glibc defines every GLIBC_2.x up to its own."""
        self.elf("usr/lib/libc.so.6", "GLIBC_2.17",
                 defined="  Name: libc.so.6\n  GLIBC_2.38 defined\n")
        result = self.run_check(str(self.appdir))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_non_elf_files_are_skipped_and_an_empty_appdir_fails(self) -> None:
        (self.appdir / "usr" / "share").mkdir()
        (self.appdir / "usr" / "share" / "notes.txt").write_text("these notes are not a binary")
        (self.appdir / "usr" / "bin").mkdir()
        (self.appdir / "usr" / "bin" / "obs").write_bytes(ELF + b"payload")
        self.stubs.joinpath("obs").write_text(references("GLIBC_2.17"))
        result = self.run_check(str(self.appdir))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("all 1 ELF files", result.stdout)

        empty = self.root / "empty"
        empty.mkdir()
        result = self.run_check(str(empty))
        self.assertEqual(result.returncode, 1)
        self.assertIn("no ELF files under", result.stderr)

    def test_assemble_appdir_runs_it_before_the_appimage_is_written(self) -> None:
        assemble = ROOT / "build-aux" / "appimage" / "assemble-appdir.sh"
        text = assemble.read_text(encoding="utf-8")
        self.assertIn("check-glibc-floor.py", text)


if __name__ == "__main__":
    unittest.main()
