#!/usr/bin/env -S uv run --script
"""Tests for rust/tools/linux-validate/ported_proof.py.

    uv run python -m unittest ci/test_tier2_ported_proof.py

validate.sh proves every ported C object is compiled into libobs-c-reference
and kept out of hybrid libobs. The shell loop spawned a grep per object per
unity group: on the Dell (#4954) it took 176 s of a 385 s warm build with
nothing to rebuild. ported_proof.py reads each target's Ninja inputs, unity
groups and Unity batches once and answers the same predicate in memory.

These cases drive it over a fake build tree, with a stand-in `ninja` on PATH
that prints canned `-t inputs` output.
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
TOOLS = ROOT / "rust" / "tools" / "linux-validate"
SCRIPT = TOOLS / "ported_proof.py"
VALIDATE = TOOLS / "validate.sh"


def load():
    spec = importlib.util.spec_from_file_location("ported_proof", SCRIPT)
    assert spec and spec.loader, f"no {SCRIPT}"
    module = importlib.util.module_from_spec(spec)
    sys.modules["ported_proof"] = module
    spec.loader.exec_module(module)
    return module


class Tree:
    """A fake source tree and build dir with a canned `ninja -t inputs`."""

    def __init__(self, inputs: dict[str, list[str]]):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.src = base / "src"
        self.build = base / "build"
        self.bin = base / "bin"
        for d in (self.src / "libobs" / "build", self.build, self.bin):
            d.mkdir(parents=True)
        for target, lines in inputs.items():
            (self.bin / f"{target}.inputs").write_text("".join(f"{line}\n" for line in lines))
        ninja = self.bin / "ninja"
        # ninja -C <build> -t inputs <target>
        ninja.write_text(
            "#!/bin/sh\n"
            f'f="{self.bin}/$5.inputs"\n'
            '[ -f "$f" ] && cat "$f" || exit 1\n'
        )
        ninja.chmod(ninja.stat().st_mode | stat.S_IXUSR)
        self.env = dict(os.environ, PATH=f"{self.bin}{os.pathsep}{os.environ['PATH']}")

    def group(self, name: str, includes: list[str]) -> None:
        (self.src / "libobs" / name).write_text("".join(f'#include "{i}"\n' for i in includes))

    def unity(self, target: str, includes: list[str]) -> None:
        d = self.build / "libobs" / "CMakeFiles" / f"{target}.dir" / "Unity"
        d.mkdir(parents=True)
        (d / "unity_0_c.c").write_text("".join(f'#include "{i}"\n' for i in includes))

    def run(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--build", str(self.build), "--src", str(self.src), *args],
            env=self.env, capture_output=True, text=True, check=False,
        )


C_REF = "libobs/CMakeFiles/libobs-c-reference.dir"
HYBRID = "libobs/CMakeFiles/libobs.dir"


class PortedProofTest(unittest.TestCase):
    def tree(self, inputs: dict[str, list[str]]) -> Tree:
        tree = Tree(inputs)
        self.addCleanup(tree.tmp.cleanup)
        return tree

    def test_object_in_reference_only_passes(self):
        t = self.tree({"libobs": [f"{HYBRID}/obs.c.o"], "libobs-c-reference": [f"{C_REF}/util/bmem.c.o"]})
        r = t.run("ported", "util/bmem.c.o")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)

    def test_object_compiled_into_hybrid_libobs_fails(self):
        t = self.tree({"libobs": [f"{HYBRID}/util/bmem.c.o"], "libobs-c-reference": [f"{C_REF}/util/bmem.c.o"]})
        r = t.run("ported", "util/bmem.c.o")
        self.assertEqual(r.returncode, 1)
        self.assertIn("FAIL: util/bmem.c.o was compiled into hybrid libobs", r.stdout)

    def test_object_missing_from_reference_fails(self):
        t = self.tree({"libobs": [], "libobs-c-reference": [f"{C_REF}/obs.c.o"]})
        r = t.run("ported", "util/bmem.c.o")
        self.assertEqual(r.returncode, 1)
        self.assertIn("FAIL: util/bmem.c.o was not compiled into libobs-c-reference", r.stdout)

    def test_unity_group_including_the_source_counts(self):
        # libobs/build/<group>+.h.c includes the source by its path below the source tree.
        group = "build/util-core+.h.c"
        t = self.tree({"libobs": [], "libobs-c-reference": [f"{C_REF}/{group}.o"]})
        t.group(group, ["libobs/util/bmem.c"])
        self.assertEqual(t.run("ported", "util/bmem.c.o").returncode, 0)
        t2 = self.tree({"libobs": [f"{HYBRID}/{group}.o"], "libobs-c-reference": [f"{C_REF}/{group}.o"]})
        t2.group(group, ["libobs/util/bmem.c"])
        r = t2.run("ported", "util/bmem.c.o")
        self.assertIn("compiled into hybrid libobs", r.stdout)

    def test_group_needs_the_exact_quoted_path(self):
        group = "build/util-core+.h.c"
        t = self.tree({"libobs": [], "libobs-c-reference": [f"{C_REF}/{group}.o"]})
        t.group(group, ["libobs/util/bmem.c.h", "other/libobs/util/bmem.cc"])
        self.assertEqual(t.run("ported", "util/bmem.c.o").returncode, 1)

    def test_cmake_unity_batch_counts(self):
        t = self.tree({"libobs": [], "libobs-c-reference": [f"{C_REF}/Unity/unity_0_c.c.o"]})
        t.unity("libobs-c-reference", ["/abs/src/libobs/util/bmem.c"])
        self.assertEqual(t.run("ported", "util/bmem.c.o").returncode, 0)

    def test_every_object_is_checked_and_the_first_failure_is_reported(self):
        t = self.tree({"libobs": [f"{HYBRID}/util/b.c.o"],
                  "libobs-c-reference": [f"{C_REF}/util/a.c.o", f"{C_REF}/util/b.c.o"]})
        r = t.run("ported", "util/a.c.o", "util/b.c.o")
        self.assertEqual(r.returncode, 1)
        self.assertIn("util/b.c.o was compiled into hybrid libobs", r.stdout)

    def test_unknown_target_reads_as_empty(self):
        t = self.tree({"libobs-c-reference": [f"{C_REF}/util/bmem.c.o"]})
        self.assertEqual(t.run("ported", "util/bmem.c.o").returncode, 0)

    def test_compiles_answers_by_exit_code(self):
        obj = "build/obs-nvenc.cuda-load-rust+.h.c.o"
        t = self.tree({"obs-nvenc": [f"plugins/obs-nvenc/CMakeFiles/obs-nvenc.dir/{obj}"]})
        self.assertEqual(t.run("compiles", "obs-nvenc", "-", obj).returncode, 0)
        self.assertEqual(
            t.run("compiles", "obs-nvenc", "-", "build/obs-nvenc.cuda-load+.h.c.o").returncode, 1)

    def test_predicate_matches_the_shell_rules(self):
        m = load()
        lists = m.TargetLists(
            inputs="x/CMakeFiles/t.dir/build/g+.h.c.o\nx/CMakeFiles/t.dir/a/b.c.o\n",
            groups=["build/g+.h.c"],
            unity=["/s/libobs/q/r.c"],
        )
        reads = {"build/g+.h.c": '#include "libobs/p/s.c"\n'}
        self.assertTrue(m.compiled(lists, "t", "libobs/a/b.c", "a/b.c.o", reads.get))
        self.assertTrue(m.compiled(lists, "t", "libobs/p/s.c", "p/s.c.o", reads.get))
        self.assertTrue(m.compiled(lists, "t", "libobs/q/r.c", "q/r.c.o", reads.get))
        self.assertFalse(m.compiled(lists, "t", "libobs/q/z.c", "q/z.c.o", reads.get))


class ValidateUsesItTest(unittest.TestCase):
    def test_validate_runs_the_proof_in_one_process(self):
        text = VALIDATE.read_text()
        self.assertIn('ported_proof.py" --build "$build" --src "$SRC" ported $ported_objs', text)
        self.assertNotIn("for obj in $ported_objs", text)
        self.assertNotIn("target_lists()", text)

    def test_nvenc_check_uses_the_same_predicate(self):
        text = VALIDATE.read_text()
        self.assertIn('ported_proof.py" --build "$build" --src "$SRC" compiles "$target" -', text)


if __name__ == "__main__":
    unittest.main()
