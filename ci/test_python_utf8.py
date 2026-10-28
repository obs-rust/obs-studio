#!/usr/bin/env -S uv run --script
"""Every workflow that runs a ci/ module forces UTF-8 for its Python.

    uv run python -m unittest ci/test_python_utf8.py

A Windows runner's Python decodes with the ANSI code page (cp1252) unless the
environment says otherwise, so `Path.read_text()` in a ci/test_*.py raises
UnicodeDecodeError on any tracked file holding UTF-8 bytes -- the workflow files
do, because their job names carry emoji. The suite is written for a POSIX host,
where the locale encoding is UTF-8, and `ci/local_gate.py` spawns each test as
`sys.executable -m unittest`, so one `PYTHONUTF8: "1"` on the workflow covers
the gate and every child it starts (#2757, PR #2743 run 40659332451: 128 of
273 files failed that way).

`PYTHONUTF8=1` is a no-op on Linux and macOS, where UTF-8 is already the
default (PEP 540).
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
# A workflow-level env entry: exactly two spaces of indentation, under `env:`.
WORKFLOW_ENV = re.compile(r"^  PYTHONUTF8: ['\"]?1['\"]?\s*$", re.M)


def runs_a_ci_module(text: str) -> bool:
    """The workflow runs the local gate, which runs the ci/ self-tests."""
    return bool(re.search(r"^ *run:.*\bci/local_gate\.py\b", text, re.M))


def offenders() -> list[str]:
    found = []
    for path in sorted(WORKFLOWS.glob("*.yaml")):
        text = path.read_text(encoding="utf-8")
        if not runs_a_ci_module(text):
            continue
        if WORKFLOW_ENV.search(text):
            continue
        found.append(path.name)
    return found


class PythonUtf8Test(unittest.TestCase):
    def test_workflows_running_a_ci_module_force_utf8(self) -> None:
        self.assertEqual(
            offenders(),
            [],
            'add PYTHONUTF8: "1" to the workflow-level env: of each workflow that '
            "runs the local gate; the Windows runner decodes cp1252 otherwise (#2757)",
        )


if __name__ == "__main__":
    unittest.main()
