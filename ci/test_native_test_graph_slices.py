#!/usr/bin/env -S uv run --script
"""The native test graph consumer queries each entry through a small slice.

    uv run python -m unittest ci/test_native_test_graph_slices.py

Every CMake string(JSON) call reparses the whole document it is given.
test/cmocka/native-test-graph.cmake queried the full 170 KB graph about
35 000 times (all 304 targets and 304 tests, every field), which made
configure take 51 s of the Tier 2 Linux build's 63 s configure. Inside the
per-target and per-test loops the full "${graph}" may only be read to name
an entry and cut its slice out (MEMBER, TYPE and GET of targets/tests
"${name}"); every other query goes to that slice.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

GRAPH = Path(__file__).resolve().parents[1] / "test" / "cmocka" / "native-test-graph.cmake"

# The only full-graph reads allowed per loop iteration.
ALLOWED = re.compile(
    r'string\(JSON \w+ (MEMBER "\$\{graph\}" (targets|tests) \$\{index\}'
    r'|(TYPE|GET) "\$\{graph\}" (targets|tests) "\$\{name\}")\)'
)


def loop_body(text: str) -> list[str]:
    """Lines of obs_native_test_graph from its first per-entry loop on."""
    start = text.index("function(obs_native_test_graph ")
    body = text[start:].splitlines()
    first = next(i for i, line in enumerate(body) if "foreach(index RANGE" in line)
    return body[first:]


class NativeTestGraphSlices(unittest.TestCase):
    def test_loops_query_only_the_entry_slice(self) -> None:
        offenders = [
            line.strip()
            for line in loop_body(GRAPH.read_text())
            if '"${graph}"' in line and not ALLOWED.search(line)
            # The tests list itself is counted once, between the loops.
            and line.strip() != 'string(JSON count LENGTH "${graph}" tests)'
        ]
        self.assertEqual(
            offenders,
            [],
            "query the per-entry slice, not the full graph, inside the loops",
        )


if __name__ == "__main__":
    unittest.main()
