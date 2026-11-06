#!/usr/bin/env -S uv run --script
"""No workflow or action YAML repeats a key or cuts a scalar short at a " #".

Two PRs each added `rust_libobs` to appimage.yaml's workflow_call inputs (one
at the top, one at the bottom); git merged both without a conflict, and
GitHub then refused every release dispatch: "'rust_libobs' is already
defined". Each PR passed on its own, so only a check of the merged file
catches it. This scans block-style YAML by indentation (stdlib only).

An unquoted value ends at " #": YAML reads the rest as a comment. A run: step
with "for #3168" in its message reached the shell as an unterminated double
quote, and failed only when that step ran. So a plain run: value may hold no
" #" at all, and no other plain value may be followed by " #<digit>", which is
an issue reference that lost its text and not a comment. Quote the value, or
use a block scalar. A real "# v1.2.3" comment after a uses: pin stays allowed.

    uv run python -m unittest ci/test_workflow_duplicate_keys.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FILES = sorted([*ROOT.glob(".github/workflows/*.y*ml"), *ROOT.glob(".github/actions/*/action.y*ml")])
KEY = re.compile(r"^(?P<indent> *)(?:- +)?(?P<key>[A-Za-z0-9_.-]+|'[^']*'|\"[^\"]*\"):(?:\s|$)")
KEY_VALUE = re.compile(r"^ *(?:- +)*(?P<key>[A-Za-z0-9_.-]+):[ \t]+(?P<value>\S.*)$")
COMMENT = re.compile(r"[ \t]#")
ISSUE_REF = re.compile(r"[ \t]#\d")


def truncated_scalars(text: str) -> list[str]:
    found = []
    block_column = None  # inside a | or > block scalar opened by the key at this column
    run_column = None  # inside a plain run: value, which may continue on deeper lines
    for lineno, line in enumerate(text.splitlines(), 1):
        stripped = line.lstrip(" ")
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(stripped)
        if block_column is not None:
            if indent > block_column:
                continue
            block_column = None
        if run_column is not None:
            if indent > run_column:
                if COMMENT.search(line):
                    found.append(f"line {lineno}: run")
                continue
            run_column = None
        m = KEY_VALUE.match(line)
        if not m:
            continue
        key, value = m.group("key"), m.group("value")
        if value[0] in "|>":
            block_column = m.start("key")
        elif value[0] not in "'\"":
            if key == "run":
                run_column = m.start("key")
            if ISSUE_REF.search(value) or (key == "run" and COMMENT.search(value)):
                found.append(f"line {lineno}: {key}")
    return found


def duplicate_keys(text: str) -> list[str]:  # noqa: C901
    found = []
    # Each open mapping: (indent of its keys, keys seen). A "- " list item
    # starts a new mapping at the indent of its first key.
    stack: list[tuple[int, set[str]]] = []
    block_indent = None  # inside a | or > block scalar: skip deeper lines
    for lineno, line in enumerate(text.splitlines(), 1):
        stripped = line.lstrip(" ")
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(stripped)
        if block_indent is not None:
            if indent > block_indent:
                continue
            block_indent = None
        m = KEY.match(line)
        if not m:
            if stripped.startswith("- "):
                while stack and stack[-1][0] > indent:
                    stack.pop()
            continue
        key_indent = indent + (len(stripped) - len(stripped.lstrip("- ")) if stripped.startswith("- ") else 0)
        while stack and stack[-1][0] > key_indent:
            stack.pop()
        if stripped.startswith("- ") or not stack or stack[-1][0] < key_indent:
            stack.append((key_indent, set()))
        keys = stack[-1][1]
        key = m.group("key").strip("'\"")
        if key in keys:
            found.append(f"line {lineno}: {key}")
        keys.add(key)
        if re.search(r":\s*[|>][-+0-9]*\s*(#.*)?$", line):
            block_indent = indent
    return found


class WorkflowDuplicateKeysTest(unittest.TestCase):
    def test_no_duplicate_keys(self):
        bad = []
        for path in FILES:
            for d in duplicate_keys(path.read_text()):
                bad.append(f"{path.relative_to(ROOT)} {d}")
        self.assertEqual(bad, [])

    def test_detects_a_duplicate_input(self):
        text = "on:\n  workflow_call:\n    inputs:\n      a:\n        type: string\n      a:\n        type: boolean\n"
        self.assertEqual(duplicate_keys(text), ["line 6: a"])

    def test_list_items_are_separate_mappings(self):
        text = "steps:\n  - name: x\n    run: |\n      name: y\n  - name: z\n"
        self.assertEqual(duplicate_keys(text), [])


class WorkflowTruncatedScalarsTest(unittest.TestCase):
    def test_no_truncated_scalars(self):
        bad = []
        for path in FILES:
            for d in truncated_scalars(path.read_text()):
                bad.append(f"{path.relative_to(ROOT)} {d}")
        self.assertEqual(bad, [])

    def test_detects_a_plain_run_cut_at_a_hash(self):
        text = 'steps:\n  - name: x\n    run: echo "see #1 now"\n  - run: echo a # b\n'
        self.assertEqual(truncated_scalars(text), ["line 3: run", "line 4: run"])

    def test_detects_a_hash_on_a_continuation_line(self):
        text = "steps:\n  - run: echo a\n      b # c\n    name: x\n"
        self.assertEqual(truncated_scalars(text), ["line 3: run"])

    def test_detects_an_issue_reference_after_another_key(self):
        text = "inputs:\n  a:\n    description: Fail (always, #3238)\n"
        self.assertEqual(truncated_scalars(text), ["line 3: description"])

    def test_allows_what_survives_parsing(self):
        text = (
            "steps:\n"
            "  - uses: actions/checkout@abc # v6.0.2\n"
            "    with:\n"
            "      ref: ${{ inputs.ref }} # empty outside workflow_call\n"
            "  - run: echo \"a#b\"\n"
            "    name: x\n"
            "  - run: 'echo \"see #1\"'\n"
            "  - run: \"echo see #1\"\n"
            "  - run: |\n"
            "      echo a # b\n"
            "      run: echo c # d\n"
            "  - name: y\n"
        )
        self.assertEqual(truncated_scalars(text), [])


if __name__ == "__main__":
    unittest.main()
