#!/usr/bin/env -S uv run --script
"""The Swift check's trigger is decided in Python (#962).

The check-format workflow only spends a macOS runner when a Swift file
changed; GEN-005 wants that decision out of a multi-line run: step.

    uv run --no-project ci/test_swift_changes.py
"""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci/swift_changes.py"
WORKFLOW = ROOT / ".github/workflows/check-format.yaml"


class SwiftChangesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = Path(self.tmp.name)
        self.output = self.repo / "output"
        self.git("init", "-q")
        self.git("config", "user.email", "test@example.com")
        self.git("config", "user.name", "Test")
        self.commit({"README.md": "base\n"})
        self.base = self.git("rev-parse", "HEAD").stdout.strip()

    def git(self, *args: str) -> subprocess.CompletedProcess:
        return subprocess.run(["git", *args], cwd=self.repo, capture_output=True, text=True, check=True)

    def commit(self, files: dict[str, str]) -> None:
        for name, body in files.items():
            path = self.repo / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(body)
            subprocess.run(["git", "add", "--", name], cwd=self.repo, check=True)
        self.git("commit", "-q", "-m", "change")

    def run_script(self, base: str | None = None) -> subprocess.CompletedProcess:
        self.output.write_text("")
        env = {k: v for k, v in os.environ.items() if k != "BASE"}
        env.update({"GITHUB_OUTPUT": str(self.output)})
        if base is not None:
            env["BASE"] = base
        return subprocess.run(
            [sys.executable, str(SCRIPT)], cwd=self.repo, capture_output=True, text=True, env=env
        )

    def output_value(self) -> str:
        return self.output.read_text().strip()

    def test_without_a_base_every_push_checks(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.output_value(), "swift=true")
        self.assertIn("Swift files changed: true", result.stdout)

    def test_a_swift_change_triggers_the_check(self) -> None:
        self.commit({"frontend/App.swift": "let x = 1\n"})
        result = self.run_script(self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("swift=true", self.output_value())

    def test_other_changes_do_not(self) -> None:
        self.commit({"README.md": "changed\n", "frontend/widget.cpp": "int x;\n", "Swift.swift.in": "\n"})
        result = self.run_script(self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("swift=false", self.output_value())
        self.assertIn("Swift files changed: false", result.stdout)

    def test_the_workflow_runs_the_script_in_one_line(self) -> None:
        text = WORKFLOW.read_text()
        self.assertIn("run: uv run --no-project ci/swift_changes.py", text)
        step = text.split("- name: Check for Swift Changes 🔎", 1)[1].split("- name:", 1)[0]
        self.assertNotIn("|", step.split("run:", 1)[1])


if __name__ == "__main__":
    unittest.main()
