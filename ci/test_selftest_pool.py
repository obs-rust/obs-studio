#!/usr/bin/env -S uv run --script
"""The ci/ self-tests run on the standard pool, not in the 26.04 lint job (#427).

    uv run python -m unittest ci/test_selftest_pool.py

The lint lane spent 27 s of its 70 s on a scarce ubuntu-26.04 runner
running the 373 plain-Python ci/ tests. CI now runs them in their own
ubuntu-latest job alongside, and the 26.04 lint step skips them
(OBS_GATE_SKIP_SELF_TEST=1). ./lint still runs them locally.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"
sys.path.insert(0, str(ROOT / "ci"))


def job(name: str) -> str:
    text = WORKFLOW.read_text(encoding="utf-8")
    m = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M)
    assert m, f"no job {name}"
    return m.group(1)


class WorkflowTest(unittest.TestCase):
    def test_self_tests_have_their_own_standard_pool_job(self):
        fmt = (ROOT / ".github" / "workflows" / "check-format.yaml").read_text(encoding="utf-8")
        j = fmt[fmt.index("  checks:"):fmt.index("  swift-format:")]
        self.assertIn("runs-on: ubuntu-latest", j)
        self.assertIn('unittest discover -s ci -p "test_*.py"', j)

    def test_lint_step_skips_them_in_ci(self):
        j = job("rust-tests-linux")
        step = j[j.index("Local Gate (lint)"):]
        step = step[: step.index("\n\n")]
        self.assertIn('OBS_GATE_SKIP_SELF_TEST: "1"', step)


def gate_soldr_tests() -> str:
    import importlib

    return importlib.import_module("local_gate").SOLDR_SELF_TESTS


class GateTest(unittest.TestCase):
    def test_env_var_runs_only_the_soldr_tests(self):
        import importlib

        gate = importlib.import_module("local_gate")
        with mock.patch.object(gate, "run") as run, mock.patch.dict(os.environ, {"OBS_GATE_SKIP_SELF_TEST": "1"}):
            gate.self_test()
        run.assert_called_once()
        self.assertEqual(run.call_args.args[0][-1], gate.SOLDR_SELF_TESTS)

    def test_soldr_tests_skip_without_soldr(self):
        text = (ROOT / "ci" / gate_soldr_tests()).read_text(encoding="utf-8")
        self.assertIn('@unittest.skipUnless(shutil.which("soldr")', text)

    def test_self_test_runs_by_default(self):
        import importlib

        gate = importlib.import_module("local_gate")
        env = {k: v for k, v in os.environ.items() if k != "OBS_GATE_SKIP_SELF_TEST"}
        with mock.patch.object(gate, "run") as run, mock.patch.dict(os.environ, env, clear=True):
            gate.self_test()
        run.assert_called_once()


if __name__ == "__main__":
    unittest.main()
