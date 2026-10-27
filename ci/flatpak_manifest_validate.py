#!/usr/bin/env -S uv run --no-project --script
"""The Flatpak manifest validator is one Python call per step (#962, GEN-005).

`.github/actions/flatpak-manifest-validator/action.yaml` carried two multi-line
`run:` steps -- the Windows runner guard and the manifest order check -- so the
action produced a GEN-005 finding for each. Each step is now a single call:

    uv run --no-project --script ci/flatpak_manifest_validate.py guard \\
        --notice "..."
    uv run --no-project --script ci/flatpak_manifest_validate.py validate \\
        --manifest build-aux/io.github.obs_rust.OBSStudioRust.json \\
        --fail-condition error

`guard` keeps the step's behaviour: the action's own
`if: runner.os == 'Windows'` selects it, it prints the notice, and it exits 2
so an unsupported runner fails the job instead of running the check.

`validate` runs `build-aux/format-manifest.py --check` (standard library only)
and returns its status. `--fail-condition never` -- the action input's default
-- reproduces the old step's `set +e`: the report is printed, the step passes.
The run is bracketed by `::group::` / `::endgroup::` annotations so the CI log
collapses the formatter output the same way.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FORMATTER = ROOT / 'build-aux' / 'format-manifest.py'
DEFAULT_MANIFEST = 'build-aux/io.github.obs_rust.OBSStudioRust.json'
ADVISORY = 'never'


def guard(notice: str) -> int:
    """Fail a runner this action does not support, with the reason in the log."""

    print(f'::notice::{notice}', flush=True)
    return 2


def validate(manifest: str, fail_condition: str) -> int:
    """Run the manifest formatter in check mode; its status unless advisory."""

    print('::group::Run Validation', flush=True)
    try:
        completed = subprocess.run(
            [
                sys.executable,
                str(FORMATTER),
                manifest,
                '--check',
                '--loglevel',
                'INFO',
            ],
            check=False,
        )
    finally:
        print('::endgroup::', flush=True)
    if fail_condition == ADVISORY:
        return 0
    return completed.returncode


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = parser.add_subparsers(dest='command', required=True)

    guard_parser = subcommands.add_parser(
        'guard', help='fail a runner the Flatpak manifest validator does not support'
    )
    guard_parser.add_argument(
        '--notice', required=True, help='the reason printed before the failure'
    )

    validate_parser = subcommands.add_parser(
        'validate', help='check the Flatpak manifest module order'
    )
    validate_parser.add_argument(
        '--manifest',
        default=DEFAULT_MANIFEST,
        help=f'the manifest to check (default: {DEFAULT_MANIFEST})',
    )
    validate_parser.add_argument(
        '--fail-condition',
        default=ADVISORY,
        help="'never' reports without failing; anything else returns the check's status",
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'guard':
        return guard(arguments.notice)
    return validate(arguments.manifest, arguments.fail_condition)


if __name__ == '__main__':
    sys.exit(main())
