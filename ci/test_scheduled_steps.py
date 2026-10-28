#!/usr/bin/env -S uv run --script
"""The scheduled workflow's three multi-line steps are one Python call each (#962).

    uv run python -m unittest ci/test_scheduled_steps.py

GEN-005 gives a workflow `run:` step a one-line shell budget. Three steps in
`.github/workflows/scheduled.yaml` were over it, and all three are now
`ci/scheduled_steps.py`:

- `report-nightly-failure`'s `Open or Update the Nightly Failure Issue`: the
  same title, the same body from `SHA`/`RUN_URL`, the same `gh issue list`
  lookup, then `gh issue comment` on the open issue or `gh issue create`.
- `upload-language-files`' `Check Nightly Runs` (bash) and `steam-upload`'s
  `Check Nightly Runs` (zsh): the same `gh run list` argv over the previous
  nightly's `headSha`, then `passed` -- and, as the bash copy wrote it,
  `lastNightly` -- into `$GITHUB_OUTPUT`.

RED first: committed while the three steps were still shell blocks and the
module did not exist, so every test here failed then (nothing to import, no
one-line `run:`, and the GEN-005 ``[[exceptions]]`` block still in `ci.toml`).
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'scheduled_steps.py'
WORKFLOW = ROOT / '.github' / 'workflows' / 'scheduled.yaml'
CI_TOML = ROOT / 'ci.toml'
WORKFLOW_PATH = '.github/workflows/scheduled.yaml'
CALL = 'uv run --no-project --script ${{ github.workspace }}/ci/scheduled_steps.py'
TITLE = 'ci: Nightly full run failed on master'
RUN_ARGS = [
    'run',
    'list',
    '--workflow',
    'scheduled.yaml',
    '--limit',
    '2',
    '--json',
    'headSha',
    '--jq',
    '.[1].headSha',
]
ISSUE_ARGS = [
    'issue',
    'list',
    '--state',
    'open',
    '--search',
    f'in:title "{TITLE}"',
    '--json',
    'number',
    '--jq',
    '.[0].number',
]

# A `gh` that records one argv per `--END--` record (NUL-separated, so an
# argument with newlines in it stays one argument) and prints canned stdout.
STUB = """#!/bin/sh
for argument in "$@"; do printf '%s\\0' "$argument"; done >> "$GH_LOG"
printf '%s\\0' --END-- >> "$GH_LOG"
cat "$GH_STDOUT" 2>/dev/null
exit "${GH_STATUS:-0}"
"""


def load_module():
    """ci/scheduled_steps.py by path: `uv run python -m unittest` does not put
    the repo root on sys.path, and no file here edits sys.path."""

    spec = importlib.util.spec_from_file_location('scheduled_steps', MODULE)
    assert spec is not None and spec.loader is not None, f'cannot load {MODULE}'
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class Recorded:
    """One run of the module: its status, `$GITHUB_OUTPUT`, and each `gh` argv."""

    def __init__(self, status: int, stdout: str, stderr: str, output: str, calls: list[list[str]]):
        self.status = status
        self.stdout = stdout
        self.stderr = stderr
        self.output = output
        self.calls = calls


def record(
    command: str,
    gh_stdout: str = '',
    gh_status: int = 0,
    **environment: str,
) -> Recorded:
    """Run one subcommand with the stub `gh` first on `PATH`."""

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        bin_directory = root / 'bin'
        bin_directory.mkdir()
        stub = bin_directory / 'gh'
        stub.write_text(STUB, encoding='utf-8')
        stub.chmod(0o755)
        recorded = root / 'calls'
        recorded.touch()
        canned = root / 'stdout'
        canned.write_text(gh_stdout, encoding='utf-8')
        output = root / 'github-output'
        output.touch()

        env = {
            key: value for key, value in os.environ.items() if not key.startswith('GH_')
        }
        env.update(
            {
                'PATH': f'{bin_directory}{os.pathsep}{env.get("PATH", "")}',
                'GH_LOG': str(recorded),
                'GH_STDOUT': str(canned),
                'GH_STATUS': str(gh_status),
                'GITHUB_OUTPUT': str(output),
            }
        )
        env.update(environment)
        done = subprocess.run(
            [sys.executable, str(MODULE), command],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )

        calls: list[list[str]] = []
        current: list[str] = []
        for field in recorded.read_text(encoding='utf-8').split('\0'):
            if field == '--END--':
                calls.append(current)
                current = []
            elif field:
                current.append(field)
        return Recorded(done.returncode, done.stdout, done.stderr, output.read_text(), calls)


class CheckNightlyRunsTest(unittest.TestCase):
    """`check-nightly-runs` publishes the same outputs the two shell steps did."""

    def test_same_commit_already_ran(self):
        run = record('check-nightly-runs', 'a8ac9c0ed\n', GITHUB_SHA='a8ac9c0ed')
        self.assertEqual(run.status, 0, run.stderr)
        self.assertEqual(run.output, 'passed=false\n')
        self.assertEqual(run.calls, [RUN_ARGS])

    def test_new_commit_publishes_the_previous_nightly(self):
        run = record('check-nightly-runs', 'a8ac9c0ed\n', GITHUB_SHA='deadbeef')
        self.assertEqual(run.status, 0, run.stderr)
        self.assertEqual(run.output, 'passed=true\nlastNightly=a8ac9c0ed\n')

    def test_failing_gh_fails_the_step(self):
        run = record('check-nightly-runs', gh_status=1, GITHUB_SHA='deadbeef')
        self.assertEqual(run.status, 1)
        self.assertEqual(run.output, '')


class ReportNightlyFailureTest(unittest.TestCase):
    """`report-nightly-failure` comments on the open issue, or opens it."""

    def test_open_issue_gets_a_comment(self):
        run = record(
            'report-nightly-failure',
            '1395\n',
            SHA='deadbeef',
            RUN_URL='https://github.com/obs-rust/obs-studio/actions/runs/1',
        )
        self.assertEqual(run.status, 0, run.stderr)
        body = (
            'The scheduled full run failed on deadbeef: '
            'https://github.com/obs-rust/obs-studio/actions/runs/1\n'
            '\n'
            'PRs skip these checks (#992), so a recent merge may have broken them.'
        )
        self.assertEqual(
            run.calls,
            [ISSUE_ARGS, ['issue', 'comment', '1395', '--body', body]],
        )

    def test_no_open_issue_opens_one(self):
        run = record(
            'report-nightly-failure',
            '\n',
            SHA='deadbeef',
            RUN_URL='https://github.com/obs-rust/obs-studio/actions/runs/1',
        )
        self.assertEqual(run.status, 0, run.stderr)
        created = run.calls[1]
        self.assertEqual(created[:4], ['issue', 'create', '--title', TITLE])
        self.assertEqual(created[4], '--body')
        self.assertTrue(created[5].startswith('The scheduled full run failed on deadbeef: '))

    def test_failing_gh_fails_the_step(self):
        run = record('report-nightly-failure', gh_status=1, SHA='deadbeef', RUN_URL='u')
        self.assertEqual(run.status, 1)
        self.assertEqual(len(run.calls), 1)


class WorkflowTest(unittest.TestCase):
    """Every step is one line, and the ci.toml GEN-005 block is gone."""

    def setUp(self):
        self.text = WORKFLOW.read_text(encoding='utf-8')

    def job(self, name: str) -> str:
        block = self.text.split(f'\n  {name}:', 1)
        self.assertEqual(len(block), 2, f'no {name!r} job')
        return re.split(r'\n  (?=\S)', block[1], maxsplit=1)[0]

    def runs(self, name: str) -> list[str]:
        """Every `run:` line of one job, in order."""

        return [
            line.strip()
            for line in self.job(name).splitlines()
            if line.strip().startswith('run:')
        ]

    def test_no_block_run_step_left(self):
        self.assertNotIn('run: |', self.text)

    def test_nightly_checks_are_one_call(self):
        for name in ('upload-language-files', 'steam-upload'):
            with self.subTest(job=name):
                runs = self.runs(name)
                self.assertEqual(len(runs), 1, runs)
                self.assertEqual(runs[0], f'run: {CALL} check-nightly-runs')
                self.assertNotRegex(runs[0], r'[;|&]')

    def test_failure_report_is_one_call(self):
        runs = self.runs('report-nightly-failure')
        self.assertEqual(len(runs), 1, runs)
        self.assertEqual(runs[0], f'run: {CALL} report-nightly-failure')
        self.assertNotRegex(runs[0], r'[;|&]')

    def test_report_job_checks_out_and_sets_uv_up(self):
        # The module lives in the repository, and the job had no checkout: the
        # shell block it replaced needed neither (#2797 was the same miss).
        job = self.job('report-nightly-failure')
        self.assertIn('uses: actions/checkout@', job)
        self.assertIn('persist-credentials: false', job)
        self.assertIn('astral-sh/setup-uv@', job)
        self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", job)
        self.assertLess(
            job.index('uses: actions/checkout@'), job.index('ci/scheduled_steps.py')
        )
        self.assertIn('contents: read', job)

    def test_every_porting_job_can_run_uv(self):
        for name in ('upload-language-files', 'steam-upload'):
            with self.subTest(job=name):
                job = self.job(name)
                self.assertIn('astral-sh/setup-uv@', job)
                self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", job)

    def test_steam_upload_no_longer_needs_zsh(self):
        job = self.job('steam-upload')
        self.assertNotIn('shell: zsh', job)
        self.assertNotIn('defaults:', job)

    def test_no_gen_005_exception_left(self):
        exceptions = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))['exceptions']
        left = [
            entry
            for entry in exceptions
            if entry.get('path') == WORKFLOW_PATH and entry.get('rule') == 'GEN-005'
        ]
        self.assertEqual(left, [])


class CopyTest(unittest.TestCase):
    """The module is a real port, not a shell wrapper."""

    def test_module_has_no_shell_control_flow(self):
        source = MODULE.read_text(encoding='utf-8')
        for fragment in ('shell=True', 'bash -c', 'set -e', 'setopt'):
            with self.subTest(fragment=fragment):
                self.assertNotIn(fragment, source)


if __name__ == '__main__':
    unittest.main()
