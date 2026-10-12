#!/usr/bin/env python3
"""A job that calls a reusable workflow must grant every permission the
called workflow's jobs ask for.

    python3 -m unittest ci/test_reusable_permissions.py

GitHub checks this when it loads the caller, before any `if:` runs, and
refuses to start the whole workflow (startup_failure, no jobs) when a called
job asks for more. release-rust.yaml called appimage.yaml with
contents: read after appimage.yaml gained a release job with
contents: write, and no daily beta was published (#410).
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
LEVEL = {"none": 0, "read": 1, "write": 2}
NAME = {v: k for k, v in LEVEL.items()}


def jobs(text: str) -> dict[str, list[str]]:
    """Job name -> its lines, for the top-level `jobs:` map."""
    out: dict[str, list[str]] = {}
    in_jobs = False
    current = None
    for line in text.splitlines():
        if re.match(r"^jobs:\s*$", line):
            in_jobs = True
            continue
        if in_jobs and re.match(r"^\S", line):
            in_jobs = False
        if not in_jobs:
            continue
        m = re.match(r"^  ([A-Za-z0-9_-]+):\s*$", line)
        if m:
            current = m.group(1)
            out[current] = []
        elif current is not None:
            out[current].append(line)
    return out


def permissions(lines: list[str], indent: int) -> dict[str, int] | None:
    """The `permissions:` map at this indent, or None if there is none."""
    for i, line in enumerate(lines):
        if re.match(rf"^ {{{indent}}}permissions:\s*$", line):
            perms = {}
            for sub in lines[i + 1 :]:
                m = re.match(rf"^ {{{indent + 2}}}([a-z-]+):\s*(\w+)", sub)
                if not m:
                    break
                perms[m.group(1)] = LEVEL[m.group(2)]
            return perms
    return None


def top_permissions(text: str) -> dict[str, int] | None:
    return permissions(text.splitlines(), 0)


class ReusablePermissionsTest(unittest.TestCase):
    def test_callers_grant_what_called_jobs_ask_for(self):
        problems = []
        for caller in sorted(WORKFLOWS.glob("*.y*ml")):
            for name, lines in jobs(caller.read_text()).items():
                uses = next(
                    (m.group(1) for line in lines if (m := re.match(r"^    uses: \./\.github/workflows/(\S+)", line))),
                    None,
                )
                if uses is None:
                    continue
                granted = permissions(lines, 4)
                if granted is None:
                    continue  # inherits the caller's top-level permissions
                callee = (WORKFLOWS / uses).read_text()
                asked = [top_permissions(callee) or {}]
                asked += [p for p in (permissions(l, 4) for l in jobs(callee).values()) if p]
                for perms in asked:
                    for scope, level in perms.items():
                        if granted.get(scope, 0) < level:
                            problems.append(
                                f"{caller.name} job {name} grants {scope}: "
                                f"{NAME[granted.get(scope, 0)]} but {uses} asks for {NAME[level]}"
                            )
        self.assertEqual(sorted(set(problems)), [])


if __name__ == "__main__":
    unittest.main()
