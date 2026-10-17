#!/usr/bin/env -S uv run --script
"""Every cmocka test includes <cmocka.h> so that cmocka 1.1 can compile it.

    uv run python -m unittest ci/test_cmocka_includes.py

cmocka 1.1 (Debian 13 libcmocka-dev 1.1.7) needs <stdarg.h>, <stddef.h> and
<setjmp.h> before <cmocka.h>, and its header has no extern "C" guard (the
Windows cmocka.h has none either). A C file without the three headers stops with "unknown type
name 'va_list'"; a C++ file without extern "C" stops at link time with
"undefined reference to `_assert_true(unsigned long, ...)'".
"""

from __future__ import annotations

import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CMOCKA = re.compile(r"^\s*#\s*include\s*<cmocka\.h>", re.M)
NEEDED = ("stdarg.h", "stddef.h", "setjmp.h")
CXX = (".cpp", ".cc", ".cxx", ".mm")


def cmocka_users() -> list[str]:
    out = subprocess.run(
        ["git", "-C", str(ROOT), "grep", "-lE", r"#[[:space:]]*include[[:space:]]*<cmocka\.h>", "--", "*.c", "*.cpp", "*.cc", "*.cxx", "*.mm"],
        capture_output=True,
        text=True,
    ).stdout
    return out.split()


def problems(path: str, src: str) -> list[str]:
    match = CMOCKA.search(src)
    if not match:
        return []
    before = src[: match.start()]
    found = []
    for header in NEEDED:
        if not re.search(r"#\s*include\s*<%s>" % re.escape(header), before):
            found.append(f"<{header}> is not included before <cmocka.h>")
    if path.endswith(CXX):
        opened = len(re.findall(r'extern\s+"C"\s*\{', before))
        if opened == 0:
            found.append('<cmocka.h> is not inside extern "C" { }')
    return found


class CmockaIncludesTest(unittest.TestCase):
    maxDiff = None

    def test_every_cmocka_user_compiles_with_cmocka_1_1(self):
        files = cmocka_users()
        self.assertTrue(files, "no file includes <cmocka.h>")
        bad = {}
        for path in files:
            found = problems(path, (ROOT / path).read_text(encoding="utf-8", errors="replace"))
            if found:
                bad[path] = found
        self.assertEqual(bad, {})

    def test_check_finds_each_problem(self):
        self.assertEqual(
            problems("t.c", "#include <cmocka.h>\n"),
            [f"<{h}> is not included before <cmocka.h>" for h in NEEDED],
        )
        ok_c = "#include <stdarg.h>\n#include <stddef.h>\n#include <setjmp.h>\n#include <cmocka.h>\n"
        self.assertEqual(problems("t.c", ok_c), [])
        self.assertEqual(problems("t.cpp", ok_c), ['<cmocka.h> is not inside extern "C" { }'])
        ok_cpp = ok_c.replace("#include <cmocka.h>", 'extern "C" {\n#include <cmocka.h>\n}')
        self.assertEqual(problems("t.cpp", ok_cpp), [])


if __name__ == "__main__":
    unittest.main()
