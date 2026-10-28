#!/usr/bin/env -S uv run --script
"""A sparse checkout must keep the local composite actions its job later calls.

    uv run python -m unittest ci/test_sparse_checkout_local_actions.py

`actions/checkout` with `sparse-checkout` rewrites the working tree to the
listed paths, so a second checkout inside a job deletes everything outside the
cone -- including `./.github/actions/*`. A step that then runs a local composite
action fails before it starts:

    ##[error]Can't find 'action.yml', 'action.yaml' or 'Dockerfile' under
    '.../.github/actions/cache'. Did you forget to run actions/checkout ...

That is a runtime failure, invisible to every offline lane, and on
`rust-tier2-linux` it only exists on master pushes (`if:` master), so PR CI is
green while master's Tier 2 lane is red (#2764). The rule here: when a job uses
a local action after a checkout, the nearest preceding checkout must not be a
sparse one that omits that action's directory.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"
JOB = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")
CHECKOUT = re.compile(r"^\s*(?:-\s*)?uses:\s+actions/checkout@")
LOCAL_ACTION = re.compile(r"^\s*(?:-\s*)?uses:\s+\./(\.github/actions/[A-Za-z0-9_-]+)\s*$")
SPARSE = re.compile(r"^(\s*)sparse-checkout:\s*(\S*)\s*$")


def job_blocks(text: str) -> list[tuple[str, list[str]]]:
    """Split a workflow into ``(job name, job lines)`` pairs.

    Jobs are the two-space keys under ``jobs:``; the reusable-workflow jobs of
    the other files use the same indent. Anything before the first job key is
    dropped, so ``on:``/``env:`` blocks cannot be mistaken for jobs.
    """
    blocks: list[tuple[str, list[str]]] = []
    name = ""
    lines: list[str] = []
    for line in text.splitlines():
        m = JOB.match(line)
        if m:
            if name:
                blocks.append((name, lines))
            name, lines = m.group(1), []
        elif name:
            lines.append(line)
    if name:
        blocks.append((name, lines))
    return blocks


def sparse_paths(lines: list[str], index: int, indent: int, inline: str) -> list[str]:
    """The paths of the sparse checkout whose `sparse-checkout:` is at `index`."""
    if inline and inline not in {"|", "|-", ">", ">-"}:
        return [inline]
    paths: list[str] = []
    for line in lines[index + 1 :]:
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if len(line) - len(line.lstrip()) <= indent:
            break
        paths.append(line.strip())
    return paths


def checkouts(lines: list[str]) -> list[tuple[int, list[str] | None]]:
    """`(line index, sparse paths or None for a full checkout)` per checkout."""
    found: list[tuple[int, list[str] | None]] = []
    for index, line in enumerate(lines):
        if not CHECKOUT.match(line):
            continue
        paths: list[str] | None = None
        for offset in range(index + 1, min(index + 12, len(lines))):
            m = SPARSE.match(lines[offset])
            if m:
                paths = sparse_paths(lines, offset, len(m.group(1)), m.group(2))
                break
        found.append((index, paths))
    return found


def covered(action: str, paths: list[str]) -> bool:
    return any(action == p or action.startswith(p.rstrip("/") + "/") for p in paths)


def violations() -> list[str]:
    out: list[str] = []
    for workflow in sorted(WORKFLOWS.glob("*.y*ml")):
        for job, lines in job_blocks(workflow.read_text(encoding="utf-8")):
            found = checkouts(lines)
            for index, line in enumerate(lines):
                m = LOCAL_ACTION.match(line)
                if not m:
                    continue
                before = [c for c in found if c[0] < index]
                if not before:
                    continue
                _, paths = before[-1]
                if paths is not None and not covered(m.group(1), paths):
                    out.append(
                        f"{workflow.name}:{job}: uses ./{m.group(1)} after a sparse "
                        f"checkout of {paths!r}, which drops it"
                    )
    return out


class SparseCheckoutLocalActionsTest(unittest.TestCase):
    def test_sparse_checkouts_keep_local_actions(self) -> None:
        problems = violations()
        self.assertEqual(
            problems,
            [],
            "a sparse checkout removes a local composite action the job calls "
            "afterwards; add its directory to `sparse-checkout`:\n"
            + "\n".join(problems),
        )

    def test_the_checkout_parser_sees_a_sparse_and_a_full_checkout(self) -> None:
        # Guard against a parser that silently finds nothing: build-project's
        # `rust-linux-tier2-done` makes a second, sparse checkout of the repo.
        text = (WORKFLOWS / "build-project.yaml").read_text(encoding="utf-8")
        blocks = dict(job_blocks(text))
        self.assertTrue("rust-linux-tier2-done" in blocks, "job not parsed")
        paths = [
            p for _, p in checkouts(blocks["rust-linux-tier2-done"]) if p is not None
        ]
        self.assertTrue(paths, "no sparse checkout parsed in rust-linux-tier2-done")


if __name__ == "__main__":
    unittest.main()
