#!/usr/bin/env -S uv run --script
"""SWIG's scripting runtimes cannot compile under -Wpedantic (#2695).

    uv run python -m unittest ci/test_scripting_pedantic_exemption.py

#2623 added `-Wpedantic` to the C option lists, and the Ubuntu app build then
failed in `shared/obs-scripting`. Every diagnostic is in generated code:

* `swig/swigpyrun.h` / `swig/swigluarun.h` (swig `-external-runtime`) convert
  function pointers to object pointers — compiled into `obs-scripting`;
* the SWIG wrapper objects `obspythonPYTHON_wrap.c` and `obsluaLUA_wrap.c`
  (swig `-python` / `-lua`) do the same — the `obspython` and `obslua`
  targets;
* the upstream `py_error()` macro in `obs-scripting-python.h` uses
  `__FUNCTION__` — `obs-scripting` again.

None of that is our code, so each target opts out of that one diagnostic
beside the `-Wno-cast-qual` exemption that already exists for the same
generated files (#516). These tests keep the exemption where it belongs: the
global C flag lists must keep `-Wpedantic`, or the whole tree silently loses
it.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTING = ROOT / "shared" / "obs-scripting"
CLANG_OPTIONS = ROOT / "cmake" / "common" / "compiler_common.cmake"
GNU_OPTIONS = ROOT / "cmake" / "linux" / "compilerconfig.cmake"

# target -> every file that compiles SWIG-generated code into that target, so
# the exemption cannot hide behind one of the ENABLE_SCRIPTING_* options
# (#2711: it lived in python.cmake, and the AppImage build turns Python off).
TARGETS = {
    "obs-scripting": (
        SCRIPTING / "cmake" / "lua.cmake",
        SCRIPTING / "cmake" / "python.cmake",
    ),
    "obspython": (SCRIPTING / "obspython" / "CMakeLists.txt",),
    "obslua": (SCRIPTING / "obslua" / "CMakeLists.txt",),
}

C_ONLY_PEDANTIC = re.compile(
    r"\$<\$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:\s*[^>]*-Wno-pedantic[^>]*>"
)


def target_options(text: str, target: str) -> str:
    """The argument lists of every `target_compile_options(<target> ...)` call."""
    pattern = re.compile(
        rf"target_compile_options\(\s*{re.escape(target)}\b(.*?)\)\s*$",
        re.S | re.M,
    )
    blocks = [m.group(1) for m in pattern.finditer(text)]
    assert blocks, f"no target_compile_options({target} ...) block"
    return "\n".join(blocks)


class ScriptingPedanticExemptionTest(unittest.TestCase):
    def test_each_generated_swig_target_opts_out_of_pedantic(self):
        """-Wno-pedantic on every SWIG target, always inside a C-only genex."""
        for target, paths in TARGETS.items():
            for path in paths:
                with self.subTest(target=target, file=path.name):
                    options = target_options(path.read_text(encoding="utf-8"), target)
                    self.assertIn("-Wno-pedantic", options)
                    self.assertIsNotNone(
                        C_ONLY_PEDANTIC.search(options),
                        f"{target} ({path.name}): -Wno-pedantic must sit in the "
                        "C-only COMPILE_LANG_AND_ID guard, never in a C++ option "
                        "list",
                    )

    def test_the_lua_sources_are_exempt_even_with_python_off(self):
        """The AppImage build sets ENABLE_SCRIPTING_PYTHON=OFF and still links
        the Lua SWIG runtime, so python.cmake cannot be its only exemption."""
        lua = target_options(
            (SCRIPTING / "cmake" / "lua.cmake").read_text(encoding="utf-8"),
            "obs-scripting",
        )
        self.assertIsNotNone(
            C_ONLY_PEDANTIC.search(lua),
            "cmake/lua.cmake must carry -Wno-pedantic for obs-scripting: with "
            "ENABLE_SCRIPTING_PYTHON=OFF (build-aux/appimage/build.py) "
            "python.cmake's exemption is not applied at all (#2711)",
        )

    def test_exemptions_match_the_cast_qual_precedent(self):
        """Both exonerations cover the same generated files (#516, #2695)."""
        for target, paths in TARGETS.items():
            for path in paths:
                with self.subTest(target=target, file=path.name):
                    options = target_options(path.read_text(encoding="utf-8"), target)
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
