#!/usr/bin/env -S uv run --script
"""The libcaption astyle format step is Python (obs-rust/obs-studio#962, GEN-005).

Ported off `deps/libcaption/format.sh`: the module selects the same suffixes the
shell's `find` did (`.cpp/.c/.h/.hpp/.re2c`) and runs the same `astyle` command
line on each source, with the working directory set to its own directory so the
`./`-relative paths resolve exactly as the shell's `cd "$(dirname "$0")"` did.
No real astyle runs here: a stub `astyle` on PATH records the working directory
and the argv it was handed.

The script has no CI caller (it is the developer helper that reformats the
libcaption sources), so this suite drives the module directly and pins that its
wiring -- the shell script gone, its GEN-005 exception gone -- is complete.
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
LIB = ROOT / "deps" / "libcaption"
MODULE = LIB / "format.py"
SHELL = LIB / "format.sh"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "deps/libcaption/format.sh"'

# The shell's astyle options, token for token.
EXPECTED_STYLE = [
    "--style=stroustrup",
    "--attach-extern-c",
    "--break-blocks",
    "--pad-header",
    "--pad-paren-out",
    "--unpad-paren",
    "--add-brackets",
    "--keep-one-line-blocks",
    "--keep-one-line-statements",
    "--convert-tabs",
    "--align-pointer=type",
    "--align-reference=type",
    "--suffix=none",
    "--lineend=linux",
    "--max-code-length=180",
]


def load_module():
    """Import format.py by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("libcaption_format", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FormatTest(unittest.TestCase):
    def test_astyle_options_are_unchanged(self):
        self.assertEqual(list(load_module().STYLE), EXPECTED_STYLE)

    def test_targets_are_the_sources_find_selected(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as work:
            root = Path(work)
            (root / "caption").mkdir()
            (root / "src").mkdir()
            for name in (
                "z.c",
                "b.cpp",
                "caption.h",
                "d.hpp",
                "eia608_from_utf8.re2c",
                "CMakeLists.txt",
                "Doxyfile.in",
                "README.md",
                ".clang-format",
            ):
                (root / "src" / name).write_text("")
            (root / "caption" / "caption.h").write_text("")
            self.assertEqual(
                module.targets(root),
                [
                    "./caption/caption.h",
                    "./src/b.cpp",
                    "./src/caption.h",
                    "./src/d.hpp",
                    "./src/eia608_from_utf8.re2c",
                    "./src/z.c",
                ],
            )

    def test_argv_is_astyle_then_the_source(self):
        self.assertEqual(
            load_module().argv("./src/caption.c"),
            ["astyle", *EXPECTED_STYLE, "./src/caption.c"],
        )

    def test_real_tree_yields_the_libcaption_sources(self):
        targets = load_module().targets(LIB)
        self.assertIn("./src/caption.c", targets)
        self.assertIn("./src/eia608_from_utf8.re2c", targets)
        self.assertIn("./caption/caption.h", targets)
        self.assertNotIn("./CMakeLists.txt", targets)
        self.assertNotIn("./format.sh", targets)

    def test_main_runs_astyle_once_per_source_in_the_module_directory(self):
        module = load_module()
        expected = module.targets(LIB)
        self.assertTrue(expected, "the libcaption tree has sources")
        with tempfile.TemporaryDirectory() as work:
            work_path = Path(work)
            capture = work_path / "calls.txt"
            stub = work_path / "astyle"
            stub.write_text(
                f'#!/bin/sh\nprintf \'%s|%s\\n\' "$PWD" "$*" >> {capture}\nexit 0\n'
            )
            stub.chmod(stub.stat().st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
            env = dict(os.environ)
            env["PATH"] = f"{work_path}{os.pathsep}{env.get('PATH', '')}"
            previous = os.getcwd()
            try:
                os.chdir("/tmp")
                result = subprocess.run(
                    [sys.executable, str(MODULE)],
                    env=env,
                    capture_output=True,
                    text=True,
                )
            finally:
                os.chdir(previous)
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = [
                line.split("|", 1) for line in capture.read_text().splitlines() if line
            ]
            # One astyle run per source, each from the module's own directory.
            self.assertEqual(len(calls), len(expected))
            self.assertEqual({cwd for cwd, _ in calls}, {str(LIB)})
            style = " ".join(EXPECTED_STYLE)
            self.assertEqual(
                {args for _, args in calls},
                {f"{style} {source}" for source in expected},
            )

    def test_wiring_is_complete(self):
        self.assertFalse(SHELL.exists(), "format.sh should be gone")
        self.assertTrue(MODULE.is_file(), "format.py should exist")
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(),
            "the GEN-005 exception for format.sh should be removed",
        )


if __name__ == "__main__":
    unittest.main()
