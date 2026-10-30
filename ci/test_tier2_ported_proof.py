#!/usr/bin/env -S uv run --script
"""Tests for the ported-object proof in rust/tools/linux-validate/validate.sh.

    uv run python -m unittest ci/test_tier2_ported_proof.py

The proof asks, for each ported object, whether libobs and
libobs-c-reference compile it. Before #3046 each answer ran
`ninja -t inputs <target>` (which loads the whole build graph) up to twice
and walked the build tree with `find`. With 176 objects that was about 700
graph loads, 59 s of the merge queue's Tier 2 build (run 41034283809). This
runs the script's own `compiles` over a fake build tree and a counting fake
`ninja`. It checks that the answers are unchanged and that each target's
graph is loaded once.
"""

from __future__ import annotations

import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
START = "  # Query the current Ninja build graph"
END = "  for obj in $ported_objs; do"


def proof_functions() -> str:
    text = VALIDATE.read_text(encoding="utf-8")
    start = text.index(START)
    end = text.index(END, start)
    return text[start:end]


class PortedProofTest(unittest.TestCase):
    def setUp(self) -> None:
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.tmp = Path(tmp.name)
        self.src = self.tmp / "src"
        self.build = self.tmp / "build"
        (self.src / "libobs" / "build").mkdir(parents=True)
        (self.src / "libobs" / "build" / "libobs.grp+.h.c").write_text(
            '#include "libobs/grouped.c"\n', encoding="utf-8"
        )
        unity = self.build / "libobs" / "CMakeFiles" / "libobs.dir" / "Unity"
        unity.mkdir(parents=True)
        (unity / "unity_0_c.c").write_text(
            '#include "/src/libobs/unified.c"\n', encoding="utf-8"
        )
        bindir = self.tmp / "bin"
        bindir.mkdir()
        self.calls = self.tmp / "ninja-calls"
        ninja = bindir / "ninja"
        ninja.write_text(
            textwrap.dedent(
                f"""\
                #!/bin/sh
                echo "$@" >>"{self.calls}"
                target=$5
                echo "libobs/CMakeFiles/$target.dir/util/direct.c.o"
                echo "libobs/CMakeFiles/$target.dir/build/libobs.grp+.h.c.o"
                """
            ),
            encoding="utf-8",
        )
        ninja.chmod(0o755)
        self.path = f"{bindir}{os.pathsep}{os.environ['PATH']}"

    def run_proof(self, queries: list[tuple[str, str, str]]) -> list[str]:
        lines = [
            f'if compiles {t} {s} {o}; then echo yes; else echo no; fi'
            for t, s, o in queries
        ]
        script = "\n".join(
            [
                "set -euo pipefail",
                f'SRC="{self.src}"',
                f'build="{self.build}"',
                proof_functions(),
                *lines,
            ]
        )
        result = subprocess.run(
            ["bash", "-c", script],
            capture_output=True,
            text=True,
            env={**os.environ, "PATH": self.path},
            check=True,
        )
        return result.stdout.split()

    def test_answers(self) -> None:
        got = self.run_proof(
            [
                ("libobs", "libobs/util/direct.c", "util/direct.c.o"),
                ("libobs", "libobs/grouped.c", "grouped.c.o"),
                ("libobs", "libobs/unified.c", "unified.c.o"),
                ("libobs", "libobs/absent.c", "absent.c.o"),
                ("libobs-c-reference", "libobs/unified.c", "unified.c.o"),
            ]
        )
        self.assertEqual(got, ["yes", "yes", "yes", "no", "no"])

    def test_loads_each_graph_once(self) -> None:
        queries = []
        for i in range(40):
            for target in ("libobs", "libobs-c-reference"):
                queries.append((target, f"libobs/p{i}.c", f"p{i}.c.o"))
        got = self.run_proof(queries)
        self.assertEqual(got, ["no"] * len(queries))
        calls = self.calls.read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(calls), 2, f"ninja ran {len(calls)} times")


if __name__ == "__main__":
    unittest.main()
