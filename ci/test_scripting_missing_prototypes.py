#!/usr/bin/env -S uv run --script
"""SWIG's scripting sources declare their entry points without prototypes (#2703).

    uv run python -m unittest ci/test_scripting_missing_prototypes.py

#2678 put `-Wmissing-prototypes`/`-Wstrict-prototypes` in the global C option
lists, and the Ubuntu app build (`OBS-Studio-Rust`) and the Flatpak build then
stopped in `shared/obs-scripting`: the SWIG-generated glue declares functions
like `obs_lua_script_load` and `add_python_frontend_funcs` with no prototype in
scope, which is `-Werror=missing-prototypes` under a generated file we do not
own (`swig -external-runtime` output and the `*_wrap.c` objects).

The fix is the #2696 shape one diagnostic over: each SWIG target opts out of
that one warning beside the `-Wno-cast-qual` exemption that exists for the same
generated files (#516). These tests keep the exemption where it belongs -- the
global C flag lists must keep `-Wmissing-prototypes`, or the #2624 ratchet
silently loses the rule for first-party code.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTING = ROOT / "shared" / "obs-scripting"
CLANG_OPTIONS = ROOT / "cmake" / "common" / "compiler_common.cmake"
GNU_OPTIONS = ROOT / "cmake" / "linux" / "compilerconfig.cmake"

# (target, file that declares its options) -- every site that already exempts
# the generated code from -Wcast-qual.
SITES = [
    ("obs-scripting", SCRIPTING / "cmake" / "lua.cmake"),
    ("obs-scripting", SCRIPTING / "cmake" / "python.cmake"),
    ("obspython", SCRIPTING / "obspython" / "CMakeLists.txt"),
    ("obslua", SCRIPTING / "obslua" / "CMakeLists.txt"),
]

C_ONLY = re.compile(
    r"\$<\$<COMPILE_LANG_AND_ID:C,AppleClang,Clang,GNU>:\s*([^>]*)>"
)


def option_blocks(text: str, target: str) -> list[str]:
    """The argument lists of every `target_compile_options(<target> ...)` call."""
    pattern = re.compile(
        rf"target_compile_options\(\s*{re.escape(target)}\b(.*?)\)\s*$",
        re.S | re.M,
    )
    return [m.group(1) for m in pattern.finditer(text)]


def exempted_c_warnings(options: str) -> set[str]:
    """The -Wno-* tokens declared inside C-only generator expressions."""
    found: set[str] = set()
    for guarded in C_ONLY.findall(options):
        found.update(re.findall(r"-Wno-[a-z0-9-]+", guarded))
    return found


class ScriptingMissingPrototypesExemptionTest(unittest.TestCase):
    def test_every_cast_qual_exemption_also_exempts_the_prototype_rules(self):
        """Each SWIG site opts out of both prototype diagnostics, C-only."""
        for target, path in SITES:
            with self.subTest(target=target, path=str(path.relative_to(ROOT))):
                blocks = option_blocks(path.read_text(encoding="utf-8"), target)
                self.assertTrue(blocks, f"no target_compile_options({target} ...)")
                exempting = [
                    b for b in blocks if "-Wno-cast-qual" in exempted_c_warnings(b)
                ]
                self.assertTrue(
                    exempting,
                    f"{target}: expected an existing C-only -Wno-cast-qual "
                    "exemption for the SWIG-generated sources (#516)",
                )
                for block in exempting:
                    guarded = exempted_c_warnings(block)
                    for diagnostic in ("-Wno-missing-prototypes", "-Wno-strict-prototypes"):
                        self.assertIn(
                            diagnostic,
                            guarded,
                            f"{target}: {diagnostic} must sit in the same C-only "
                            "COMPILE_LANG_AND_ID guard, never in a C++ list (#2703)",
                        )

    def test_global_c_flag_lists_keep_missing_prototypes(self):
        """The fix is target-scoped: first-party C still compiles at the rule."""
        clang = CLANG_OPTIONS.read_text(encoding="utf-8")
        clang_c = re.search(r"set\(\s*_obs_clang_c_options(.*?)\)", clang, re.S)
        if clang_c is None:
            self.fail("no _obs_clang_c_options in compiler_common.cmake")
        self.assertIn("-Wmissing-prototypes", clang_c.group(1))
        self.assertIn("-Wstrict-prototypes", clang_c.group(1))

        gnu = GNU_OPTIONS.read_text(encoding="utf-8")
        gnu_c = [
            line for line in gnu.splitlines() if "COMPILE_LANG_AND_ID:C,GNU" in line
        ]
        self.assertTrue(gnu_c, "no GNU C option line in compilerconfig.cmake")
        self.assertTrue(
            any("-Wmissing-prototypes" in line for line in gnu_c),
            "the GNU C option line must keep -Wmissing-prototypes",
        )


if __name__ == "__main__":
    unittest.main()
