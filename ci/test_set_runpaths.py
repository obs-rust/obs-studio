#!/usr/bin/env -S uv run --script
"""set-runpaths gives every bundled ELF a relative DT_RPATH (#962), in Python.

The rule lives in build-aux/appimage/set_runpaths.py since the port off the
shell script, and assemble_appdir.py calls it through `uv run`. A stub
`patchelf` on PATH records the argv it was given, so this pins the exact RPATH
the shell set: `$ORIGIN` for a file in usr/lib, `$ORIGIN/..` one level below it,
`$ORIGIN/../lib` for usr/bin, plus the optional trailing absolute directory.

    uv run python -m unittest ci/test_set_runpaths.py
"""

from __future__ import annotations

import importlib.util
import io
import os
import stat
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
APPIMAGE = ROOT / "build-aux" / "appimage"
MODULE = APPIMAGE / "set_runpaths.py"
SHELL_SCRIPT = APPIMAGE / "set-runpaths.sh"
ASSEMBLE_MODULE = APPIMAGE / "assemble_appdir.py"
ASSEMBLE_SHELL = APPIMAGE / "assemble-appdir.sh"

# Enough of an ELF header for the `head -c4` magic test the shell used.
ELF = b"\x7fELF\x02\x01\x01\x00" + b"\x00" * 8

# Records one line per call in $PATCHELF_LOG; PATCHELF_EXIT makes it fail.
STUB_PATCHELF = """#!/bin/sh
printf '%s\\n' "$*" >> "$PATCHELF_LOG"
exit "${PATCHELF_EXIT:-0}"
"""


def load_module(path: Path = MODULE, name: str = "set_runpaths"):
    """build-aux/appimage/set_runpaths.py by path: `uv run python -m unittest
    ci/test_set_runpaths.py` does not put build-aux/ on sys.path, and no file in
    this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None, f"cannot load {path}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class RunpathsTest(unittest.TestCase):
    """The RPATHs the shell set, over a small AppDir."""

    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        stub = self.bin / "patchelf"
        stub.write_text(STUB_PATCHELF, encoding="utf-8")
        stub.chmod(stub.stat().st_mode | stat.S_IEXEC)
        self.log = self.root / "patchelf.log"
        self.appdir = self.root / "AppDir"
        # usr/bin sorts before usr/lib, so the calls below come out in this
        # order; `find`'s own order was whatever the filesystem returned.
        self.obs = self.elf(self.appdir / "usr" / "bin" / "obs", executable=True)
        self.liba = self.elf(self.appdir / "usr" / "lib" / "liba.so")
        self.ld = self.elf(self.appdir / "usr" / "lib" / "ld-linux-x86-64.so.2")
        self.libb = self.elf(self.appdir / "usr" / "lib" / "sub" / "libb.so.1.2")
        # An executable file that is not an ELF, and a symlink to one: neither
        # was patched (`head -c4` and `find -type f` respectively).
        notes = self.appdir / "usr" / "share" / "notes.txt"
        notes.parent.mkdir(parents=True, exist_ok=True)
        notes.write_text("not an ELF\n", encoding="utf-8")
        notes.chmod(notes.stat().st_mode | stat.S_IEXEC)
        (self.appdir / "usr" / "lib" / "liblink.so").symlink_to("liba.so")

    def elf(self, path: Path, executable: bool = False) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(ELF)
        if executable:
            path.chmod(path.stat().st_mode | stat.S_IEXEC)
        return path

    def run_main(self, *args: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        env = {
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "PATCHELF_LOG": str(self.log),
        }
        with patch.dict(os.environ, env):
            with redirect_stdout(out), redirect_stderr(err):
                code = self.module.main(list(args))
        return code, out.getvalue(), err.getvalue()

    def calls(self) -> list[str]:
        if not self.log.exists():
            return []
        return self.log.read_text(encoding="utf-8").splitlines()

    def test_relative_rpath_per_file_and_the_rest_is_skipped(self):
        code, out, err = self.run_main(str(self.appdir))
        self.assertEqual(code, 0, err)
        self.assertEqual(err, "")
        self.assertEqual(
            self.calls(),
            [
                f"--force-rpath --set-rpath $ORIGIN/../lib {self.obs}",
                f"--force-rpath --set-rpath $ORIGIN {self.liba}",
                f"--force-rpath --set-rpath $ORIGIN/.. {self.libb}",
            ],
        )
        self.assertEqual(out.strip(), "RPATH set on 3 ELF files")

    def test_extra_directory_is_appended_to_every_rpath(self):
        code, out, err = self.run_main(str(self.appdir), "/opt/glibc/lib")
        self.assertEqual(code, 0, err)
        self.assertEqual(
            self.calls(),
            [
                f"--force-rpath --set-rpath $ORIGIN/../lib:/opt/glibc/lib {self.obs}",
                f"--force-rpath --set-rpath $ORIGIN:/opt/glibc/lib {self.liba}",
                f"--force-rpath --set-rpath $ORIGIN/..:/opt/glibc/lib {self.libb}",
            ],
        )

    def test_a_failing_patchelf_is_reported_and_not_counted(self):
        with patch.dict(os.environ, {"PATCHELF_EXIT": "1"}):
            code, out, err = self.run_main(str(self.appdir))
        self.assertEqual(code, 0)
        for path in (self.obs, self.liba, self.libb):
            self.assertIn(f"patchelf failed: {path}", err)
        self.assertEqual(out.strip(), "RPATH set on 0 ELF files")

    def test_no_appdir_is_a_usage_error(self):
        code, out, err = self.run_main()
        self.assertEqual(code, 2)
        self.assertIn("usage: set_runpaths.py <AppDir>", err)
        self.assertEqual(self.calls(), [])


class WiringTest(unittest.TestCase):
    """The shell scripts are gone; assemble_appdir.py runs the Python through uv."""

    def test_assemble_appdir_runs_the_python_module_through_uv(self):
        assemble = load_module(ASSEMBLE_MODULE, "assemble_appdir")
        appdir = Path("/tmp/AppDir")
        self.assertEqual(
            assemble.set_runpaths_command(APPIMAGE, appdir, ""),
            ["uv", "run", "--no-project", str(APPIMAGE / "set_runpaths.py"), str(appdir)],
        )
        # When glibc is bundled the loader's lib directory is appended.
        self.assertEqual(
            assemble.set_runpaths_command(APPIMAGE, appdir, "/tmp/.obs-rust-appimage-abc123"),
            [
                "uv",
                "run",
                "--no-project",
                str(APPIMAGE / "set_runpaths.py"),
                str(appdir),
                "/tmp/.obs-rust-appimage-abc123/lib",
            ],
        )
        self.assertFalse(SHELL_SCRIPT.exists(), "the shell script is the ported one")
        self.assertFalse(ASSEMBLE_SHELL.exists(), "assemble-appdir.sh is the ported one")

    def test_no_exception_is_left_for_the_ported_script(self):
        ci_toml = (ROOT / "ci.toml").read_text(encoding="utf-8")
        self.assertFalse(
            "build-aux/appimage/set-runpaths.sh" in ci_toml,
            "the GEN-005 exception for set-runpaths.sh is still in ci.toml",
        )


if __name__ == "__main__":
    unittest.main()
