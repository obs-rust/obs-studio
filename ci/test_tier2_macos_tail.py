#!/usr/bin/env -S uv run --script
"""macOS Tier 2 overlaps preparation, preserves every result and joins children.

    uv run python -m unittest ci/test_tier2_macos_tail.py
"""

from __future__ import annotations

import contextlib
import io
import tempfile
import threading
import unittest
from pathlib import Path
from unittest import mock

from ci import tier2_macos_tail as tail


class MacosTailTest(unittest.TestCase):
    def run_fake(self, root: Path, failing: str = "") -> tuple[int, str]:
        validator = root / "validate.sh"
        validator.write_text(
            '#!/bin/bash\nset -eu\nphase="$2"\n'
            'printf "%s\\n" "$@" > "$0.$phase.args"\n'
            'echo "stdout $phase"\necho "stderr $phase" >&2\n'
            'touch "$0.$phase.done"\n'
            f'if [ "$phase" = "{failing}" ]; then exit 23; fi\n',
            encoding="utf-8",
        )
        output = io.StringIO()
        with contextlib.redirect_stdout(output):
            status = tail.run_tail(validator, ["--repeat", "2", "--tests", "with spaces"], root / "logs")
        return status, output.getvalue()

    def test_success_keeps_logs_and_forwards_arguments(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            status, output = self.run_fake(root)
            self.assertEqual(status, 0)
            for phase in ("cmake-tests", "cargo-build", "cargo-tests"):
                self.assertTrue((root / f"validate.sh.{phase}.done").is_file())
                self.assertEqual(
                    (root / f"validate.sh.{phase}.args").read_text().splitlines(),
                    ["--phase", phase, "--repeat", "2", "--tests", "with spaces"],
                )
                self.assertIn(f"stdout {phase}", output)
                self.assertIn(f"stderr {phase}", (root / "logs" / f"{phase}.log").read_text())

    def test_any_failure_fails_without_abandoning_independent_work(self):
        for failing in ("cmake-tests", "cargo-build", "cargo-tests"):
            with self.subTest(failing=failing), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                status, output = self.run_fake(root, failing)
                self.assertNotEqual(status, 0)
                self.assertIn(f"{failing}: exit 23", output)
                for independent in ("cmake-tests", "cargo-build"):
                    self.assertTrue((root / f"validate.sh.{independent}.done").is_file())
                # Native suites require successful preparation; a failed CMake
                # suite must still allow them to run and report their result.
                self.assertEqual((root / "validate.sh.cargo-tests.done").exists(), failing != "cargo-build")

    def test_overlap_and_barrier_before_native_suites(self):
        barrier = threading.Barrier(2, timeout=5)
        finished: set[str] = set()

        def phase(name, command, log_dir):
            if name == "cargo-tests":
                self.assertEqual(finished, {"cmake-tests", "cargo-build"})
            else:
                barrier.wait()
                finished.add(name)
            return 0

        with tempfile.TemporaryDirectory() as directory, mock.patch.object(tail, "run_phase", side_effect=phase):
            with mock.patch.object(tail, "show_log"):
                self.assertEqual(tail.run_tail(Path("validate.sh"), [], Path(directory)), 0)

    def test_launch_error_still_runs_other_independent_phase(self):
        def launch(command, **kwargs):
            if "cargo-build" in command:
                raise OSError("launch failed")
            return mock.Mock(returncode=0)

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with mock.patch.object(tail.subprocess, "run", side_effect=launch) as run:
                with contextlib.redirect_stdout(io.StringIO()) as output:
                    self.assertNotEqual(tail.run_tail(root / "validate.sh", [], root), 0)
                self.assertEqual(run.call_count, 2)
            self.assertIn("launch failed", output.getvalue())


if __name__ == "__main__":
    unittest.main()
