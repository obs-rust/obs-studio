#!/usr/bin/env -S uv run --no-project --script
"""The swift-format action is one Python call per step (#962, GEN-005).

`.github/actions/run-swift-format/action.yaml` carried three multi-line `run:`
steps -- the Windows runner guard, the Linux Homebrew dependency step and the
`Run swift-format` step -- so the action produced a GEN-005 finding for each.
Each step is now a single call:

    uv run --no-project --script ci/run_swift_format.py guard \\
        --notice "..."

    uv run --no-project --script ci/run_swift_format.py install-deps

    uv run --no-project --script ci/run_swift_format.py format \\
        --fail-condition never

`guard` keeps the step's behaviour: the action's own
`if: runner.os == 'Windows'` selects it, it prints the notice, and it exits 2 so
an unsupported runner fails the job instead of running the check.

`install-deps` is the Linux step: `brew update` and `brew install --quiet zsh`
through `.github/scripts/retry`, then the linuxbrew `bin`/`sbin` directories on
`$GITHUB_PATH`. The shell step also `eval`ed `brew shellenv`, which only
exported `PATH` inside that one step; the module prepends the same directories
to its own `PATH` for the rest of the run and puts them on `$GITHUB_PATH` for
the following steps.

`format` installs `swift-format` the same way, then splits `CHANGED_FILES` the
way the zsh line did -- `${(s:,:)CHANGED_FILES//[\\[\\]\\'\\"]/}`, i.e. drop
`[`, `]`, `'` and `"`, then split on `,` (the `s:,` flag's separator) -- and
returns
`build-aux/run-swift-format --fail-<condition> --check <files>`'s status. The
`::group::` / `::endgroup::` annotations bracket the same two blocks, so the CI
log collapses the output as before.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RETRY = ROOT / '.github' / 'scripts' / 'retry'
FORMATTER = ROOT / 'build-aux' / 'run-swift-format'
LINUXBREW_DIRECTORIES = ('/home/linuxbrew/.linuxbrew/bin', '/home/linuxbrew/.linuxbrew/sbin')
STRIPPED_CHARACTERS = str.maketrans('', '', "[]'\"")


def guard(notice: str) -> int:
    """Fail a runner this action does not support, with the reason in the log."""

    print(f'::notice::{notice}', flush=True)
    return 2


def brew(arguments: list[str]) -> int:
    """Run one Homebrew command the way the shell step did: through retry."""

    completed = subprocess.run([str(RETRY), 'brew', *arguments], check=False)
    return completed.returncode


def install_dependencies() -> int:
    """Homebrew itself plus `zsh`, and the linuxbrew directories on the PATH."""

    for arguments in (['update'], ['install', '--quiet', 'zsh']):
        status = brew(arguments)
        if status:
            return status

    current = os.environ.get('PATH', '')
    os.environ['PATH'] = os.pathsep.join([*LINUXBREW_DIRECTORIES, current])
    github_path = os.environ.get('GITHUB_PATH')
    if github_path:
        with Path(github_path).open('a', encoding='utf-8') as handle:
            for directory in LINUXBREW_DIRECTORIES:
                handle.write(f'{directory}\n')
    return 0


def split_changed_files(value: str) -> list[str]:
    """The zsh step's `${(s:,:)CHANGED_FILES//[\\[\\]\\'\\"]/}`, in Python."""

    stripped = value.translate(STRIPPED_CHARACTERS)
    return [entry for entry in stripped.split(',') if entry]


def format_swift(condition: str) -> int:
    """Install `swift-format`, then check the changed files with it."""

    for arguments in (['update'], ['install', '--quiet', 'swift-format']):
        status = brew(arguments)
        if status:
            return status

    changed = split_changed_files(os.environ.get('CHANGED_FILES', ''))
    command = [str(FORMATTER), f'--fail-{condition}', '--check', *changed]
    print('::group::Run swift-format', flush=True)
    try:
        completed = subprocess.run(command, check=False)
    finally:
        print('::endgroup::', flush=True)
    return completed.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = parser.add_subparsers(dest='command', required=True)

    guard_parser = subcommands.add_parser(
        'guard', help='fail a runner the swift-format action does not support'
    )
    guard_parser.add_argument(
        '--notice', required=True, help='the reason printed before the failure'
    )

    subcommands.add_parser('install-deps', help='install the Linux Homebrew deps')

    format_parser = subcommands.add_parser(
        'format', help='run build-aux/run-swift-format over CHANGED_FILES'
    )
    format_parser.add_argument(
        '--fail-condition',
        required=True,
        help="the action input: 'never' or 'error' (the step is --fail-<condition>)",
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'guard':
        return guard(arguments.notice)
    if arguments.command == 'install-deps':
        return install_dependencies()
    return format_swift(arguments.fail_condition)


if __name__ == '__main__':
    sys.exit(main())
