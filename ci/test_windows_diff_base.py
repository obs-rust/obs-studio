#!/usr/bin/env -S uv run --script
"""The Windows changed-crates step must hand local_gate its base commit.

A `run:` block in a job that runs on `windows-*` uses PowerShell unless the
step says `shell: bash`, and PowerShell reads an environment variable as
`$env:NAME`: a bash-style `${BASE}` is an *unset PowerShell variable*, i.e. the
empty string. The step then ran

    uv run --no-project ci/local_gate.py --test-diff ""

which is falsy to argparse, so local_gate fell through to running every lane —
including the POSIX-only ci/ self-tests (grep, `/run/opengl-driver` paths,
`Path.read_text()` under cp1252) — and the Windows Rust Tests job went red on
every PR that touches a Windows path (#2743's run, 2026-10-28).

    uv run --no-project ci/test_windows_diff_base.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"

JOB = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")
STEP = re.compile(r"^      - ")
ENV_NAME = re.compile(r"^\s+([A-Z][A-Z0-9_]*):")


def job_blocks() -> dict[str, list[str]]:
    """The workflow's jobs, keyed by job id, split by two-space indentation."""
    lines = WORKFLOW.read_text(encoding="utf-8").splitlines()
    starts = [(index, match.group(1)) for index, line in enumerate(lines) if (match := JOB.match(line))]
    blocks = {}
    for position, (start, name) in enumerate(starts):
        end = starts[position + 1][0] if position + 1 < len(starts) else len(lines)
        blocks[name] = lines[start:end]
    return blocks


def changed_crates_steps() -> dict[str, list[str]]:
    """Every step of a Windows job that runs the changed-crates tests."""
    found = {}
    for job, block in job_blocks().items():
        runs_on = next((line for line in block if line.strip().startswith("runs-on:")), "")
        if "windows" not in runs_on:
            continue
        starts = [index for index, line in enumerate(block) if STEP.match(line)]
        for position, start in enumerate(starts):
            end = starts[position + 1] if position + 1 < len(starts) else len(block)
            step = block[start:end]
            if any("--test-diff" in line for line in step):
                found[f"{job}: {step[0].strip()[2:].strip()}"] = step
    return found


class WindowsDiffBaseTest(unittest.TestCase):
    def test_there_is_a_changed_crates_step_to_check(self) -> None:
        self.assertTrue(changed_crates_steps(), "no Windows job runs the changed-crates tests")

    def test_the_base_is_read_the_way_powershell_reads_the_environment(self) -> None:
        for name, step in changed_crates_steps().items():
            with self.subTest(step=name):
                text = "\n".join(step)
                self.assertNotIn("${BASE}", text, "PowerShell reads $env:BASE, not ${BASE}")
                self.assertIn("$env:BASE", text)

    def test_no_env_variable_of_the_step_is_read_with_bash_syntax(self) -> None:
        """An env name the step declares must not appear as `${NAME}`: PowerShell
        reads a declared environment variable only through `$env:NAME`."""
        for name, step in changed_crates_steps().items():
            text = "\n".join(step)
            for env_name in sorted({match.group(1) for line in step if (match := ENV_NAME.match(line))}):
                with self.subTest(step=name, variable=env_name):
                    self.assertNotIn("${" + env_name + "}", text)


if __name__ == "__main__":
    unittest.main()
