#!/usr/bin/env -S uv run --script
"""validate.sh runs a chosen set of modes: which libobs the tests load (#930 A2, #1065).

The CI layout this introduced (one job per mode) is superseded by #937's
build-once, shard-the-tests jobs, pinned in ci/test_tier2_fanout.py.

    uv run python -m unittest ci/test_tier2_split.py

validate.sh built and tested OFF, then ON, in one job: about 13 min, nearly
all of it two back-to-back ctest runs. Each mode now runs in its own matrix
job; a small job compares the two exported-symbol lists and publishes the
lane proof once both pass. The local gate still runs both modes in one go.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
RUN = ROOT / "rust" / "tools" / "linux-validate" / "run.py"


def _unused_job(text: str, name: str) -> str:
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    if not m:
        raise AssertionError(f"no job {name}")
    return m.group(1)


class HarnessTest(unittest.TestCase):
    def test_validate_runs_the_requested_modes(self):
        text = VALIDATE.read_text(encoding="utf-8")
        self.assertIn('modes="${OBS_TIER2_MODES:-OFF ON}"', text)
        self.assertIn("for mode in $modes; do", text)

    def test_validate_writes_exports_for_the_host(self):
        self.assertIn("/exports", VALIDATE.read_text(encoding="utf-8"))

    def test_run_passes_modes_and_exports_dir(self):
        text = RUN.read_text(encoding="utf-8")
        self.assertIn("OBS_TIER2_MODES", text)
        self.assertIn("OBS_TIER2_EXPORTS_DIR", text)


if __name__ == "__main__":
    unittest.main()
