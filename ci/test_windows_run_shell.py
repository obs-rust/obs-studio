#!/usr/bin/env -S uv run --script
"""Every Windows-lane step must say which shell it wants (#2766).

A `run:` block in a job whose `runs-on:` names Windows is handed to PowerShell
unless the step (or its job's `defaults: run:`) says otherwise, so a bash body
there is a parse error, not a test failure: the step never runs and the lane is
red for a reason that has nothing to do with the change.

`rust-tests-windows`'s "Check Published Qt SDK" step was written from the macOS
twin (`if [[ ... ]]`, `start=${SECONDS}`, `export ...`) and kept its bash body
while losing nothing but the `shell:` key: a PR that touches
`rust/obs-build/src/platform/qt_sdk.rs` starts the Windows lane
(`ci/ci_scope.py` sets `qt_sdk = macos = windows = True`) and that step could
only fail. `ci/test_windows_diff_base.py` pins the one `${BASE}` read of the
changed-crates step; this test covers the whole shell question for every step
of every Windows job.

The tokens below are bash/POSIX-only. `${NAME}` is deliberately *not* one of
them: PowerShell expands `${Version}` and `${env:RUNNER_TEMP}` too, and the
ccache setup steps rely on that.

    uv run --no-project ci/test_windows_run_shell.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"

JOB = re.compile(r"^  ([A-Za-z0-9_-]+):\s*$")
STEP = re.compile(r"^      - ")
JOB_DEFAULT_SHELL = re.compile(r"^ {8}shell:\s*(\S+)")
STEP_SHELL = re.compile(r"^\s{8,}shell:\s*(\S+)")
POSIX_SHELLS = ("bash", "sh")

# Unambiguous bash/POSIX syntax; a PowerShell body does not contain any of it.
BASH_ONLY = (
    re.compile(r"\[\["),
    re.compile(r"^\s*(?:fi|esac|done)\s*$"),
    re.compile(r"^\s*export\s+[A-Za-z_][A-Za-z0-9_]*="),
    re.compile(r"\$\(\("),
    re.compile(r"\$\{SECONDS\}"),
    re.compile(r";\s*then\b"),
)


def job_blocks(text: str) -> dict[str, list[str]]:
    """The workflow's jobs, keyed by job id, split by two-space indentation."""
    lines = text.splitlines()
    starts = [(index, match.group(1)) for index, line in enumerate(lines) if (match := JOB.match(line))]
    blocks = {}
    for position, (start, name) in enumerate(starts):
        end = starts[position + 1][0] if position + 1 < len(starts) else len(lines)
        blocks[name] = lines[start:end]
    return blocks


def is_windows_job(block: list[str]) -> bool:
    return "windows" in next((line for line in block if line.strip().startswith("runs-on:")), "")


def job_default_shell(block: list[str]) -> str:
    """The shell a job's `defaults: run:` sets, or "" when it sets none."""
    for match in (JOB_DEFAULT_SHELL.match(line) for line in block):
        if match:
            return match.group(1)
    return ""


def step_shell(step: list[str]) -> str:
    for match in (STEP_SHELL.match(line) for line in step):
        if match:
            return match.group(1)
    return ""


def run_body(step: list[str]) -> list[str]:
    """The lines of the step's `run:` value (block scalar or single line)."""
    body: list[str] = []
    inside, run_indent = False, 0
    for line in step:
        if re.match(r"^\s*run:", line):
            inside, run_indent = True, len(line) - len(line.lstrip())
            after = line.split("run:", 1)[1].strip()
            if after and after not in ("|", ">", "|-", ">-"):
                body.append(after)
            continue
        if inside:
            if line.strip() and len(line) - len(line.lstrip()) <= run_indent:
                inside = False
            else:
                body.append(line)
    return body


def bash_only_lines(body: list[str]) -> list[str]:
    return [line.strip() for line in body if any(pattern.search(line) for pattern in BASH_ONLY)]


def powershell_steps() -> dict[str, list[str]]:
    """Steps of a Windows job that default to PowerShell but hold a bash body."""
    found: dict[str, list[str]] = {}
    for workflow in sorted(WORKFLOWS.glob("*.y*ml")):
        for job, block in job_blocks(workflow.read_text(encoding="utf-8")).items():
            if not is_windows_job(block):
                continue
            default = job_default_shell(block)
            if default.startswith(POSIX_SHELLS):
                continue
            starts = [index for index, line in enumerate(block) if STEP.match(line)]
            for position, start in enumerate(starts):
                end = starts[position + 1] if position + 1 < len(starts) else len(block)
                step = block[start:end]
                if step_shell(step):
                    continue
                offending = bash_only_lines(run_body(step))
                if offending:
                    found[f"{workflow.name}: {job}: {step[0].strip()[2:]}"] = offending
    return found


class WindowsRunShellTest(unittest.TestCase):
    def test_the_scan_finds_a_windows_job(self) -> None:
        names = [workflow.name for workflow in sorted(WORKFLOWS.glob("*.y*ml"))]
        self.assertIn("build-project.yaml", names)

    def test_no_bash_body_runs_under_powershell(self) -> None:
        offenders = powershell_steps()
        self.assertEqual(
            {},
            offenders,
            "a Windows step runs a bash body under PowerShell; add `shell: bash`:\n"
            + "\n".join(f"{name}\n    " + "\n    ".join(lines) for name, lines in offenders.items()),
        )


if __name__ == "__main__":
    unittest.main()
