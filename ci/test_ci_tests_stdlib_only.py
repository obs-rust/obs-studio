#!/usr/bin/env -S uv run --script
"""Every ci/test_*.py runs on a plain Python: standard library and ci/
modules only.

    uv run python -m unittest ci/test_ci_tests_stdlib_only.py

The local gate runs each file with `sys.executable -m unittest`
(ci/local_gate.py) and CI's "ci/ Self-Tests" step with
`uv run python -m unittest discover` (check-format.yaml). Neither installs
packages, and neither reads the dependencies in inline script metadata
(`# /// script`), which apply only to `uv run --script <file>`. A
third-party import fails on any host whose Python lacks the package
(#1196: PyYAML).
"""

from __future__ import annotations

import ast
import re
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI = ROOT / "ci"
# A non-empty `dependencies` list in inline script metadata.
INLINE_DEPENDENCIES = re.compile(r"^#\s*dependencies\s*=\s*\[\s*[^\]\s]", re.M)


def allowed_modules() -> set[str]:
    return set(sys.stdlib_module_names) | {"__future__"} | {p.stem for p in CI.glob("*.py")}


def problems(source: str, allowed: set[str]) -> list[str]:
    found = []
    if INLINE_DEPENDENCIES.search(source):
        found.append("inline script dependencies that the test runners ignore")
    for node in ast.walk(ast.parse(source)):
        if isinstance(node, ast.Import):
            names = [a.name for a in node.names]
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            names = [node.module]
        else:
            continue
        for name in names:
            top = name.split(".")[0]
            if top not in allowed:
                found.append(f"imports {top}")
    return found


class CiTestsStdlibOnlyTest(unittest.TestCase):
    maxDiff = None

    def test_every_ci_test_uses_only_the_standard_library(self):
        allowed = allowed_modules()
        bad = {}
        for path in sorted(CI.glob("test_*.py")):
            found = problems(path.read_text(encoding="utf-8"), allowed)
            if found:
                bad[path.name] = found
        self.assertEqual(bad, {})

    def test_check_finds_each_problem(self):
        allowed = {"re", "unittest", "local_gate"}
        self.assertEqual(problems("import re\nimport unittest\nimport local_gate\n", allowed), [])
        self.assertEqual(problems("import yaml\n", allowed), ["imports yaml"])
        self.assertEqual(problems("from yaml import safe_load\n", allowed), ["imports yaml"])
        self.assertEqual(problems("import re\n", {"re"}), [])
        self.assertEqual(
            problems('# /// script\n# dependencies = ["x"]\n# ///\n', allowed),
            ["inline script dependencies that the test runners ignore"],
        )
        # Metadata without dependencies (test_no_dict_access.py) is harmless.
        self.assertEqual(problems('# /// script\n# requires-python = ">=3.11"\n# ///\n', allowed), [])
        self.assertEqual(problems("# /// script\n# dependencies = []\n# ///\n", allowed), [])


if __name__ == "__main__":
    unittest.main()
