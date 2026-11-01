#!/usr/bin/env -S uv run --script
"""The libobs lib-module cache keys modules by what they compile (#3262).

    uv run python -m unittest ci/test_libobs_modules_cache.py

ci/libobs_modules.py keys each lib-module by its compile commands, the
files they read and the compiler. These tests build a tiny fake tree (a
compile_commands.json over two groups in two modules, compiled by the host
C compiler) and check that the key:

- is the same in two build dirs of one checkout;
- changes for the module whose header changed, and only for it;
- ignores the compiler launcher and a cache hit's probe target name.

They also check that libobs/cmake/modules.cmake imports a hit.
"""

from __future__ import annotations

import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location("libobs_modules", ROOT / "ci" / "libobs_modules.py")
assert SPEC and SPEC.loader
mods = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(mods)

CC = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")


def fake_tree(base: Path, build_name: str, launcher: str | None = None, target_suffix: str = "") -> tuple[Path, Path]:
    source = base / "src"
    (source / "libobs" / "build").mkdir(parents=True, exist_ok=True)
    (source / "libobs" / "util").mkdir(parents=True, exist_ok=True)
    (source / "libobs" / "util" / "a.h").write_text("int a(void);\n") if not (
        source / "libobs" / "util" / "a.h"
    ).exists() else None
    (source / "libobs" / "graphics.h").write_text("int g(void);\n") if not (
        source / "libobs" / "graphics.h"
    ).exists() else None
    groups = {"util-cf": "util/a.h", "graphics": "graphics.h"}
    build = base / build_name
    build.mkdir(exist_ok=True)
    entries = []
    for group, header in groups.items():
        src = source / "libobs" / "build" / f"libobs.{group}+.h.c"
        src.write_text(f'#include "{header}"\n')
        module = "core" if group.startswith("util") else "graphics"
        obj = f"libobs/CMakeFiles/libobs-mod-{module}{target_suffix}.dir/build/libobs.{group}+.h.c.o"
        args = [*([launcher] if launcher else []), CC, f"-I{source}/libobs", f"-I{build}/config", "-O2", "-o", obj, "-c", str(src)]
        entries.append({"directory": str(build), "file": str(src), "output": obj, "arguments": args})
    (build / "compile_commands.json").write_text(json.dumps(entries))
    return source, build


@unittest.skipUnless(CC, "no C compiler")
class KeyTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp)

    def keys(self, build: Path, source: Path) -> dict[str, str]:
        return mods.keys(build, source)

    def test_two_build_dirs_agree(self):
        source, one = fake_tree(self.tmp, "b1")
        _, two = fake_tree(self.tmp, "b2")
        k1 = self.keys(one, source)
        self.assertEqual(set(k1), {"core", "graphics"})
        self.assertEqual(k1, self.keys(two, source))

    def test_a_header_edit_changes_only_its_module(self):
        source, build = fake_tree(self.tmp, "b")
        before = self.keys(build, source)
        (source / "libobs" / "graphics.h").write_text("int g(void);\nint h(void);\n")
        after = self.keys(build, source)
        self.assertNotEqual(before["graphics"], after["graphics"])
        self.assertEqual(before["core"], after["core"])

    def test_launcher_and_probe_name_do_not_change_the_key(self):
        source, plain = fake_tree(self.tmp, "b1")
        _, probed = fake_tree(self.tmp, "b2", launcher="zccache", target_suffix="-keyprobe")
        self.assertEqual(self.keys(plain, source), self.keys(probed, source))


class CmakeImportTest(unittest.TestCase):
    def test_a_hit_is_imported_not_compiled(self):
        text = (ROOT / "libobs" / "cmake" / "modules.cmake").read_text(encoding="utf-8")
        self.assertIn('include("${OBS_LIBOBS_MODULE_CACHE}/hits.cmake")', text)
        self.assertRegex(text, r"add_library\(\$\{_target\} STATIC IMPORTED\)")
        self.assertIn("EXCLUDE_FROM_ALL", text, "a hit's commands must stay visible to the key")


if __name__ == "__main__":
    unittest.main()
