#!/usr/bin/env -S uv run --script
"""format.py reproduces libcaption's astyle invocation (#962), in Python.

The rule lives in deps/libcaption/format.py since the port off the 3-line shell
script: the shell `cd`s to its own directory and runs `astyle` with a fixed
argument list over every `*.c`/`*.cpp`/`*.h`/`*.hpp`/`*.re2c` file below it.
The script has no caller (it is a developer formatter), so the port has no
interpreter constraint to satisfy. This drives the port with a stub `astyle` on
PATH that records its argv and working directory, over a small tree that mixes
the five suffixes with files the shell's `find` skipped.

    uv run python -m unittest ci/test_libcaption_format.py
"""

from __future__ import annotations

import importlib.util
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LIB = ROOT / "deps" / "libcaption"
MODULE = LIB / "format.py"
SHELL_SCRIPT = LIB / "format.sh"

# The astyle argv the shell built, token for token (`astyle` first, then the
# flags on the single `-exec ... {} \;` line, then the file).
ASTYLE_ARGS = [
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

# What `find . \( -name '*.cpp' -o ... \)` selected, as the shell passed it to
# astyle (`./`-prefixed, recursive, sorted here for a stable assertion).
MATCHED = [
    "./a.c",
    "./b.cpp",
    "./c.h",
    "./d.hpp",
    "./e.re2c",
    "./sub/f.c",
]


def load_module():
    """deps/libcaption/format.py by path: `uv run python -m unittest
    ci/test_libcaption_format.py` does not put deps/libcaption/ on sys.path, and
    no file in this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("libcaption_format", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class FormatTest(unittest.TestCase):
    """The stub astyle receives the shell's own argv, from the script's dir."""

    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        for rel in MATCHED + ["./notes.txt", "./README.md", "./.clang-format"]:
            path = self.root / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("int x;\n", encoding="utf-8")
        self.log = self.root / "astyle.log"
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        stub = bin_dir / "astyle"
        stub.write_text('#!/bin/sh\nprintf \'%s\\t%s\\n\' "$PWD" "$*" >> "$ASTYLE_LOG"\n', encoding="utf-8")
        stub.chmod(stub.stat().st_mode | stat.S_IEXEC)
        self.bin_dir = bin_dir

    def run_port(self) -> int:
        old_path = os.environ.get("PATH", "")
        old_log = os.environ.get("ASTYLE_LOG")
        os.environ["PATH"] = f"{self.bin_dir}{os.pathsep}{old_path}"
        os.environ["ASTYLE_LOG"] = str(self.log)
        try:
            return self.module.run(self.root)
        finally:
            os.environ["PATH"] = old_path
            if old_log is None:
                os.environ.pop("ASTYLE_LOG", None)
            else:
                os.environ["ASTYLE_LOG"] = old_log

    def invocations(self) -> list[tuple[str, str]]:
        if not self.log.exists():
            return []
        calls: list[tuple[str, str]] = []
        for line in self.log.read_text(encoding="utf-8").splitlines():
            cwd, argv = line.split("\t", 1)
            calls.append((cwd, argv))
        return calls

    def test_only_the_five_suffixes_are_collected(self):
        self.assertEqual(sorted(self.module.collect_sources(self.root)), MATCHED)

    def test_astyle_receives_the_shell_argv_for_every_matched_file(self):
        self.assertEqual(self.run_port(), 0)
        calls = self.invocations()
        self.assertEqual(len(calls), len(MATCHED))
        for cwd, argv in calls:
            self.assertEqual(cwd, str(self.root), "astyle did not run in the script's directory")
            # The stub logs "$*": the arguments only, the module invokes `astyle`.
            source = argv.rsplit(" ", 1)[1]
            self.assertIn(source, MATCHED)
            self.assertEqual(argv, " ".join([*ASTYLE_ARGS, source]))

    def test_a_failing_astyle_makes_the_port_fail(self):
        stub = self.bin_dir / "astyle"
        stub.write_text('#!/bin/sh\nexit 3\n', encoding="utf-8")
        stub.chmod(stub.stat().st_mode | stat.S_IEXEC)
        self.assertEqual(self.run_port(), 3)


class WiringTest(unittest.TestCase):
    """The shell script is gone and its GEN-005 exception with it."""

    def test_the_shell_script_is_gone(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "format.sh is the ported script")

    def test_the_python_opts_into_uv(self):
        first = MODULE.read_text(encoding="utf-8").splitlines()[0]
        self.assertEqual(first, "#!/usr/bin/env -S uv run --script")

    def test_no_exception_is_left_for_the_ported_script(self):
        ci_toml = (ROOT / "ci.toml").read_text(encoding="utf-8")
        self.assertNotIn(
            "deps/libcaption/format.sh",
            ci_toml,
            "the GEN-005 exception for format.sh is still in ci.toml",
        )


if __name__ == "__main__":
    unittest.main()
