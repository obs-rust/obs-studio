#!/usr/bin/env -S uv run --script
"""Tests for rust/tools/linux-validate/drop_depless_objects.py (#2244).

    uv run python -m unittest ci/test_tier2_depless_objects.py

Some local Tier 2 build volumes had hundreds of objects that ninja recorded with
zero header dependencies (`ninja -t deps`: "#deps 0"), libobs unity groups and
its precompiled header among them. Ninja never rebuilds such an object after a
header edit, so Tier 2 tested the old libobs and could report a wrong RED or
GREEN, and the local gate could stamp it. Every compiled C/C++ object includes
at least one header (stdc-predef.h), so zero deps is never right: before each
build, validate.sh deletes those objects, and ninja compiles them again from
the current sources (a cache hit is keyed on those sources too).
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "rust" / "tools" / "linux-validate" / "drop_depless_objects.py"
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"

DEPS = """\
libobs/CMakeFiles/libobs-objects.dir/cmake_pch.h.gch: #deps 0, deps mtime 1792620571433351740 (VALID)

libobs/CMakeFiles/libobs-objects.dir/build/libobs.core+.h.c.o: #deps 0, deps mtime 0 (STALE)

libobs/CMakeFiles/libobs-objects.dir/build/libobs.graphics+.h.c.o: #deps 3, deps mtime 1792620571433351740 (VALID)
    /src/libobs/graphics/graphics.c.h
    /usr/include/stdc-predef.h
    /src/libobs/obs-internal.h

"""


class DropDeplessObjects(unittest.TestCase):
    def run_script(self, deps: str) -> tuple[subprocess.CompletedProcess, Path]:
        tmp = Path(tempfile.mkdtemp())
        build = tmp / "tree"
        bin_dir = tmp / "bin"
        bin_dir.mkdir()
        fake = bin_dir / "ninja"
        (tmp / "deps.txt").write_text(deps)
        fake.write_text(textwrap.dedent(f"""\
            #!/usr/bin/env bash
            [ "$1" = -C ] && [ "$3" = -t ] && [ "$4" = deps ] && exec cat {tmp / "deps.txt"}
            exit 2
            """))
        fake.chmod(0o755)
        for line in deps.splitlines():
            if line and not line.startswith(" "):
                obj = build / line.split(":", 1)[0]
                obj.parent.mkdir(parents=True, exist_ok=True)
                obj.write_text("object")
        env = dict(os.environ, PATH=f"{bin_dir}:{os.environ['PATH']}")
        result = subprocess.run([sys.executable, str(SCRIPT), str(build)], env=env, capture_output=True, text=True)
        return result, build

    def test_objects_with_zero_deps_are_deleted(self) -> None:
        result, build = self.run_script(DEPS)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((build / "libobs/CMakeFiles/libobs-objects.dir/cmake_pch.h.gch").exists())
        self.assertFalse((build / "libobs/CMakeFiles/libobs-objects.dir/build/libobs.core+.h.c.o").exists())
        self.assertTrue((build / "libobs/CMakeFiles/libobs-objects.dir/build/libobs.graphics+.h.c.o").exists())
        self.assertIn("2 objects", result.stdout)

    def test_a_clean_tree_is_left_alone(self) -> None:
        clean = DEPS.split("\n\n", 2)[2]
        result, build = self.run_script(clean)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue((build / "libobs/CMakeFiles/libobs-objects.dir/build/libobs.graphics+.h.c.o").exists())
        self.assertEqual(result.stdout, "")

    def test_validate_runs_it_before_the_build(self) -> None:
        text = VALIDATE.read_text(encoding="utf-8")
        guard = text.find('drop_depless_objects.py')
        build = text.find('cmake --build "$build" --target "${tier2_targets[@]}"')
        self.assertGreater(guard, 0, "validate.sh does not run drop_depless_objects.py")
        self.assertLess(guard, build)


if __name__ == "__main__":
    unittest.main()
