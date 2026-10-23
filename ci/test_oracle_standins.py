#!/usr/bin/env -S uv run --script
"""The C oracle's stand-ins for libobs functions match libobs (#145).

    uv run python -m unittest ci/test_oracle_standins.py

Tier 3 parity is only as good as the oracle. The remaining *_host.c files
provide platform-specific oracle link helpers, and when one re-implements a
libobs function it can drift and certify the difference (#77, #79, #84).
Every function defined there that libobs also defines must be identical to
one libobs definition (comments and whitespace ignored), or be listed in
DIFFERENT with the reason it may differ.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ORACLE = ROOT / "rust" / "obs-c-oracle" / "oracle"
STANDINS = tuple(sorted(p.name for p in ORACLE.glob("*_host.c")))
DEFINITION = re.compile(
    r"^(?:static\s+)?(?:inline\s+)?[A-Za-z_][\w \t*]*?\b([a-z_][a-z0-9_]*)\s*\([^;{}]*\)\s*\{", re.M
)

# Functions that may differ from libobs, with the reason. The Windows
# forwarders of the string conversions went when obs-util ported them (#262
# slice 1): the plain names are the Rust shims, and oracle TUs call the
# oracle_ copies from oracle/platform.c.
DIFFERENT: dict[str, str] = {}


def functions(text: str) -> dict[str, list[str]]:
    found: dict[str, list[str]] = {}
    for match in DEFINITION.finditer(text):
        depth = 0
        for end in range(match.end() - 1, len(text)):
            if text[end] == "{":
                depth += 1
            elif text[end] == "}":
                depth -= 1
                if depth == 0:
                    break
        found.setdefault(match.group(1), []).append(text[match.start() : end + 1])
    return found


def normalized(code: str) -> str:
    code = re.sub(r"/\*.*?\*/|//[^\n]*", "", code, flags=re.S)
    return re.sub(r"\s+", " ", code).strip()


def libobs_functions() -> dict[str, set[str]]:
    found: dict[str, set[str]] = {}
    for path in sorted((ROOT / "libobs").rglob("*.c")):
        for name, bodies in functions(path.read_text(errors="ignore")).items():
            found.setdefault(name, set()).update(normalized(b) for b in bodies)
    return found


class OracleStandinTest(unittest.TestCase):
    def test_standins_match_libobs(self):
        libobs = libobs_functions()
        drifted = []
        for file in STANDINS:
            for name, bodies in functions((ORACLE / file).read_text()).items():
                if name not in libobs or name in DIFFERENT:
                    continue
                for body in bodies:
                    if normalized(body) not in libobs[name]:
                        drifted.append(f"{file}: {name}")
        self.assertEqual(drifted, [], "copy libobs exactly, or list the function in DIFFERENT with a reason")

    def test_different_entries_are_still_standins(self):
        defined = set()
        for file in STANDINS:
            defined.update(functions((ORACLE / file).read_text()))
        self.assertEqual(sorted(set(DIFFERENT) - defined), [], "remove stale DIFFERENT entries")


if __name__ == "__main__":
    unittest.main()
