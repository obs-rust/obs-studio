#!/usr/bin/env -S uv run --script
"""The Sphinx docs build is Python (obs-rust/obs-studio#962, GEN-005).

Ported off `docs/sphinx/make.bat`: the module builds the same `sphinx-build`
command line the batch file wrote -- `-M <target> . _build` plus `SPHINXOPTS`
-- and runs it with the working directory set to the docs directory, so the
`SOURCEDIR=.`/`BUILDDIR=_build` pair resolves exactly as it did. No real
sphinx-build runs here: a stub `sphinx-build` on PATH records the argv and the
working directory it was handed.

The batch file has no caller in the tree, so this suite drives the module
directly and pins that its wiring -- the batch file gone, its GEN-005
exception gone -- is complete.
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
DOCS = ROOT / "docs" / "sphinx"
MODULE = DOCS / "make.py"
BATCH = DOCS / "make.bat"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "docs/sphinx/make.bat"'

# The batch file's one sphinx-build line, token for token.
EXPECTED_ARGV = ["sphinx-build", "-M", "build", ".", "_build"]


def load_module():
    """Import make.py by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("sphinx_make", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_module(args, env):
    """Run the module as a script from an unrelated cwd."""
    previous = os.getcwd()
    try:
        os.chdir(tempfile.gettempdir())
        return subprocess.run(
            [sys.executable, str(MODULE), *args],
            env=env,
            capture_output=True,
            text=True,
        )
    finally:
        os.chdir(previous)


def stub_path(work_path):
    """A temp dir holding a stub sphinx-build that records argv and cwd."""
    capture = work_path / "argv.txt"
    stub = work_path / "sphinx-build"
    stub.write_text(
        "#!/bin/sh\n"
        f"printf '%s\\n' \"$PWD\" > {capture}\n"
        f"printf '%s\\n' \"$@\" >> {capture}\n"
        "exit 0\n"
    )
    stub.chmod(stub.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    return capture


class SphinxMakeTest(unittest.TestCase):
    def test_sphinx_build_argv_is_unchanged(self):
        self.assertEqual(load_module().argv("build"), EXPECTED_ARGV)

    def test_sphinxopts_is_appended_as_tokens(self):
        previous = os.environ.get("SPHINXOPTS")
        os.environ["SPHINXOPTS"] = "-W --keep-going"
        try:
            argv = load_module().argv("build")
        finally:
            if previous is None:
                os.environ.pop("SPHINXOPTS", None)
            else:
                os.environ["SPHINXOPTS"] = previous
        self.assertEqual(argv, [*EXPECTED_ARGV, "-W", "--keep-going"])

    def test_main_runs_sphinx_build_in_the_docs_directory(self):
        with tempfile.TemporaryDirectory() as work:
            work_path = Path(work)
            capture = stub_path(work_path)
            env = dict(os.environ)
            env["PATH"] = str(work_path)
            rc = run_module(["build"], env)
            self.assertEqual(rc.returncode, 0, rc.stderr)
            recorded = capture.read_text().splitlines()
            self.assertEqual(recorded[0], str(DOCS))
            # The stub records "$@" (no argv[0]), so drop the program name.
            self.assertEqual(recorded[1:], EXPECTED_ARGV[1:])

    def test_a_missing_target_defaults_to_help(self):
        with tempfile.TemporaryDirectory() as work:
            work_path = Path(work)
            capture = stub_path(work_path)
            env = dict(os.environ)
            env["PATH"] = str(work_path)
            rc = run_module([], env)
            self.assertEqual(rc.returncode, 0, rc.stderr)
            self.assertEqual(
                capture.read_text().splitlines()[1:], ["-M", "help", ".", "_build"]
            )

    def test_a_missing_tool_hints_and_fails(self):
        with tempfile.TemporaryDirectory() as work:
            env = dict(os.environ)
            env["PATH"] = work  # no sphinx-build here
            rc = run_module(["build"], env)
            self.assertEqual(rc.returncode, 1)
            output = rc.stdout + rc.stderr
            self.assertIn("'sphinx-build' command was not found", output)

    def test_wiring_is_complete(self):
        self.assertFalse(BATCH.exists(), "make.bat should be gone")
        self.assertTrue(MODULE.is_file(), "make.py should exist")
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(),
            "the GEN-005 exception for make.bat should be removed",
        )


if __name__ == "__main__":
    unittest.main()
