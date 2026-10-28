#!/usr/bin/env -S uv run --no-project --script
"""The scheduled workflow's three multi-line steps are one Python call each (#962).

`.github/workflows/scheduled.yaml` carried three `run:` blocks, so `ci-lint
precheck` reported a GEN-005 finding for the path. Each step is now a single
call:

    uv run --no-project --script ci/scheduled_steps.py check-nightly-runs

    uv run --no-project --script ci/scheduled_steps.py report-nightly-failure

`check-nightly-runs` is the `Check Nightly Runs` step both the
`upload-language-files` (bash) and `steam-upload` (zsh) jobs carry: it asks
`gh run list --workflow scheduled.yaml --limit 2 --json headSha` for the run
list, takes the second entry -- the run before this one, since the first is the
active run -- and compares its `headSha` with `GITHUB_SHA`. Equal means this
nightly already ran on the same commit, so `passed=false`; otherwise
`passed=true` and, as the bash copy did, `lastNightly=<sha>`. Both land in
`$GITHUB_OUTPUT`. The zsh copy omitted `lastNightly`, which only the bash job's
`check-changes` step reads; one subcommand serves both.

`report-nightly-failure` keeps the step that opens or updates the nightly
failure issue: the same title, the same body built from `SHA` and `RUN_URL`, the
same `gh issue list --state open --search 'in:title "<title>"'` lookup, then
`gh issue comment` on the open issue or `gh issue create` when there is none.
`GH_TOKEN` and `GH_REPO` stay in the step's `env:`, as they were.

Every `gh` argv is the step's argv, and its exit status is the step's status.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

TITLE = 'ci: Nightly full run failed on master'
BODY = (
    'The scheduled full run failed on {sha}: {url}\n'
    '\n'
    'PRs skip these checks (#992), so a recent merge may have broken them.'
)
RUN_LIST = (
    'gh',
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
)
ISSUE_LIST = (
    'gh',
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
)


def announce(argv: Sequence[str], completed: subprocess.CompletedProcess[str] | None = None) -> None:
    """The bash copy's `set -x` and the zsh copy's `XTRACE`: log the call."""

    if not os.environ.get('RUNNER_DEBUG'):
        return
    print(f'::debug::{completed.args if completed else " ".join(argv)}', flush=True)


def write_outputs(outputs: dict[str, str]) -> None:
    """Append `name=value` lines to the step's `$GITHUB_OUTPUT` file."""

    output = os.environ.get('GITHUB_OUTPUT')
    if not output:
        raise SystemExit('scheduled-steps: GITHUB_OUTPUT is not set')
    with Path(output).open('a', encoding='utf-8') as handle:
        for name, value in outputs.items():
            handle.write(f'{name}={value}\n')


def check_nightly_runs() -> int:
    """`passed` (and `lastNightly`) from the previous nightly run's commit."""

    announce(RUN_LIST)
    listed = subprocess.run(RUN_LIST, capture_output=True, text=True, check=False)
    announce(RUN_LIST, listed)
    if listed.returncode != 0:
        sys.stderr.write(listed.stderr)
        return listed.returncode

    last_nightly = listed.stdout.strip()
    if os.environ.get('GITHUB_SHA') == last_nightly:
        write_outputs({'passed': 'false'})
        print(f'This commit already ran the nightly: {last_nightly}', flush=True)
        return 0

    write_outputs({'passed': 'true', 'lastNightly': last_nightly})
    print(f'Last nightly ran on: {last_nightly}', flush=True)
    return 0


def report_nightly_failure() -> int:
    """Comment on the open nightly failure issue, or open it."""

    body = BODY.format(
        sha=os.environ.get('SHA', ''),
        url=os.environ.get('RUN_URL', ''),
    )
    announce(ISSUE_LIST)
    listed = subprocess.run(ISSUE_LIST, capture_output=True, text=True, check=False)
    announce(ISSUE_LIST, listed)
    if listed.returncode != 0:
        sys.stderr.write(listed.stderr)
        return listed.returncode

    existing = listed.stdout.strip()
    if existing:
        argv = ('gh', 'issue', 'comment', existing, '--body', body)
    else:
        argv = ('gh', 'issue', 'create', '--title', TITLE, '--body', body)
    announce(argv)
    return subprocess.run(argv, check=False).returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = parser.add_subparsers(dest='command', required=True)
    subcommands.add_parser(
        'check-nightly-runs', help='publish passed/lastNightly for this commit'
    )
    subcommands.add_parser(
        'report-nightly-failure', help='open or update the nightly failure issue'
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'check-nightly-runs':
        return check_nightly_runs()
    return report_nightly_failure()


if __name__ == '__main__':
    sys.exit(main())
