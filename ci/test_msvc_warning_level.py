#!/usr/bin/env -S uv run --script
"""The MSVC build compiles at /W4 and does not hand /WX to the linker (#2626).

    uv run python -m unittest ci/test_msvc_warning_level.py

`/WX` is not a compile flag this file sets: `CMAKE_COMPILE_WARNING_AS_ERROR`
in `cmake/common/compiler_common.cmake` makes CMake pass it to `cl.exe`
(#516). The `add_link_options(/WX)` block was dead — `link.exe` ignores
`/WX` — so it is gone and only the compile-time setting carries the intent.
The warning level itself is `/W4`, the strictest level `cl.exe` has.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WINDOWS = ROOT / "cmake" / "windows" / "compilerconfig.cmake"
COMMON = ROOT / "cmake" / "common" / "compiler_common.cmake"


def call_tokens(text: str, name: str) -> list[str]:
    """Every whitespace-separated argument of each `name(...)` call."""
    calls = re.findall(rf"{name}\(([^)]*)\)", text, re.S)
    if not calls:
        raise AssertionError(f"no {name}( call")
    return [token for body in calls for token in body.split()]


class MsvcWarningLevelTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = WINDOWS.read_text(encoding="utf-8")

    def test_compile_options_request_w4(self) -> None:
        options = call_tokens(self.text, "add_compile_options")
        self.assertIn("/W4", options)
        self.assertNotIn("/W3", options)

    def test_no_w3_anywhere(self) -> None:
        self.assertIsNone(re.search(r"(?<![\w/])/W3\b", self.text), "/W3 left in the MSVC config")

    def test_link_options_do_not_pass_wx(self) -> None:
        self.assertNotIn("/WX", call_tokens(self.text, "add_link_options"))

    def test_wx_comes_from_the_compile_time_setting(self) -> None:
        common = COMMON.read_text(encoding="utf-8")
        self.assertIn("set(CMAKE_COMPILE_WARNING_AS_ERROR ON)", common)
        self.assertNotIn("/WX", call_tokens(self.text, "add_compile_options"))


if __name__ == "__main__":
    unittest.main()
