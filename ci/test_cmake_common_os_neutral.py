#!/usr/bin/env -S uv run --script
"""cmake/common/ carries no host-OS branches (#3234, tracker #3232).

`ci/ci_scope.py` treats every `cmake/common/` edit as shared and starts the
macOS and Windows lanes. A Windows- or macOS-only CMake change belongs in
`cmake/windows/`, `cmake/macos/` or `cmake/linux/`, which
`ci/platform_paths.py` maps to one OS. This test fails a new `OS_<HOST>`
branch, an MSVC generator expression, or a CMAKE_OSX_* / CMAKE_VS_* read in
a common module; the OS module then overrides a neutral hook instead.

    uv run python -m unittest ci/test_cmake_common_os_neutral.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

from ci_scope import scope

COMMON = ROOT / "cmake" / "common"

HOST_TOKENS = re.compile(
    r"\bOS_(WINDOWS|MACOS|LINUX|FREEBSD|OPENBSD|POSIX)\b|\bMSVC\b|\bCMAKE_OSX_\w+|\bCMAKE_VS_\w+|\bCMAKE_HOST_SYSTEM_NAME\b"
)

# Files that must name the hosts, and why.
ALLOWED = {
    # Defines OS_WINDOWS/OS_MACOS/OS_LINUX from CMAKE_HOST_SYSTEM_NAME and
    # puts cmake/<os>/ on CMAKE_MODULE_PATH: the one place the host is chosen.
    "osconfig.cmake",
    # The cross-compile lane (cmake/cross/, CROSS_SELF in ci_scope.py) targets
    # every OS from one Linux host by design (#1294).
    "soldr.cmake",
}


def code_lines(text: str):
    for number, line in enumerate(text.splitlines(), 1):
        code = line.split("#", 1)[0]
        if code.strip():
            yield number, code


class CmakeCommonOsNeutralTest(unittest.TestCase):
    def test_no_host_branches_in_common_modules(self) -> None:
        found = []
        for path in sorted(COMMON.glob("*.cmake")):
            if path.name in ALLOWED:
                continue
            for number, code in code_lines(path.read_text(encoding="utf-8")):
                if HOST_TOKENS.search(code):
                    found.append(f"cmake/common/{path.name}:{number}: {code.strip()}")
        self.assertEqual(found, [], "move host-only CMake into cmake/<os>/")

    def test_allowlist_is_still_needed(self) -> None:
        for name in ALLOWED:
            text = (COMMON / name).read_text(encoding="utf-8")
            self.assertTrue(any(HOST_TOKENS.search(c) for _, c in code_lines(text)), name)

    def test_moved_os_modules_select_only_their_os(self) -> None:
        for path, macos, windows in (
            ("cmake/macos/buildspec.cmake", True, False),
            ("cmake/windows/helpers.cmake", False, True),
            ("cmake/linux/cpackconfig.cmake", False, False),
        ):
            with self.subTest(path=path):
                got = scope([path])
                self.assertEqual((got.macos, got.windows), (macos, windows))


if __name__ == "__main__":
    unittest.main()
