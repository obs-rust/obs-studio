#!/usr/bin/env -S uv run --no-project --script
"""The Qt UI XML validator is one Python call per step (#2724, GEN-005).

`.github/actions/qt-xml-validator/action.yaml` carried three multi-line `run:`
blocks -- the Windows runner guard, the `libxml2-utils` install and the
`xmllint` schema check over the changed `frontend/forms/**/*.ui` files -- so
the action produced a GEN-005 finding for each. Each step is now one call:

    uv run --no-project --script ci/qt_xml_validate.py guard --notice "..."
    uv run --no-project --script ci/qt_xml_validate.py validate \
        --changed-files-json "$CHANGED_FILES" \
        --schema frontend/forms/XML-Schema-Qt5.15.xsd \
        --fail-condition error

`guard` keeps the step's behaviour: the action's own
`if: runner.os == 'Windows'` selects it, it prints the notice, and it exits 2
so an unsupported runner fails the job instead of running the check.

`validate` parses the `check-changes` output, ensures `xmllint` is on `PATH`
(installing `libxml2-utils` through `sudo apt-get` on Linux, which is what the
deleted install step did), and runs the schema check. `--fail-condition never`
-- the action input's default -- reproduces the old block's `set +e`: the
validator's report is printed and the step passes.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys

ADVISORY = 'never'
XMMLINT_PACKAGE = 'libxml2-utils'
DEFAULT_SCHEMA = 'frontend/forms/XML-Schema-Qt5.15.xsd'
_STRIPPED = str.maketrans('', '', "[]'\"")


def guard(notice: str) -> int:
    """Fail a runner this action does not support, with the reason in the log."""

    print(f'::notice::{notice}', flush=True)
    return 2


def parse_changed_files(raw: str) -> list[str]:
    """Read the `check-changes` output: a JSON list, or the shell's `,` form."""

    text = raw.strip()
    if not text:
        return []
    try:
        parsed = json.loads(text)
    except ValueError:
        parsed = None
    if isinstance(parsed, list):
        return [str(item).strip() for item in parsed if str(item).strip()]
    return [item.strip() for item in text.translate(_STRIPPED).split(',') if item.strip()]


def ensure_xmllint() -> str | None:
    """Return an `xmllint` path, installing the package on Linux when absent."""

    found = shutil.which('xmllint')
    if found:
        return found
    apt_get = shutil.which('apt-get')
    if apt_get is None:
        print(
            '::error::xmllint is not installed and this runner has no apt-get',
            flush=True,
        )
        return None
    print(f'::group::Installing {XMMLINT_PACKAGE}', flush=True)
    try:
        subprocess.run(['sudo', apt_get, '-qq', 'update'], check=False)
        installed = subprocess.run(
            [
                'sudo',
                apt_get,
                'install',
                '--no-install-recommends',
                '-y',
                XMMLINT_PACKAGE,
            ],
            check=False,
        )
    finally:
        print('::endgroup::', flush=True)
    if installed.returncode != 0:
        print(f'::error::could not install {XMMLINT_PACKAGE}', flush=True)
        return None
    return shutil.which('xmllint')


def validate(changed_files: str, schema: str, fail_condition: str) -> int:
    """Validate every changed `.ui` file against the Qt schema."""

    files = parse_changed_files(changed_files)
    if not files:
        return 0
    xmllint = ensure_xmllint()
    if xmllint is None:
        return 1
    completed = subprocess.run(
        [xmllint, '--schema', schema, '--noout', *files],
        check=False,
    )
    if fail_condition == ADVISORY:
        return 0
    return completed.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = parser.add_subparsers(dest='command', required=True)

    guard_parser = subcommands.add_parser(
        'guard', help='fail a runner the Qt XML validator does not support'
    )
    guard_parser.add_argument(
        '--notice', required=True, help='the reason printed before the failure'
    )

    validate_parser = subcommands.add_parser(
        'validate', help='validate the changed Qt UI XML files'
    )
    validate_parser.add_argument(
        '--changed-files-json',
        default='',
        help='the check-changes output: a JSON list of paths',
    )
    validate_parser.add_argument(
        '--schema',
        default=DEFAULT_SCHEMA,
        help=f'the schema to validate against (default: {DEFAULT_SCHEMA})',
    )
    validate_parser.add_argument(
        '--fail-condition',
        default=ADVISORY,
        help="'never' reports without failing; anything else returns the check's status",
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'guard':
        return guard(arguments.notice)
    return validate(
        arguments.changed_files_json, arguments.schema, arguments.fail_condition
    )


if __name__ == '__main__':
    sys.exit(main())
