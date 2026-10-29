#!/usr/bin/env -S uv run --no-project --script
"""The swift-format action is one Python call per step, and the checker is Python (#962).

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

`format` installs `swift-format` the same way, splits `CHANGED_FILES` the way
the zsh line did -- drop `[`, `]`, `'` and `"`, then split on `,` (the `s:,`
flag's separator) -- and then runs the check itself. The formatter used to be
`build-aux/run-swift-format`, upstream OBS's zsh dispatcher (a symlink to
`build-aux/.run-format.zsh`): that was the last live GEN-005 shell entry point
in `build-aux/`, so the check lives here instead -- the same `swift-format`
minimum version, the same formatted output compared byte for byte against the
file, the same `::error::` line per file that needs changes, and the same
`--fail-never|error|fast` status. Only the `swift` arm of the dispatcher
survived (the script's name selected it), so `clang-format` and `gersemi` are
untouched: the lint lane runs those from `uvx`. The `::group::` /
`::endgroup::` annotations bracket the same blocks, so the CI log collapses the
output as before.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
RETRY = ROOT / '.github' / 'scripts' / 'retry'
LINUXBREW_DIRECTORIES = ('/home/linuxbrew/.linuxbrew/bin', '/home/linuxbrew/.linuxbrew/sbin')
STRIPPED_CHARACTERS = str.maketrans('', '', "[]'\"")
SWIFT_FORMAT = 'swift-format'
MINIMUM_VERSION = (508, 0, 0)
SWIFT_GLOBS = ('libobs*/**/*.swift', 'frontend/**/*.swift', 'plugins/**/*.swift')
FAIL_ON_ERROR = {'never': 0, 'error': 1, 'fast': 2}


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
    """The zsh step's CHANGED_FILES split: drop brackets and quotes, then commas."""

    stripped = value.translate(STRIPPED_CHARACTERS)
    return [entry for entry in stripped.split(',') if entry]


def version_text(version: tuple[int, int, int]) -> str:
    return '.'.join(str(part) for part in version)


def parse_version(text: str) -> tuple[int, int, int]:
    """The first `MAJOR.MINOR.PATCH` in `swift-format --version`'s output."""

    match = re.search(r'(\d+)\.(\d+)\.(\d+)', text)
    if match is None:
        return (0, 0, 0)
    return (int(match.group(1)), int(match.group(2)), int(match.group(3)))


def formatter_binary() -> str:
    """`swift-format` from the PATH when it is there, otherwise by name."""

    return shutil.which(SWIFT_FORMAT) or SWIFT_FORMAT


def formatter_version() -> tuple[int, int, int] | None:
    """`swift-format`'s version, or None when the binary is not runnable."""

    try:
        completed = subprocess.run(
            [formatter_binary(), '--version'], capture_output=True, text=True, check=False
        )
    except OSError:
        return None
    return parse_version(f'{completed.stdout}\n{completed.stderr}')


def swift_sources() -> list[str]:
    """The dispatcher's default source list, for a run with no CHANGED_FILES."""

    found: list[str] = []
    for pattern in SWIFT_GLOBS:
        for path in sorted(ROOT.glob(pattern)):
            relative = path.relative_to(ROOT).as_posix()
            if relative not in found:
                found.append(relative)
    return found


def check_files(files: list[str], fail_on_error: int) -> int:
    """Compare `swift-format`'s output against each file, as the zsh check did."""

    binary = formatter_binary()
    failures = 0
    for name in files:
        formatted = subprocess.run([binary, name], capture_output=True, check=False)
        if formatted.returncode:
            print(f'::error::{name}: swift-format exited {formatted.returncode}.', flush=True)
        elif Path(name).read_bytes() != formatted.stdout:
            print(f'::error::{name} requires formatting changes.', flush=True)
        else:
            continue
        if fail_on_error == 2:
            return 2
        failures += 1
    return 2 if failures and fail_on_error == 1 else 0


def format_files(files: list[str]) -> int:
    """The dispatcher's default mode: rewrite the files with `swift-format -i`."""

    if not files:
        return 0
    return subprocess.run([formatter_binary(), '-i', *files], check=False).returncode


def format_swift(condition: str, write: bool = False) -> int:
    """Install `swift-format`, then check (or rewrite) the changed files."""

    fail_on_error = FAIL_ON_ERROR.get(condition)
    if fail_on_error is None:
        print(
            f"::error::Invalid fail condition '{condition}': "
            f"expected one of {', '.join(FAIL_ON_ERROR)}.",
            flush=True,
        )
        return 2

    for arguments in (['update'], ['install', '--quiet', 'swift-format']):
        status = brew(arguments)
        if status:
            return status

    version = formatter_version()
    if version is None:
        print(
            f'::error::No viable swift-format found (required {version_text(MINIMUM_VERSION)}).',
            flush=True,
        )
        return 2
    if version < MINIMUM_VERSION:
        print(
            f'::error::swift-format is not version {version_text(MINIMUM_VERSION)} '
            f'or above (found {version_text(version)}).',
            flush=True,
        )
        return 2

    files = split_changed_files(os.environ.get('CHANGED_FILES', '')) or swift_sources()
    print('::group::Run swift-format', flush=True)
    try:
        if write:
            return format_files(files)
        return check_files(files, fail_on_error)
    finally:
        print('::endgroup::', flush=True)


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
        'format', help='check (or with --write, rewrite) the changed Swift files'
    )
    format_parser.add_argument(
        '--fail-condition',
        required=True,
        help="the action input: 'never' or 'error' ('fast' also accepted)",
    )
    format_parser.add_argument(
        '--write',
        action='store_true',
        help='rewrite the files in place instead of checking them',
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'guard':
        return guard(arguments.notice)
    if arguments.command == 'install-deps':
        return install_dependencies()
    return format_swift(arguments.fail_condition, write=arguments.write)


if __name__ == '__main__':
    sys.exit(main())
