#!/usr/bin/env -S uv run --script
"""The linux-jack unit declares its file-scope process callback static (#2703).

    uv run python -m unittest ci/test_linux_jack_missing_prototypes.py

#2678 put `-Wmissing-prototypes` in the global C option lists. The PR lanes never
build the JACK plugin (`ENABLE_JACK` is on in the Flatpak manifest only), so a
non-static file-scope definition without a prototype there first surfaces in a
`Scheduled`/release dispatch as `-Werror=missing-prototypes`. On master the
Flatpak app build stops, after the aja unit is fixed, on

    plugins/linux-jack/jack-wrapper.c.h:56: error: no previous prototype for
    'jack_process_callback' [-Werror=missing-prototypes]

`jack_process_callback` is the `jack_set_process_callback` argument used only by
`jack_init` in the same translation unit, so it is file-scope private and the fix
is `static` — the shape #2710 used for the OpenGL platform sources. The plugin's
public entry points keep the prototypes `jack-wrapper.h` declares. This test
keeps the class closed offline: every file-scope definition in the unit must be
`static` or declared in `jack-wrapper.h`.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PLUGIN = ROOT / "plugins" / "linux-jack"
HEADERS = (
    PLUGIN / "jack-wrapper.h",
    ROOT / "libobs" / "obs-module.h",
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
    for source in sorted(PLUGIN.glob("*.c.h")):
        lines = source.read_text(encoding="utf-8", errors="replace").splitlines()
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
            findings.append(f"{source.relative_to(ROOT)}:{number}: {name}")
    return findings


class LinuxJackMissingPrototypesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.headers = {
            path: path.read_text(encoding="utf-8", errors="replace")
            for path in HEADERS
            if path.is_file()
        }

    def test_every_linux_jack_definition_is_static_or_declared(self):
        """No file-scope linux-jack definition hides from the prototype rule."""
        findings = offending_definitions(self.headers)
        self.assertEqual(
            findings,
            [],
            "these linux-jack plugin definitions are neither `static` nor "
            "declared in a header, so the Flatpak app build fails with "
            "-Werror=missing-prototypes (#2703):\n  " + "\n  ".join(findings),
        )


if __name__ == "__main__":
    unittest.main()
