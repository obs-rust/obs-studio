#!/usr/bin/env -S uv run --script
"""`ci/warning_findings.py` replays selected TUs and groups the warnings by directory (#2689).

    uv run python -m unittest ci/test_warning_findings.py

A warning class is only reviewable as a whole: the per-directory slicing
convention costs one full `Pull` run (~35 min) per directory and invalidates
the compile cache on every slice. The helper reads a `compile_commands.json`
from the Tier 2 build tree, keeps only the TUs a caller selected, drops the
`-Wno-*` flags named on the command line, swaps `-c` for `-fsyntax-only` and
prints the findings grouped by directory.

The tests drive the module against a stub compiler, so they need no build
tree, no CMake and no network.
"""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

MODULE = Path(__file__).with_name("warning_findings.py")

# A stand-in for `cc`: records the argv it was called with and emits one
# conversion warning per translation unit whose name does not say "clean".
STUB = '''\
import json
import sys

args = sys.argv[1:]
record = {"argv": args}
with open("__RECORD__", "a", encoding="utf-8") as handle:
    handle.write(json.dumps(record) + "\\n")
source = next((arg for arg in args if arg.endswith(".c")), "")
if source and "clean" not in source:
    print(
        f"{source}:10:5: warning: conversion from 'double' to 'float' "
        "may change value [-Wfloat-conversion]",
        file=sys.stderr,
    )
'''


def load_module():
    """The module under test, imported by path (it is a script, not a package)."""
    spec = importlib.util.spec_from_file_location("warning_findings", MODULE)
    assert spec is not None and spec.loader is not None, MODULE
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module  # dataclasses resolve their module by name
    spec.loader.exec_module(module)
    return module


class WarningFindingsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.record = self.root / "argv.jsonl"
        self.stub = self.root / "stub_cc.py"
        self.stub.write_text(STUB.replace("__RECORD__", str(self.record)), encoding="utf-8")
        self.module = load_module()

    def tearDown(self) -> None:
        self.tmp.cleanup()

    def compdb(self, *entries: tuple[str, list[str]]) -> Path:
        """A compile_commands.json whose compiler is the stub."""
        commands = []
        for name, flags in entries:
            source = self.root / name
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("int x;\n", encoding="utf-8")
            commands.append(
                {
                    "directory": str(self.root),
                    "file": str(source),
                    "arguments": [sys.executable, str(self.stub), *flags, str(source)],
                }
            )
        path = self.root / "compile_commands.json"
        path.write_text(json.dumps(commands), encoding="utf-8")
        return path

    def run_cli(self, *args: str) -> tuple[int, dict]:
        """Run main(); return its exit code and the parsed JSON report."""
        out = self.root / "report.json"
        code = self.module.main([*args, "--json", "--out", str(out)])
        return code, json.loads(out.read_text(encoding="utf-8"))

    def recorded(self) -> list[dict]:
        return [json.loads(line) for line in self.record.read_text(encoding="utf-8").splitlines()]

    def test_reports_one_finding_per_warning_source(self) -> None:
        compdb = self.compdb(("libobs/util/a.c", []), ("plugins/obs-x/b.c", []))
        code, report = self.run_cli("--compdb", str(compdb), "--root", str(self.root))
        self.assertEqual(code, 1, "a finding must be a non-zero exit (the RED state)")
        self.assertEqual(report["total"], 2)

    def test_groups_the_findings_by_directory(self) -> None:
        compdb = self.compdb(
            ("libobs/util/a.c", []), ("libobs/util/b.c", []), ("plugins/obs-x/c.c", [])
        )
        _, report = self.run_cli("--compdb", str(compdb), "--root", str(self.root))
        rows = {row["directory"]: row["warnings"] for row in report["directories"]}
        self.assertEqual(rows, {"libobs/util": 2, "plugins/obs-x": 1})
        self.assertEqual(report["directories"][0]["directory"], "libobs/util", "sorted by count")

    def test_clean_replay_exits_zero(self) -> None:
        compdb = self.compdb(("libobs/util/clean_a.c", []))
        code, report = self.run_cli("--compdb", str(compdb), "--root", str(self.root))
        self.assertEqual((code, report["total"]), (0, 0))

    def test_drops_the_named_no_warning_flag_and_compiles_only(self) -> None:
        compdb = self.compdb(("libobs/util/a.c", ["-Wno-float-conversion", "-O2"]))
        self.run_cli("--compdb", str(compdb), "--root", str(self.root), "--drop", "-Wno-float-conversion")
        argv = self.recorded()[0]["argv"]
        self.assertNotIn("-Wno-float-conversion", argv, "the dropped -Wno-* must reach no compiler")
        self.assertIn("-fsyntax-only", argv, "the replay must not write an object")
        self.assertIn("-O2", argv, "every other flag is preserved")
        self.assertNotIn("-c", argv)

    def test_drops_the_object_output_and_its_argument(self) -> None:
        compdb = self.compdb(("libobs/util/a.c", ["-o", "a.c.o"]))
        self.run_cli("--compdb", str(compdb), "--root", str(self.root))
        argv = self.recorded()[0]["argv"]
        self.assertNotIn("-o", argv)
        self.assertNotIn("a.c.o", argv)

    def test_strips_a_compiler_cache_wrapper(self) -> None:
        compdb = self.compdb(("libobs/util/a.c", []))
        entry = json.loads(compdb.read_text(encoding="utf-8"))[0]
        entry["command"] = "zccache " + " ".join(entry.pop("arguments"))
        compdb.write_text(json.dumps([entry]), encoding="utf-8")
        code, report = self.run_cli("--compdb", str(compdb), "--root", str(self.root))
        self.assertEqual((code, report["total"]), (1, 1), "the wrapper must be stripped and run")
        self.assertNotIn("zccache", self.recorded()[0]["argv"])

    def test_selects_only_the_matching_translation_units(self) -> None:
        compdb = self.compdb(("libobs/util/a.c", []), ("plugins/obs-x/b.c", []))
        self.run_cli(
            "--compdb", str(compdb), "--root", str(self.root), "--select", "libobs/util/*"
        )
        self.assertEqual(len(self.recorded()), 1)
        self.assertIn(str(self.root / "libobs/util/a.c"), self.recorded()[0]["argv"])

    def test_replay_argv_is_pure(self) -> None:
        argv = ["cc", "-c", "a.c", "-o", "a.o", "-Wno-float-conversion", "-Werror", "-O2"]
        replay = self.module.replay_argv(argv, frozenset({"-Wno-float-conversion"}))
        self.assertEqual(replay, ["cc", "a.c", "-O2", "-fsyntax-only"])


if __name__ == "__main__":
    unittest.main()
