#!/usr/bin/env -S uv run --no-project --script
"""The compatibility validator is one Python call per step (#962, GEN-005).

`.github/actions/compatibility-validator/action.yaml` carried two multi-line
`run:` steps -- the Windows runner guard and the schema check -- so the action
produced a GEN-005 finding for each. Each step is now a single call:

    uv run --no-project --script ci/compatibility_validate.py guard \\
        --notice "..."

    uv run --no-project --script ci/compatibility_validate.py validate \\
        plugins/win-capture/data/compatibility.json \\
        plugins/win-capture/data/package.json

`guard` keeps the step's behaviour: the action's own
`if: runner.os == 'Windows'` selects it, it prints the notice, and it exits 2 so
an unsupported runner fails the job instead of running the check.

`validate` runs `.github/scripts/utils.py/check-jsonschema.py` for the files it
is given, through `uv run --no-project --with jsonschema --with json_source_map`
(the checker's two dependencies), and returns the checker's status: 0 valid,
1 for a schema violation with `validation_errors.json` written to the current
directory for the annotations step, 2 for an unusable file. The run is
bracketed by `::group::Schema Validation` / `::endgroup::` annotations so the
CI log collapses the checker output the same way the shell step did.

The two files are named explicitly instead of through the step's
`@(compatibility|package)` extglob; that is the only shell construct the step
used, and `shopt -s extglob` went with it.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / '.github' / 'scripts' / 'utils.py' / 'check-jsonschema.py'
DEPENDENCIES = ('jsonschema', 'json_source_map')
LOG_LEVEL = 'INFO'


def guard(notice: str) -> int:
    """Fail a runner this action does not support, with the reason in the log."""

    print(f'::notice::{notice}', flush=True)
    return 2


def uv_executable() -> str:
    """The `uv` on PATH (installed by the action's Set Up uv step)."""

    found = shutil.which('uv')
    if found is None:
        print('::error::uv is not on PATH; the action must run Set Up uv first')
        return ''
    return found


def validate(files: list[str]) -> int:
    """Run the schema checker for every file; return its status."""

    uv = uv_executable()
    if not uv:
        return 2

    command = [uv, 'run', '--no-project']
    for dependency in DEPENDENCIES:
        command.extend(['--with', dependency])
    command.extend([str(CHECKER), '--loglevel', LOG_LEVEL, *files])

    environment = dict(os.environ, PYTHONUNBUFFERED='1')
    print('::group::Schema Validation', flush=True)
    try:
        completed = subprocess.run(
            command, env=environment, check=False
        )
    finally:
        print('::endgroup::', flush=True)
    return completed.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = parser.add_subparsers(dest='command', required=True)

    guard_parser = subcommands.add_parser(
        'guard', help='fail a runner the compatibility validator does not support'
    )
    guard_parser.add_argument(
        '--notice', required=True, help='the reason printed before the failure'
    )

    validate_parser = subcommands.add_parser(
        'validate', help='check Windows compatibility data against its JSON schema'
    )
    validate_parser.add_argument(
        'files', nargs='+', metavar='FILE', help='the JSON data file to validate'
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'guard':
        return guard(arguments.notice)
    return validate(arguments.files)


if __name__ == '__main__':
    sys.exit(main())
