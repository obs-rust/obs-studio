#!/usr/bin/env -S uv run --script
"""Every workflow step that runs apt-get install refreshes the package lists first.

GitHub-hosted Ubuntu images ship populated apt lists, so an `apt-get install`
with no `apt-get update` works there. The self-hosted Dell pool images ship
none: the same install fails with "Unable to locate package". The Windows Rust
tests under Wine (#5850) installed Wine that way and went red on every
same-repo PR once #5852 routed heavy jobs to the pool (#5859).

So a run: step that installs must also update, before the install, in the
same step (a container step that installs inside `docker run` counts too).
A step whose lists are known to be fresh already (a prebaked image, or a
container where an earlier step updated) says so with a comment holding
`apt-lists-ready:` and the reason.

    uv run python -m unittest ci/test_workflow_apt_update.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILES = sorted([*ROOT.glob(".github/workflows/*.y*ml"), *ROOT.glob(".github/actions/*/action.y*ml")])
RUN = re.compile(r"^(?P<indent> *)(?:- +)?run:[ \t]*(?P<rest>.*)$")
INSTALL = re.compile(r"\bapt-get\b[^\n;&|]*?\binstall\b")
UPDATE = re.compile(r"\bapt-get\b[^\n;&|]*?\bupdate\b")
READY = "apt-lists-ready:"


def run_blocks(text: str) -> list[tuple[int, str]]:
    """(line number, script) for each run: value, block scalar or plain."""
    lines = text.splitlines()
    blocks = []
    i = 0
    while i < len(lines):
        match = RUN.match(lines[i])
        if not match:
            i += 1
            continue
        start = i + 1
        rest = match["rest"].strip()
        if rest and rest[0] not in "|>":
            blocks.append((start, rest))
            i += 1
            continue
        indent = len(match["indent"])
        body = []
        i += 1
        while i < len(lines) and (not lines[i].strip() or len(lines[i]) - len(lines[i].lstrip(" ")) > indent):
            body.append(lines[i])
            i += 1
        blocks.append((start, "\n".join(body)))
    return blocks


def stale_installs(text: str) -> list[int]:
    """Line numbers of run: steps that install before (or without) an update."""
    bad = []
    for line, script in run_blocks(text):
        install = INSTALL.search(script)
        if not install or READY in script:
            continue
        update = UPDATE.search(script)
        if update is None or update.start() > install.start():
            bad.append(line)
    return bad


class AptUpdateTest(unittest.TestCase):
    def test_every_install_step_updates_first(self):
        failures = []
        for path in FILES:
            for line in stale_installs(path.read_text()):
                failures.append(f"{path.relative_to(ROOT)}:{line}")
        self.assertEqual(failures, [], "apt-get install with no apt-get update before it in the same step")

    def test_the_check_sees_a_bare_install(self):
        text = "      - name: Install Wine\n        run: |\n          command -v wine || sudo apt-get install -y wine\n"
        self.assertEqual(stale_installs(text), [2])

    def test_an_update_after_the_install_does_not_count(self):
        text = "    - run: |\n        sudo apt-get install -y x\n        sudo apt-get update\n"
        self.assertEqual(stale_installs(text), [1])

    def test_an_update_first_passes(self):
        text = (
            "    - run: |\n"
            "        sudo apt-get -o Acquire::Retries=1 update\n"
            "        sudo apt-get -o Acquire::Retries=1 install -y x\n"
            "    - run: sudo apt-get update -qq && sudo apt-get install -y zstd\n"
        )
        self.assertEqual(stale_installs(text), [])

    def test_a_marked_step_is_exempt(self):
        text = "    - run: |\n        # apt-lists-ready: prebaked image\n        apt-get install -y git\n"
        self.assertEqual(stale_installs(text), [])

    def test_a_plain_run_value_is_checked(self):
        self.assertEqual(stale_installs("    - run: sudo apt-get install -y zstd\n"), [1])


if __name__ == "__main__":
    unittest.main()
