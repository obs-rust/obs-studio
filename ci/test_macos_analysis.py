#!/usr/bin/env -S uv run --script
"""Deduplicate queue analysis without losing macOS target coverage (#4999)."""

from __future__ import annotations

import re
import unittest
from pathlib import Path
from unittest import mock

import gate_jobs
import local_gate
from ci_scope import scope

ROOT = Path(__file__).resolve().parent.parent
BUILD = (ROOT / ".github/workflows/build-project.yaml").read_text()


def job(name: str) -> str:
    return re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:|\Z)", BUILD, re.M | re.S)[1]


def step(name: str) -> str:
    return re.search(rf"      - name: {name}[^\n]*\n(.*?)(?=      - |\Z)",
                     job("rust-tests-macos"), re.S)[1]


def runs(condition: str, event: str, selected: str, flatpak: bool = False, native: bool = False) -> bool:
    """Evaluate the small Actions boolean expression from the actual workflow."""
    expression = condition.removeprefix("${{").removesuffix("}}")
    expression = expression.replace("github.event_name", repr(event))
    expression = expression.replace("needs.check-event.outputs.cross_lint", repr(selected))
    expression = expression.replace("inputs.flatpak-measurement-only", repr(flatpak))
    expression = expression.replace("inputs.native-cache-measurement-only", repr(native))
    expression = expression.replace("&&", " and ").replace("||", " or ")
    expression = re.sub(r"!(?!=)", " not ", expression)
    return bool(eval(expression, {"__builtins__": {}}))


class MacosAnalysis(unittest.TestCase):
    def test_only_explicit_queue_replacement_skips_native_analysis(self):
        for name in ("Run Clippy", "Check Formatting"):
            with self.subTest(step=name):
                condition = re.search(r"^        if: (.+)$", step(name), re.M)
                self.assertIsNotNone(condition, "native analysis must defer to required queue analysis")
                for event in ("merge_group", "pull_request", "push", "schedule", "workflow_dispatch"):
                    for selected in ("true", "false", "", "unknown"):
                        for flatpak, native in ((False, False), (True, False), (False, True)):
                            with self.subTest(event=event, selected=selected, flatpak=flatpak, native=native):
                                replacement = event == "merge_group" and selected == "true" and not flatpak and not native
                                self.assertEqual(runs(condition[1], event, selected, flatpak, native), not replacement)

    def test_replacement_clippy_includes_tests_and_oracle(self):
        target = "aarch64-apple-darwin"
        with mock.patch.object(local_gate, "add_target"), \
                mock.patch.object(local_gate, "soldr_cargo") as cargo, \
                mock.patch.object(local_gate, "soldr_dylint") as dylint:
            local_gate.cross_lint((target,), link=True)
        self.assertEqual(cargo.call_args_list, [
            mock.call(["clippy", "--workspace", "--all-targets", "--locked", "--target", target, "--", "-D", "warnings"]),
            mock.call(["test", "--workspace", "--no-run", "--locked", "--target", target]),
        ])
        # Dylint still checks the production boundary; test linking remains real.
        dylint.assert_called_once_with(["--all", "--", "--workspace", "--exclude", "obs-c-oracle",
                                       "--lib", "--bins", "--target", target])

    def test_failed_replacement_analysis_propagates(self):
        import subprocess

        with mock.patch.object(local_gate, "add_target"), \
                mock.patch.object(local_gate, "soldr_cargo", side_effect=subprocess.CalledProcessError(1, "clippy")), \
                mock.patch.object(local_gate, "soldr_dylint"):
            with self.assertRaises(subprocess.CalledProcessError):
                local_gate.cross_lint(("aarch64-apple-darwin",), link=True)

    def test_cumulative_queue_scope_requires_real_replacement_and_native_jobs(self):
        # A docs-only entry followed by a core entry must retain the full group's
        # analysis and native execution, even if PR/client proof metadata exists.
        selected = scope(["docs/features.md", "rust/obs-util/src/dstr.rs"], event="merge_group")
        self.assertTrue(selected.cross_lint)
        expected = gate_jobs.expected_jobs(selected)
        required = {
            "Build 🧱 / Fast Gate ⚡",
            "Build 🧱 / Rust Cross Lint 🦀 (aarch64-apple-darwin)",
            "Build 🧱 / Rust Tests 🦀 (macos-26)",
            "Build 🧱 / Rust Tier 2 (macos-26)",
        }
        self.assertLessEqual(required, expected)
        healthy = [{"name": name, "status": "completed", "conclusion": "success"} for name in expected]
        self.assertEqual(gate_jobs.blocking(healthy, expected=expected), [])
        for name in required:
            rest = [record for record in healthy if record["name"] != name]
            self.assertEqual(gate_jobs.blocking(rest, expected=expected), [name])
            for conclusion, status in (("skipped", "completed"), ("failure", "completed"),
                                       ("cancelled", "completed"), (None, "queued")):
                with self.subTest(job=name, conclusion=conclusion):
                    record = {"name": name, "status": status, "conclusion": conclusion,
                              "client_attested": True, "proof_hit": True}
                    self.assertEqual(gate_jobs.blocking([*rest, record], expected=expected), [name])

    def test_native_runtime_tests_are_still_selected(self):
        self.assertIn("run: soldr cargo test --workspace --locked", step("Run Rust Tests 🧪"))
        self.assertIn("github.event_name == 'merge_group'", step("Run Rust Tests 🧪"))
        cross = job("rust-cross-lint")
        self.assertIn("aarch64-apple-darwin", cross)
        self.assertIn("--cross-target ${{ matrix.target }}", cross)
        self.assertIn("needs.check-event.outputs.cross_lint == 'true'", cross)
        self.assertIn("runs-on: macos-26", job("rust-tests-macos"))
        self.assertIn("runs-on: macos-26", job("rust-tier2-macos"))


if __name__ == "__main__":
    unittest.main()
