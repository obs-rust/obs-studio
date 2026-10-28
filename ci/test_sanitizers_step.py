#!/usr/bin/env -S uv run --script
"""The sanitizers workflow's two multi-line steps are one Python call each (#962).

    uv run python -m unittest ci/test_sanitizers_step.py

GEN-005 gives a workflow `run:` step a one-line shell budget. Two steps in
`.github/workflows/sanitizers.yaml` were over it, and both are now
`ci/sanitizers_step.py`:

- `jobs.matrix.steps[0]` (`id: pick`) turned `inputs.tools` into the `tools`
  matrix list with an `if … else` block that echoed into `$GITHUB_OUTPUT`.
  `select` does the same with the same literal JSON: `all` is the three passes,
  anything else is that one pass.
- `jobs.sanitize.steps[6]` (`Triage 🩺`) failed when the results directory was
  missing, with a line in the step summary, and otherwise ran `triage.py` with
  its output teed into the summary. `triage` keeps both behaviours, including
  the exit status the `pipefail` shell propagated.

RED first: committed while both steps were still shell blocks and the module
did not exist; every test here failed then (nothing to import, and the GEN-005
``[[exceptions]]`` block for the path was still in ci.toml).
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "ci" / "sanitizers_step.py"
WORKFLOW = ROOT / ".github" / "workflows" / "sanitizers.yaml"
CI_TOML = ROOT / "ci.toml"
WORKFLOW_PATH = ".github/workflows/sanitizers.yaml"

# The three passes sanitize.py builds; `all` is the default and the schedule's
# value, so the matrix is the full list.
ALL_PASSES = 'tools=["asan","tsan","valgrind"]'
NO_RESULTS = "No results (build failed?)"


def load_module():
    """ci/sanitizers_step.py by path: `uv run python -m unittest` does not put
    the repo root on sys.path, and no file here edits sys.path."""
    spec = importlib.util.spec_from_file_location("sanitizers_step", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class SelectTest(unittest.TestCase):
    """`select` publishes the pass matrix the same way the shell step did."""

    def run_select(self, tools: str) -> tuple[subprocess.CompletedProcess, str]:
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "github-output"
            output.touch()
            env = dict(os.environ, GITHUB_OUTPUT=str(output))
            done = subprocess.run(
                [sys.executable, str(MODULE), "select", "--tools", tools],
                capture_output=True,
                text=True,
                env=env,
                check=False,
            )
            return done, output.read_text()

    def test_all_is_the_three_passes(self):
        done, output = self.run_select("all")
        self.assertEqual(done.returncode, 0, done.stderr)
        self.assertEqual(output, ALL_PASSES + "\n")

    def test_one_pass_is_that_pass(self):
        for tool in ("asan", "tsan", "valgrind"):
            with self.subTest(tool=tool):
                done, output = self.run_select(tool)
                self.assertEqual(done.returncode, 0, done.stderr)
                self.assertEqual(output, f'tools=["{tool}"]\n')

    def test_missing_output_file_is_an_error(self):
        env = dict(os.environ)
        env.pop("GITHUB_OUTPUT", None)
        done = subprocess.run(
            [sys.executable, str(MODULE), "select", "--tools", "all"],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )
        self.assertNotEqual(done.returncode, 0)
        self.assertIn("GITHUB_OUTPUT", done.stderr)


class TriageTest(unittest.TestCase):
    """`triage` keeps the step's miss handling and its report plumbing."""

    def run_triage(self, out: Path, fail_on_findings: bool) -> tuple[int, str, str]:
        with tempfile.TemporaryDirectory() as tmp:
            summary = Path(tmp) / "summary"
            summary.touch()
            env = dict(os.environ, GITHUB_STEP_SUMMARY=str(summary))
            if fail_on_findings:
                env["FAIL_ON_FINDINGS"] = "true"
            else:
                env.pop("FAIL_ON_FINDINGS", None)
            done = subprocess.run(
                [sys.executable, str(MODULE), "triage", str(out)],
                capture_output=True,
                text=True,
                env=env,
                check=False,
            )
            return done.returncode, done.stdout, summary.read_text()

    def results(self, root: Path, summary: str) -> Path:
        """A pass directory the way sanitize.py leaves one: <out>/<tool>/tests/."""
        out = root / "asan"
        (out / "tests").mkdir(parents=True)
        (out / "tests" / "summary.txt").write_text(summary)
        return out

    def test_missing_results_fail_with_a_summary_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            code, stdout, summary = self.run_triage(Path(tmp) / "asan", False)
        self.assertEqual(code, 1)
        # The step says nothing on stdout, exactly as the shell did.
        self.assertEqual(stdout, "")
        self.assertEqual(summary, NO_RESULTS + "\n")

    def test_report_reaches_stdout_and_the_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = self.results(Path(tmp), "test_canvas exit=1\n")
            code, stdout, summary = self.run_triage(out, False)
        self.assertEqual(code, 0)
        self.assertIn("### asan", stdout)
        self.assertIn("0/1 clean", stdout)
        self.assertEqual(summary, stdout)

    def test_fail_on_findings_takes_the_failing_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = self.results(Path(tmp), "test_canvas exit=1\n")
            code, stdout, summary = self.run_triage(out, True)
        self.assertEqual(code, 1)
        self.assertIn("0/1 clean", stdout)
        self.assertEqual(summary, stdout)

    def test_clean_pass_succeeds(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = self.results(Path(tmp), "test_canvas exit=0\n")
            code, _, _ = self.run_triage(out, True)
        self.assertEqual(code, 0)


class WorkflowTest(unittest.TestCase):
    """Both steps are one line and the ci.toml GEN-005 block is gone."""

    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")

    def step_run(self, marker: str) -> str:
        block = self.text.split(marker, 1)
        self.assertEqual(len(block), 2, f"no {marker!r} step")
        rest = block[1].lstrip("\n")
        runs = []
        for line in rest.splitlines():
            if line == "" or line.startswith("      - "):
                break
            if line.strip().startswith("run:"):
                runs.append(line.strip())
        self.assertTrue(runs, f"{marker!r} has no run: line")
        return runs[0]

    def test_select_is_one_call(self):
        run = self.step_run("- id: pick")
        self.assertIn("uv run --no-project ci/sanitizers_step.py select", run)
        self.assertNotRegex(run, r"[;|&]")

    def test_triage_step_is_one_call(self):
        run = self.step_run("- name: Triage")
        self.assertIn("uv run --no-project ci/sanitizers_step.py triage", run)
        self.assertNotRegex(run, r"[;|&]")

    def test_matrix_job_can_run_uv(self):
        # The job installed nothing before the port; uv has to exist for the
        # one-line step, and only master saves its cache.
        block = self.text.split("  matrix:", 1)[1].split("  sanitize:", 1)[0]
        self.assertIn("astral-sh/setup-uv@", block)
        self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", block)

    def test_select_step_is_preceded_by_a_checkout(self):
        # `select` is a repo module, so the job needs the working tree. Without
        # it the nightly run died at `Failed to spawn: ci/sanitizers_step.py`
        # (#2797); the shell block it replaced needed no checkout.
        block = self.text.split("  matrix:", 1)[1].split("  sanitize:", 1)[0]
        self.assertIn("uses: actions/checkout@", block)
        self.assertLess(
            block.index("uses: actions/checkout@"),
            block.index("- id: pick"),
            "jobs.matrix must check the repository out before `pick`",
        )
        before_pick = block.split("- id: pick", 1)[0]
        self.assertIn("persist-credentials: false", before_pick)

    def test_no_gen_005_exception_left(self):
        exceptions = tomllib.loads(CI_TOML.read_text(encoding="utf-8"))["exceptions"]
        left = [e for e in exceptions if e.get("path") == WORKFLOW_PATH and e.get("rule") == "GEN-005"]
        self.assertEqual(left, [])


class CopyTest(unittest.TestCase):
    """The module is a real port, not a shell wrapper."""

    def test_module_has_no_shell_control_flow(self):
        module = load_module()
        self.assertTrue(callable(module.main))
        source = MODULE.read_text(encoding="utf-8")
        self.assertNotIn("shell=True", source)


if __name__ == "__main__":
    unittest.main()
