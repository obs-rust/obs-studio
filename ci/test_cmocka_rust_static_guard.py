#!/usr/bin/env -S uv run --script
"""cmocka tests use libobs_rust-static only inside if(TARGET libobs_rust-static).

    uv run python -m unittest ci/test_cmocka_rust_static_guard.py

The Rust static library exists only with OBS_LIBOBS_BACKEND=HYBRID. With the C
backend (the rollback build), a test that links it unconditionally fails to
link ("cannot find -llibobs_rust-static"), and CI builds no C backend with
unit tests, so only this check sees it there.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "libobs_rust-static"
GUARD = re.compile(r"^\s*if\s*\(\s*TARGET\s+libobs_rust-static\s*\)", re.I)
OPEN = re.compile(r"^\s*if\s*\(", re.I)
ELSE = re.compile(r"^\s*else(if)?\s*\(", re.I)
CLOSE = re.compile(r"^\s*endif\s*\(", re.I)


def unguarded(text: str) -> list[tuple[int, str]]:
    """Lines naming the Rust static library outside an if(TARGET ...) guard."""
    found = []
    stack: list[bool] = []
    for number, line in enumerate(text.splitlines(), 1):
        code = line.split("#", 1)[0]
        if OPEN.match(code):
            stack.append(bool(GUARD.match(code)))
            continue
        if ELSE.match(code):
            # The other branch of a guard is where the library is missing.
            if stack:
                stack[-1] = False
            continue
        if CLOSE.match(code):
            if stack:
                stack.pop()
            continue
        if TARGET in code and not any(stack):
            found.append((number, line.strip()))
    return found


class CmockaRustStaticGuardTest(unittest.TestCase):
    def test_rust_static_lib_is_guarded(self):
        found = []
        for path in sorted((ROOT / "test" / "cmocka" / "tests.d").glob("*.cmake")):
            for number, line in unguarded(path.read_text()):
                found.append(f"{path.relative_to(ROOT)}:{number}: {line}")
        self.assertEqual(found, [])

    def test_checker_sees_an_unguarded_link(self):
        self.assertEqual(unguarded(f"target_link_libraries(t PRIVATE {TARGET})\n"), [(1, f"target_link_libraries(t PRIVATE {TARGET})")])
        self.assertEqual(unguarded(f"if(OS_LINUX)\n  target_link_libraries(t PRIVATE {TARGET})\nendif()\n"), [(2, f"target_link_libraries(t PRIVATE {TARGET})")])
        self.assertEqual(unguarded(f"if(TARGET {TARGET})\n  if(OS_LINUX)\n    target_link_libraries(t PRIVATE {TARGET})\n  endif()\nendif()\n"), [])
        self.assertEqual(unguarded(f"if(TARGET {TARGET})\nelse()\n  target_link_libraries(t PRIVATE {TARGET})\nendif()\n"), [(3, f"target_link_libraries(t PRIVATE {TARGET})")])
        self.assertEqual(unguarded(f"# links {TARGET}\n"), [])


if __name__ == "__main__":
    unittest.main()
