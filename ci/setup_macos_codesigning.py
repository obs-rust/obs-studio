#!/usr/bin/env -S uv run --no-project --script
"""The macOS code-signing action is one Python call per step (#962, GEN-005).

`.github/actions/setup-macos-codesigning/action.yaml` carried four multi-line
`run:` steps -- the non-macOS guard, the keychain/certificate import, the
provisioning-profile import and the notarization flag -- so the action produced
a GEN-005 finding for each. Each step is now a single call:

    uv run --no-project --script ci/setup_macos_codesigning.py guard

    uv run --no-project --script ci/setup_macos_codesigning.py codesign

    uv run --no-project --script ci/setup_macos_codesigning.py provisioning \\
        --codesign-team "${{ steps.codesign.outputs.codesignTeam }}"

    uv run --no-project --script ci/setup_macos_codesigning.py notarization

Each keeps its step's behaviour exactly:

- `guard` prints the reason and exits 2; the action's own
  `if: runner.os != 'macOS'` still selects it.
- `codesign` writes `haveCodesignIdent`, then, when an identity and a
  certificate are present, decodes the PKCS12 into `${RUNNER_TEMP}`, creates
  and unlocks a temporary keychain, imports the certificate with the same
  `security` arguments (including the three `-T` tools), writes the keychain
  password to `${GITHUB_ENV}`, and writes `codesignIdent` and `codesignTeam`
  (the identity's trailing team, parentheses removed). Without credentials it
  writes the three `false` flags every later step reads.
- `provisioning` decodes the profile, installs it into the user's profiles
  directory after reading its UUID and team with `plutil`, warns through a
  `::notice::` when the profile's team differs from the certificate's, and
  writes `provisioningProfileUUID`.
- `notarization` writes `haveNotarizationUser` from the presence of both
  credentials.

The zsh `${${IDENTITY##* }//(\\(|\\))/}` team extraction, the `print ${RANDOM}
| shasum | head -c 32` keychain password and the `base64 --decode` pipelines
are the only shell constructs the steps used; everything else was already a
sequence of external commands, which this module runs with `subprocess`.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import os
from pathlib import Path
import random
import subprocess
import sys

UNSUPPORTED = 'setup-macos-codesigning action requires a macOS-based runner.'
PROFILE_DIRECTORIES = ('Library', 'MobileDevice', 'Provisioning Profiles')


class Failure(Exception):
    """A command the step cannot continue without returned a non-zero status."""

    def __init__(self, status: int) -> None:
        self.status = status
        super().__init__(f'a command failed with status {status}')


def fail(message: str) -> None:
    print(f'::error::{message}', flush=True)


def append(variable: str, line: str) -> None:
    """Append one line to a GitHub file such as `$GITHUB_OUTPUT`."""

    path = os.environ.get(variable, '')
    if not path:
        fail(f'{variable} is not set; this step must run inside a workflow')
        raise Failure(2)
    with Path(path).open('a', encoding='utf-8') as handle:
        handle.write(f'{line}\n')


def set_output(name: str, value: str) -> None:
    append('GITHUB_OUTPUT', f'{name}={value}')


def set_environment(name: str, value: str) -> None:
    append('GITHUB_ENV', f'{name}={value}')


def run(
    command: list[str], *, capture: bool = False
) -> subprocess.CompletedProcess[str]:
    """Run one external command; `RUNNER_DEBUG` traces it like zsh's XTRACE."""

    if os.environ.get('RUNNER_DEBUG'):
        print(f'+ {" ".join(command)}', flush=True)
    return subprocess.run(
        command, capture_output=capture, text=True, check=False
    )


def require(
    completed: subprocess.CompletedProcess[str], description: str
) -> subprocess.CompletedProcess[str]:
    if completed.returncode:
        fail(f'{description} failed with status {completed.returncode}')
        raise Failure(completed.returncode)
    return completed


def decode(encoded: str, destination: Path) -> None:
    destination.write_bytes(base64.b64decode(encoded))


def team_from_identity(identity: str) -> str:
    """`Developer ID Application: Example Corp (ABCDE12345)` -> `ABCDE12345`."""

    return identity.rsplit(' ', 1)[-1].replace('(', '').replace(')', '')


def random_keychain_password() -> str:
    """The step's `print ${RANDOM} | shasum | head -c 32`."""

    draw = random.SystemRandom().randrange(0, 32768)
    return hashlib.sha1(str(draw).encode()).hexdigest()[:32]


def guard() -> int:
    """Fail a runner this action does not support, with the reason in the log."""

    print(UNSUPPORTED, flush=True)
    return 2


def keychain(
    path: Path, password: str, certificate: Path, certificate_password: str
) -> None:
    """Create, unlock and import into the temporary keychain, in the old order."""

    for command in (
        ['security', 'create-keychain', '-p', password, str(path)],
        ['security', 'set-keychain-settings', '-lut', '21600', str(path)],
        ['security', 'unlock-keychain', '-p', password, str(path)],
        [
            'security',
            'import',
            str(certificate),
            '-P',
            certificate_password,
            '-A',
            '-t',
            'cert',
            '-f',
            'pkcs12',
            '-k',
            str(path),
            '-T',
            '/usr/bin/codesign',
            '-T',
            '/usr/bin/security',
            '-T',
            '/usr/bin/xcrun',
        ],
        [
            'security',
            'set-key-partition-list',
            '-S',
            'apple-tool:,apple:',
            '-k',
            password,
            str(path),
        ],
        [
            'security',
            'list-keychain',
            '-d',
            'user',
            '-s',
            str(path),
            'login-keychain',
        ],
    ):
        # The partition-list call was redirected to /dev/null in the step; the
        # status is what the step observed, so its output is only captured.
        capture = command[1] == 'set-key-partition-list'
        require(run(command, capture=capture), f'security {command[1]}')


def codesign() -> int:
    identity = os.environ.get('MACOS_SIGNING_IDENTITY', '')
    certificate = os.environ.get('MACOS_SIGNING_CERT', '')
    if not (identity and certificate):
        set_output('haveCodesignIdent', 'false')
        set_output('haveProvisioningProfile', 'false')
        set_output('haveNotarizationUser', 'false')
        return 0

    set_output('haveCodesignIdent', 'true')

    runner_temp = Path(os.environ['RUNNER_TEMP'])
    certificate_path = runner_temp / 'build_certificate.p12'
    keychain_path = runner_temp / 'app-signing.keychain-db'
    decode(certificate, certificate_path)

    password = os.environ.get('MACOS_KEYCHAIN_PASSWORD') or random_keychain_password()

    print('::group::Keychain setup', flush=True)
    try:
        keychain(
            keychain_path,
            password,
            certificate_path,
            os.environ.get('MACOS_SIGNING_CERT_PASSWORD', ''),
        )
    finally:
        print('::endgroup::', flush=True)

    set_output('codesignIdent', identity)
    set_environment('MACOS_KEYCHAIN_PASSWORD', password)
    set_output('codesignTeam', team_from_identity(identity))
    return 0


def extract_plist_value(path: Path, key: str, *extra: str) -> str:
    completed = require(
        run(['plutil', '-extract', key, 'raw', *extra, str(path)], capture=True),
        f'plutil -extract {key}',
    )
    return completed.stdout.strip()


def install_profile(profile: Path, plist: Path, codesign_team: str) -> str:
    """Read the profile's UUID and team, install it, and return the UUID."""

    directory = Path.home().joinpath(*PROFILE_DIRECTORIES)
    directory.mkdir(parents=True, exist_ok=True)

    require(
        run(['security', 'cms', '-D', '-i', str(profile), '-o', str(plist)]),
        'security cms',
    )
    uuid = extract_plist_value(plist, 'UUID')
    team = extract_plist_value(plist, 'TeamIdentifier.0', '-expect', 'string')
    if codesign_team and team != codesign_team:
        print(
            '::notice::Code Signing team in provisioning profile does not'
            ' match certificate.',
            flush=True,
        )

    (directory / f'{uuid}.provisionprofile').write_bytes(profile.read_bytes())
    return uuid


def provisioning(codesign_team: str) -> int:
    encoded = os.environ.get('MACOS_SIGNING_PROVISIONING_PROFILE', '')
    if not encoded:
        set_output('haveProvisioningProfile', 'false')
        return 0

    set_output('haveProvisioningProfile', 'true')

    runner_temp = Path(os.environ['RUNNER_TEMP'])
    profile_path = runner_temp / 'build_profile.provisionprofile'
    decode(encoded, profile_path)
    plist_path = runner_temp / 'build_profile.plist'

    print('::group::Provisioning Profile Setup', flush=True)
    try:
        uuid = install_profile(profile_path, plist_path, codesign_team)
    finally:
        print('::endgroup::', flush=True)

    set_output('provisioningProfileUUID', uuid)
    return 0


def notarization() -> int:
    present = bool(os.environ.get('MACOS_NOTARIZATION_USERNAME')) and bool(
        os.environ.get('MACOS_NOTARIZATION_PASSWORD')
    )
    set_output('haveNotarizationUser', 'true' if present else 'false')
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = parser.add_subparsers(dest='command', required=True)

    subcommands.add_parser(
        'guard', help='fail a runner the code-signing action does not support'
    )
    subcommands.add_parser(
        'codesign', help='import the signing certificate into a temporary keychain'
    )
    provisioning_parser = subcommands.add_parser(
        'provisioning', help='install the provisioning profile and record its UUID'
    )
    provisioning_parser.add_argument(
        '--codesign-team',
        default='',
        help='the certificate team id, to notice a profile from another team',
    )
    subcommands.add_parser(
        'notarization', help='record whether notarization credentials are present'
    )

    arguments = parser.parse_args(argv)
    if arguments.command == 'guard':
        return guard()
    try:
        if arguments.command == 'codesign':
            return codesign()
        if arguments.command == 'provisioning':
            return provisioning(arguments.codesign_team)
        return notarization()
    except Failure as failure:
        return failure.status


if __name__ == '__main__':
    sys.exit(main())
