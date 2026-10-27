#!/usr/bin/env -S uv run --script
"""SWIG's Python runtime cannot compile under -Wpedantic (#2695).

    uv run python -m unittest ci/test_scripting_pedantic_exemption.py

#2623 added `-Wpedantic` to the C option lists, and the Ubuntu app build then
failed in `shared/obs-scripting`: the SWIG-generated `swig/swigpyrun.h`
converts function pointers to object pointers, and the upstream `py_error()`
macro in `obs-scripting-python.h` uses `__FUNCTION__`. Neither is our code, so
the `obs-scripting` target opts out of that one diagnostic in
`shared/obs-scripting/cmake/python.cmake`, beside the `-Wno-cast-qual`
exemption that already exists there for the same generated header (#516).

These tests keep the exemption where it belongs. A target-scoped
`-Wno-pedantic` is the fix; deleting `-Wpedantic` from the global C flag lists
would silently drop it for the whole tree.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PYTHON_CMAKE = ROOT / "shared" / "obs-scripting" / "cmake" / "python.cmake"
CLANG_OPTIONS = ROOT / "cmake" / "common" / "compiler_common.cmake"
GNU_OPTIONS = ROOT / "cmake" / "linux" / "compilerconfig.cmake"

C_TARGET_OPTIONS = re.compile(
    r"target_compile_options\(\s*obs-scripting\s+PRIVATE(.*?)\)\s*$",
    re.S | re.M,
)


def scripting_target_options(text: str) -> str:
    """Every `target_compile_options(obs-scripting PRIVATE ...)` argument list."""
    blocks = [m.group(1) for m in C_TARGET_OPTIONS.finditer(text)]
    assert blocks, "no target_compile_options(obs-scripting PRIVATE ...) in python.cmake"
    return "\n".join(blocks)


class ScriptingPedanticExemptionTest(unittest.TestCase):
    def setUp(self):
        self.python_cmake = PYTHON_CMAKE.read_text(encoding="utf-8")

    def test_obs_scripting_opts_out_of_pedantic_for_c(self):
        """The target must pass -Wno-pedantic to C compilers only, never to C++."""
        options = scripting_target_options(self.python_cmake)
        self.assertIn("-Wno-pedantic", options)
        guard = re.search(
            r"\$<\$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:[^>]*"
            r"-Wno-pedantic[^>]*>",
            options,
        )
        self.assertIsNotNone(
            guard,
            "-Wno-pedantic must sit inside the C-only COMPILE_LANG_AND_ID guard",
        )

    def test_exemption_is_paired_with_the_cast_qual_one(self):
        """Both exonerations come from the same generated header (#516, #2695)."""
        options = scripting_target_options(self.python_cmake)
        self.assertIn("-Wno-cast-qual", options)

    def test_global_c_flag_lists_keep_pedantic(self):
        """The fix is target-scoped: the tree still compiles C at -Wpedantic."""
        clang = CLANG_OPTIONS.read_text(encoding="utf-8")
        clang_c = re.search(r"set\(\s*_obs_clang_c_options(.*?)\)", clang, re.S)
        if clang_c is None:
            self.fail("no _obs_clang_c_options in compiler_common.cmake")
        self.assertIn("-Wpedantic", clang_c.group(1))

        gnu = GNU_OPTIONS.read_text(encoding="utf-8")
        gnu_c = [
            line for line in gnu.splitlines() if "COMPILE_LANG_AND_ID:C,GNU" in line
        ]
        self.assertTrue(gnu_c, "no GNU C option line in compilerconfig.cmake")
        self.assertTrue(
            any("-Wpedantic" in line for line in gnu_c),
            "the GNU C option line must keep -Wpedantic",
        )


if __name__ == "__main__":
    unittest.main()
