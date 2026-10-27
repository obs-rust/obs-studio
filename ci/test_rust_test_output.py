#!/usr/bin/env -S uv run --script
"""A workflow's `cargo test` step captures output instead of passing --nocapture (#962).

    uv run python -m unittest ci/test_rust_test_output.py

libtest runs tests in parallel only while their output is captured; --nocapture
serializes the run (ci-lint RUST-017). A step that needs a test's stdout, for a
later `grep` or a saved log, passes --show-output: it prints the output of
successful tests after the run and keeps the parallel schedule.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"

_CARGO_TEST = re.compile(r"\bcargo\s+test\b")
_NOCAPTURE = "--nocapture"


def find_nocapture(path: str, text: str) -> list[str]:
    """`cargo test` command lines that pass --nocapture."""
    hits = []
    for n, line in enumerate(text.splitlines(), 1):
        if line.lstrip().startswith("#"):
            continue
        if _CARGO_TEST.search(line) and _NOCAPTURE in line.split():
            hits.append(f"{path}:{n}: {line.strip()}")
    return hits


class RustTestOutputTest(unittest.TestCase):
    def test_detector(self):
        bad = "run: soldr cargo test -p obs-studio-rust --test obs-tests-app -- --exact x --nocapture"
        self.assertEqual(len(find_nocapture("w.yaml", bad)), 1)
        for line in (
            "run: soldr cargo test -p obs-studio-rust --test obs-tests-app -- --exact x --show-output",
            "run: soldr cargo test --workspace --locked",
            "# soldr cargo test -- --nocapture",
        ):
            with self.subTest(line=line):
                self.assertEqual(find_nocapture("w.yaml", line), [])

    def test_workflow_cargo_test_steps_capture_output(self):
        hits = []
        for path in sorted(WORKFLOWS.glob("*.y*ml")):
            hits += find_nocapture(path.relative_to(ROOT).as_posix(), path.read_text(encoding="utf-8"))
        self.assertEqual(hits, [], f"{len(hits)} `cargo test` step(s) pass --nocapture:\n" + "\n".join(hits))

    def test_qt_sdk_step_keeps_its_stdout_for_the_version_check(self):
        """--show-output replaces --nocapture where a later step greps the log."""
        text = (WORKFLOWS / "build-project.yaml").read_text(encoding="utf-8")
        step = text.split("- name: Check Published Qt SDK", 1)[1].split("- name:", 1)[0]
        self.assertIn("--show-output", step)
        self.assertIn('grep -q ": Qt 6\\."', step)


if __name__ == "__main__":
    unittest.main()
