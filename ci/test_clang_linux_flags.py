"""A Clang build of libobs on Linux is not stopped by GNU extensions (#3398).

Usage:
    uv run python -m unittest ci/test_clang_linux_flags.py

The Linux C uses `, ##__VA_ARGS__` (the blog() wrappers, e.g.
libobs/audio-monitoring/pulse/pulseaudio-output.c.h) and statement
expressions from macros on purpose. GCC's -Wpedantic accepts both under
gnu17; Clang's -Wpedantic flags them, and with warnings as errors a Clang
build of libobs failed (302 + 16 errors, Ubuntu clang 22.1.2). No CI lane
builds Linux with Clang, so this guards the switch that turns them off.
"""

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINUX = ROOT / "cmake/linux/compilerconfig.cmake"


class ClangLinuxFlagsTest(unittest.TestCase):
    def test_gnu_extension_warnings_are_off_for_clang_c(self):
        text = LINUX.read_text(encoding="utf-8")
        clang = [line for line in text.splitlines() if "COMPILE_LANG_AND_ID:C,Clang" in line]
        flags = ";".join(clang)
        self.assertIn("-Wno-gnu-zero-variadic-macro-arguments", flags)
        self.assertIn("-Wno-gnu-statement-expression-from-macro-expansion", flags)

    def test_gcc_keeps_its_options(self):
        text = LINUX.read_text(encoding="utf-8")
        gcc = ";".join(line for line in text.splitlines() if "COMPILE_LANG_AND_ID:C,GNU" in line)
        self.assertNotIn("-Wno-gnu-zero-variadic-macro-arguments", gcc)


if __name__ == "__main__":
    unittest.main()
