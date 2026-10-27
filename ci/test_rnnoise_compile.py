#!/usr/bin/env -S uv run --script
"""The rnnoise training build is Python (obs-rust/obs-studio#962, GEN-005).

Ported off `plugins/obs-filters/rnnoise/src/compile.sh`: the module builds the
same single `gcc` command line and runs it with the working directory set to
its own directory, so `-I../include` and the relative sources resolve exactly
as the shell did. No real gcc runs here: a stub `gcc` on PATH records the argv
and the working directory it was handed.

The script has no CI caller (it is the developer helper that regenerates the
training data generator), so this suite drives the module directly and pins
that its wiring -- the shell script gone, its GEN-005 exception gone -- is
complete.
"""

from __future__ import annotations

import importlib.util
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "plugins" / "obs-filters" / "rnnoise" / "src"
MODULE = SRC / "compile.py"
SHELL = SRC / "compile.sh"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "plugins/obs-filters/rnnoise/src/compile.sh"'

# The shell's one gcc line, token for token.
EXPECTED_ARGV = [
    "gcc",
    "-DTRAINING=1",
    "-Wall",
    "-W",
    "-O3",
    "-g",
    "-I../include",
    "denoise.c",
    "kiss_fft.c",
    "pitch.c",
    "celt_lpc.c",
    "rnn.c",
    "rnn_data.c",
    "-o",
    "denoise_training",
    "-lm",
]


def load_module():
    """Import compile.py by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("rnnoise_compile", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class CompileTest(unittest.TestCase):
    def test_gcc_argv_is_unchanged(self):
        self.assertEqual(load_module().argv(), EXPECTED_ARGV)

    def test_every_source_exists_next_to_the_module(self):
        module = load_module()
        sources = [token for token in module.argv() if token.endswith(".c")]
        self.assertTrue(sources)
        for source in sources:
            self.assertTrue((SRC / source).is_file(), source)

    def test_main_runs_gcc_in_the_source_directory(self):
        with tempfile.TemporaryDirectory() as work:
            work_path = Path(work)
            capture = work_path / "argv.txt"
            stub = work_path / "gcc"
            stub.write_text(
                "#!/bin/sh\n"
                f"printf '%s\\n' \"$PWD\" > {capture}\n"
                f"printf '%s\\n' \"$@\" >> {capture}\n"
                "exit 0\n"
            )
            stub.chmod(stub.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
            env = dict(os.environ)
            env["PATH"] = f"{work_path}{os.pathsep}{env.get('PATH', '')}"
            module = load_module()
            previous = os.getcwd()
            try:
                os.chdir("/tmp")
                rc = subprocess.run(
                    [sys.executable, str(MODULE)],
                    env=env,
                    capture_output=True,
                    text=True,
                )
            finally:
                os.chdir(previous)
            self.assertEqual(rc.returncode, 0, rc.stderr)
            recorded = capture.read_text().splitlines()
            self.assertEqual(recorded[0], str(SRC))
            # The stub records "$@" (no argv[0]), so drop the program name.
            self.assertEqual(recorded[1:], EXPECTED_ARGV[1:])
            self.assertEqual(module.argv()[0], "gcc")

    def test_wiring_is_complete(self):
        self.assertFalse(SHELL.exists(), "compile.sh should be gone")
        self.assertTrue(MODULE.is_file(), "compile.py should exist")
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(),
            "the GEN-005 exception for compile.sh should be removed",
        )


if __name__ == "__main__":
    unittest.main()
