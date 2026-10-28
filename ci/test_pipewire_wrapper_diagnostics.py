#!/usr/bin/env -S uv run --script
"""Pins the PipeWire wrapper harness's own diagnostics (obs-rust/obs-studio#2772).

`test_pipewire_wrapper` (Tier 2, ctest #161) failed once on master after the
control case ran into its 60 s cap, and the log said only `exit 124, expected
77`: the harness wrote the child's output to a temp file and dropped it. These
tests use stubs, not pipewire, and assert the three properties that failure
needed:

  - a case that runs into the cap reports the timeout *and* its captured
    output, so the next occurrence is diagnosable from the CI log alone;
  - a wrong exit status shows the child's output too;
  - every case runs uv with a pinned interpreter, so a stripped `PATH` cannot
    send uv off to discover or download one inside a case's budget.

Run: uv run python -m unittest ci/test_pipewire_wrapper_diagnostics.py
"""

import contextlib
import importlib.util
import io
import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HARNESS = ROOT / "test" / "cmocka" / "test_pipewire_wrapper.py"

spec = importlib.util.spec_from_file_location("pipewire_wrapper_harness", HARNESS)
if spec is None or spec.loader is None:  # pragma: no cover - the file is in the tree
    raise SystemExit(f"cannot load {HARNESS}")
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


def write_script(path: Path, body: str) -> Path:
    path.write_text(body)
    path.chmod(0o755)
    return path


class Diagnostics(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory(prefix="obs-pw-diag-test.")
        self.addCleanup(self.tmp.cleanup)
        self.work = Path(self.tmp.name)
        self.stub = write_script(self.work / "stub", "#!/bin/sh\nexit 3\n")
        # Stands in for uv: `uv run --script <wrapper> <stub>` runs the wrapper.
        self.uv = write_script(self.work / "uv", '#!/bin/sh\nshift 2\nexec "$@"\n')
        self.out = self.work / "out"

    def run_case(self, wrapper: Path, env: dict[str, str], timeout: float) -> object:
        previous = harness.TIMEOUT_SECONDS
        harness.TIMEOUT_SECONDS = timeout
        try:
            return harness.run_wrapper(str(self.uv), wrapper, self.stub, self.out, env)
        finally:
            harness.TIMEOUT_SECONDS = previous

    def report_for(self, name: str, want: int, got: object) -> str:
        report = harness.Report()
        stream = io.StringIO()
        with contextlib.redirect_stdout(stream):
            report.check(name, want, got)
        self.assertEqual(report.failed, 1, "a mismatch must be counted as a failure")
        return stream.getvalue()

    def test_a_case_that_hits_the_cap_reports_the_timeout_and_its_output(self) -> None:
        wrapper = write_script(
            self.work / "slow.py",
            "#!/bin/sh\necho 'pipewire: waiting for a socket'\nsleep 5\nexit 77\n",
        )
        got = self.run_case(wrapper, dict(os.environ), timeout=0.3)

        self.assertEqual(got.exit_code, 124)
        text = self.report_for("slow.py, no pipewire binary: skips", 77, got)
        self.assertIn("timed out", text)
        self.assertIn("pipewire: waiting for a socket", text)

    def test_a_wrong_status_reports_the_child_output(self) -> None:
        wrapper = write_script(
            self.work / "wrong.py",
            "#!/bin/sh\necho 'env: no pipewire here'\nexit 3\n",
        )
        got = self.run_case(wrapper, dict(os.environ), timeout=5)

        self.assertEqual(got.exit_code, 3)
        text = self.report_for("wrong.py, no pipewire binary: skips", 77, got)
        self.assertIn("exit status 3", text)
        self.assertIn("env: no pipewire here", text)

    def test_a_matching_status_stays_quiet(self) -> None:
        wrapper = write_script(self.work / "skip.py", "#!/bin/sh\necho skipping\nexit 77\n")
        got = self.run_case(wrapper, dict(os.environ), timeout=5)
        report = harness.Report()
        stream = io.StringIO()
        with contextlib.redirect_stdout(stream):
            report.check("skip.py, no pipewire binary: skips", 77, got)

        self.assertEqual(report.failed, 0)
        self.assertIn("ok:", stream.getvalue())
        self.assertNotIn("skipping", stream.getvalue())

    def test_every_case_pins_the_interpreter(self) -> None:
        env = harness.wrapper_env(dict(os.environ))

        self.assertEqual(env["UV_PYTHON"], sys.executable)
        self.assertEqual(env["UV_PYTHON_DOWNLOADS"], "never")


if __name__ == "__main__":
    unittest.main()
