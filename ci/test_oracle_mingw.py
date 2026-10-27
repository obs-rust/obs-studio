#!/usr/bin/env -S uv run --script
"""The windows-gnu oracle flags compile libobs's audio math with MinGW GCC.

MinGW's isfinite() is a type-generic macro whose float branch casts its
argument to float, and GCC checks that branch for a double too, so
audio-math.h's `isfinite((double)db)` broke the oracle's windows-gnu build
once -Wconversion joined STRICT_FLAGS (#2648). This compiles the oracle's
audio_math.c with build.rs's warning flags and its windows-gnu flags. It
skips when no MinGW cross GCC is installed.
"""

from pathlib import Path
import os
import re
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent.parent
ORACLE = ROOT / "rust/obs-c-oracle"
MINGW_GCC = "x86_64-w64-mingw32-gcc"


def mingw_gcc() -> str | None:
    """cc-rs's override, the toolchain soldr builds the oracle with, then PATH.

    soldr's comes before PATH: a distribution's MinGW headers can define
    isfinite() differently, and then miss what the oracle build hits.
    """
    for var in ("CC_x86_64_pc_windows_gnu", "CC_x86_64-pc-windows-gnu"):
        if os.environ.get(var):
            return os.environ[var]
    soldr = Path.home() / ".soldr/bin/syslib/mingw-w64-cross"
    candidates = sorted(soldr.glob(f"*/*/package/bin/{MINGW_GCC}"))
    if candidates:
        return str(candidates[-1])
    return shutil.which(MINGW_GCC)


def oracle_flags() -> list[str]:
    """-Wall -Wextra -Werror, STRICT_FLAGS, then the windows-gnu block's flags."""
    build = (ORACLE / "build.rs").read_text()
    strict = re.search(r"const STRICT_FLAGS: &\[&str\] = &\[(.*?)\];", build, re.S)
    assert strict, "build.rs defines STRICT_FLAGS"
    gnu = re.search(r"\n    if windows_gnu \{(.*?)\n    \}", build, re.S)
    assert gnu, "build.rs has an `if windows_gnu` block"
    return [
        "-Wall",
        "-Wextra",
        "-Werror",
        *re.findall(r'"([^"]+)"', strict.group(1)),
        *re.findall(r'\.flag\("([^"]+)"\)', gnu.group(1)),
    ]


class OracleMingwTest(unittest.TestCase):
    def test_audio_math_compiles_with_windows_gnu_flags(self):
        cc = mingw_gcc()
        if cc is None:
            self.skipTest(f"no {MINGW_GCC} (soldr installs it with the windows-gnu target)")
        cmd = [
            cc,
            "-std=c11",
            "-fsyntax-only",
            *oracle_flags(),
            "-I",
            str(ROOT / "libobs"),
            str(ORACLE / "oracle/audio_math.c"),
        ]
        result = subprocess.run(cmd, cwd=ORACLE, capture_output=True, text=True, errors="replace", check=False)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
