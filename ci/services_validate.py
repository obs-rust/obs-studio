#!/usr/bin/env -S uv run --no-project --script
"""The services validator is one Python call per step (#962, GEN-005).

`.github/actions/services-validator/action.yaml` carried three multi-line `run:`
steps -- the runner guard, the JSON-schema check and the defunct-service check
-- so the action produced a GEN-005 finding for each. Each step is now a single
call:

    uv run --no-project --script ci/services_validate.py guard \\
        --notice "..."

    uv run --no-project --script ci/services_validate.py schema \\
        plugins/rtmp-services/data/services.json \\
        plugins/rtmp-services/data/package.json

    uv run --no-project --script ci/services_validate.py defunct

`guard` keeps the step's behaviour: the action's own
`if: runner.os == 'Windows'` selects it, it prints the notice, and it exits 2 so
an unsupported runner fails the job instead of running the checks.

`schema` runs `.github/scripts/utils.py/check-jsonschema.py` for the files it is
given, through `uv run --no-project --with jsonschema --with json_source_map`
(the checker's two dependencies), and returns the checker's status: 0 valid, 1
for a schema violation with `validation_errors.json` written to the current
directory for the annotations step, 2 for an unusable file. The run is bracketed
by `::group::Schema Validation` / `::endgroup::` so the CI log collapses the
checker output the same way the shell step did.

The two data files are named explicitly instead of through the step's
`@(services|package)` extglob; that is the only shell construct the step used,
and `shopt -s extglob` went with it.

`defunct` runs `.github/scripts/utils.py/check-services.py` through
`uv run --no-project --with requests --with aiohttp`, inheriting the step's
environment (`GITHUB_TOKEN`, `WORKFLOW_RUN_ID`, `REPOSITORY`, `API_KEY`,
`API_SERVERS`) and returning its status; the script writes its own
`GITHUB_OUTPUT` entries, so the `Create pull request` step still sees them.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / '.github' / 'scripts' / 'utils.py'
CHECKER = SCRIPTS / 'check-jsonschema.py'
SERVICE_CHECKER = SCRIPTS / 'check-services.py'
SCHEMA_DEPENDENCIES = ('jsonschema', 'json_source_map')
SERVICE_DEPENDENCIES = ('requests', 'aiohttp')
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


def uv_run(dependencies: tuple[str, ...], script: Path, arguments: list[str]) -> int:
    """Run a checked-in script through uv with its third-party dependencies."""

    uv = uv_executable()
    if not uv:
        return 2

    command = [uv, 'run', '--no-project']
    for dependency in dependencies:
        command.extend(['--with', dependency])
    command.extend([str(script), *arguments])

    environment = dict(os.environ, PYTHONUNBUFFERED='1')
    completed = subprocess.run(command, env=environment, check=False)
    return completed.returncode


def schema(files: list[str]) -> int:
    """Validate the service data files against their JSON schema."""

    print('::group::Schema Validation', flush=True)
    try:
        return uv_run(
            SCHEMA_DEPENDENCIES, CHECKER, ['--loglevel', LOG_LEVEL, *files]
        )
    finally:
        print('::endgroup::', flush=True)


def defunct() -> int:
    """Check the configured services for defunct servers and services."""

    print('::group::Service Check', flush=True)
    try:
        return uv_run(SERVICE_DEPENDENCIES, SERVICE_CHECKER, [])
    finally:
        print('::endgroup::', flush=True)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = parser.add_subparsers(dest='command', required=True)

    guard_parser = subcommands.add_parser(
        'guard', help='fail a runner the services validator does not support'
    )
    guard_parser.add_argument(
        '--notice', required=True, help='the reason printed before the failure'
    )

    schema_parser = subcommands.add_parser(
        'schema', help='check the rtmp-services data against its JSON schema'
    )
    schema_parser.add_argument(
        'files', nargs='+', metavar='FILE', help='the JSON data file to validate'
    )

    subcommands.add_parser(
        'defunct', help='check the configured services for defunct entries'
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'guard':
        return guard(arguments.notice)
    if arguments.command == 'schema':
        return schema(arguments.files)
    return defunct()


if __name__ == '__main__':
    sys.exit(main())
