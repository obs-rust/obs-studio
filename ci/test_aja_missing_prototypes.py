#!/usr/bin/env -S uv run --script
"""The aja audio repack unit declares its file-scope helpers static (#2703).

    uv run python -m unittest ci/test_aja_missing_prototypes.py

#2678 put `-Wmissing-prototypes` in the global C option lists. The PR lanes never
build the aja plugin (its C sources are compiled only by the app builds), so a
non-static file-scope definition without a prototype there first surfaces in a
`Scheduled`/release dispatch as `-Werror=missing-prototypes`, one diagnostic and
one translation unit at a time. On master the Flatpak app build stops on

    plugins/aja/audio-repack.c.h:5: error: no previous prototype for
    'check_buffer' [-Werror=missing-prototypes]

`audio-repack.c.h` is a unity-group implementation file: its helpers are reached
only through `audio_repack_init`, which is the plugin's public C API declared in
`audio-repack.h`. A helper used only in this translation unit is file-scope
private, so it is `static`, the same fix the OpenGL platform sources took in
#2710. This test keeps the class closed offline: every file-scope definition in
the unit must be `static` or declared in the plugin's own headers.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "plugins" / "aja" / "audio-repack.c.h"
HEADERS = (
    ROOT / "plugins" / "aja" / "audio-repack.h",
    ROOT / "plugins" / "aja" / "audio-repack.hpp",
)

# A definition starts at column 0 with a return type and a name, and its
# argument list runs to the end of the line. Declarations end in ';' and are
# skipped; `static`, preprocessor lines, comments and indented bodies are not
# definitions.
DEFINITION = re.compile(r"^([A-Za-z_][\w \t\*]*?)\b(\w+)\s*\(([^;{]*)\)\s*$")
NOT_A_NAME = {"if", "for", "while", "switch", "return", "sizeof", "do", "else"}


def has_prototype(name: str, headers: dict[Path, str]) -> bool:
    pattern = re.compile(rf"\b{re.escape(name)}\s*\([^;]{{0,400}}?\)\s*;", re.S)
    return any(pattern.search(text) for text in headers.values())


def offending_definitions(headers: dict[Path, str]) -> list[str]:
    """`path:line: name` for each non-static definition with no prototype."""
    findings: list[str] = []
    lines = SOURCE.read_text(encoding="utf-8", errors="replace").splitlines()
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
        findings.append(f"{SOURCE.relative_to(ROOT)}:{number}: {name}")
    return findings


class AjaMissingPrototypesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.headers = {
            path: path.read_text(encoding="utf-8", errors="replace")
            for path in HEADERS
            if path.is_file()
        }

    def test_every_aja_definition_is_static_or_declared(self):
        """No file-scope aja definition hides from the prototype rule."""
        findings = offending_definitions(self.headers)
        self.assertEqual(
            findings,
            [],
            "these aja plugin definitions are neither `static` nor declared in "
            "a header, so the Flatpak app build fails with "
            "-Werror=missing-prototypes (#2703):\n  " + "\n  ".join(findings),
        )


if __name__ == "__main__":
    unittest.main()
