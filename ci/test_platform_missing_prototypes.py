#!/usr/bin/env -S uv run --script
"""libobs-opengl's platform sources declare their file-scope functions static (#2703).

    uv run python -m unittest ci/test_platform_missing_prototypes.py

#2678 put `-Wmissing-prototypes` in the global C option lists. The PR lanes never
compile the platform-only OpenGL backends (the Wayland/X11 EGL sources are built
only by the app builds), so a non-static file-scope definition without a
prototype there first surfaces in a `Scheduled`/release dispatch as
`-Werror=missing-prototypes`, one diagnostic and one translation unit at a time:
#2704 cleared `shared/obs-scripting`, and the next dispatch stopped on

    libobs-opengl/gl-wayland-egl.c.h:90: error: no previous prototype for
    'gl_wayland_egl_windowinfo_create' [-Werror=missing-prototypes]

The X11 sibling (`gl_x11_egl_windowinfo_create`) is already `static`, and that is
the fix: a helper used only through its `gl_winsys_vtable` slot is file-scope
private. This test keeps the class closed by scanning every platform unity
source in `libobs-opengl/` for a file-scope definition that is neither `static`
nor declared in a tracked header.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PLATFORM = ROOT / "libobs-opengl"

# A definition starts at column 0 with a return type and a name, and its
# argument list runs to the end of the line. Declarations end in ';' and are
# skipped; `static`, preprocessor lines, comments and indented bodies are not
# definitions.
DEFINITION = re.compile(r"^([A-Za-z_][\w \t\*]*?)\b(\w+)\s*\(([^;{]*)\)\s*$")
NOT_A_NAME = {"if", "for", "while", "switch", "return", "sizeof", "do", "else"}


def tracked_headers() -> list[Path]:
    """Every header a translation unit may see a prototype in."""
    return [
        path
        for path in ROOT.rglob("*")
        if path.suffix in {".h", ".hpp"}
        and not path.name.endswith(".c.h")
        and ".git" not in path.parts
        and "target" not in path.parts
    ]


def has_prototype(name: str, headers: dict[Path, str]) -> bool:
    pattern = re.compile(rf"\b{re.escape(name)}\s*\([^;]{{0,400}}?\)\s*;", re.S)
    return any(pattern.search(text) for text in headers.values())


def offending_definitions(headers: dict[Path, str]) -> list[str]:
    """`path:line: name` for each non-static definition with no prototype."""
    findings: list[str] = []
    for path in sorted(PLATFORM.glob("*.c.h")):
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        for number, line in enumerate(lines, start=1):
            if line.startswith(("static ", " ", "\t", "}", "#", "//", "/*")):
                continue
            if not line.strip():
                continue
            match = DEFINITION.match(line)
            if match is None:
                continue
            name = match.group(2)
            if name in NOT_A_NAME or has_prototype(name, headers):
                continue
            findings.append(f"{path.relative_to(ROOT)}:{number}: {name}")
    return findings


class PlatformMissingPrototypesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.headers = {
            path: path.read_text(encoding="utf-8", errors="replace")
            for path in tracked_headers()
        }

    def test_every_platform_definition_is_static_or_declared(self):
        """No file-scope OpenGL-platform definition hides from the prototype rule."""
        findings = offending_definitions(self.headers)
        self.assertEqual(
            findings,
            [],
            "these libobs-opengl platform definitions are neither `static` nor "
            "declared in a header, so the app builds fail with "
            "-Werror=missing-prototypes (#2703):\n  " + "\n  ".join(findings),
        )


if __name__ == "__main__":
    unittest.main()
