#!/usr/bin/env -S uv run --script
"""The sanitizer workflow stays off PR and push CI (#491).

    uv run python -m unittest ci/test_sanitizers_schedule.py

ASan, TSan and valgrind over the cmocka tests and a headless launch take
about 20 minutes. Per zackees/ci.yml GATE-012 they run nightly (schedule)
and on demand (workflow_dispatch): a PR that changes the harness dispatches
the workflow on its own branch. A master push only calls it build-only, to
refresh the compiler archive (#3501); that is a build, not a scan.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "sanitizers.yaml"


def triggers() -> set[str]:
    text = WORKFLOW.read_text()
    block = re.search(r"^on:\n((?:[ \t]+.*\n|\n)*)", text, re.M)
    assert block, "sanitizers.yaml has no on: block"
    return set(re.findall(r"^  ([a-z_]+):", block.group(1), re.M))


class SanitizersScheduleTest(unittest.TestCase):
    def test_not_on_pull_requests_or_pushes(self):
        self.assertFalse(triggers() & {"pull_request", "pull_request_target", "push", "merge_group"})

    def test_nightly_and_on_demand(self):
        self.assertTrue({"schedule", "workflow_dispatch"} <= triggers())


if __name__ == "__main__":
    unittest.main()
