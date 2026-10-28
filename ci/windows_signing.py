#!/usr/bin/env -S uv run --no-project --script
"""The Windows signing action is one Python call per step (#962, GEN-005).

`.github/actions/windows-signing/action.yaml` carried ten multi-line `run:`
steps in PowerShell, so the action produced a GEN-005 finding and the three
`*.ps1` helpers only it sourced produced three more. Each step is now one
call:

    uv run --no-project --script ci/windows_signing.py extract-artifact
    uv run --no-project --script ci/windows_signing.py setup-bouf
    uv run --no-project --script ci/windows_signing.py install-nsis
    uv run --no-project --script ci/windows_signing.py setup-cng
    uv run --no-project --script ci/windows_signing.py install-rclone
    uv run --no-project --script ci/windows_signing.py download-previous
    uv run --no-project --script ci/windows_signing.py sign-game-capture
    uv run --no-project --script ci/windows_signing.py run-bouf
    uv run --no-project --script ci/windows_signing.py sync-latest
    uv run --no-project --script ci/windows_signing.py upload-archive

Each keeps its step's behaviour exactly, including the working directory the
old `Ensure-Location` helper used to set:

- `extract-artifact` extracts every `build/*.zip` into `build/` and deletes it.
- `setup-bouf` downloads the bouf and bouf-nsis zips into `bouf/`, fails when
  either SHA-256 differs from the pin in the step's environment, and extracts
  them into `bouf/bin` and `bouf/nsis`.
- `install-nsis` and `install-rclone` pass the same `winget` / `choco` argv.
- `setup-cng` downloads the Google CNG provider into `gcng/`, extracts it,
  verifies the MSI against `cng-release-signing-key.pem` with `openssl dgst`,
  and installs it with `msiexec`.
- `download-previous` / `sync-latest` / `upload-archive` run the same
  `rclone` argv. Their `RCLONE_*` environment is the step's, not this
  module's, so nothing here re-implements an authentication path.
- `sign-game-capture` signs the win-capture DLLs with the KMS-backed cert.
  `SIGNTOOL` overrides the Windows SDK's `signtool.exe` path (the tests use
  that to stub it).
- `run-bouf` runs `bouf/bin/bouf.exe` with the step's argument list.

`GH_TOKEN` arrives from the action's `env:`, as before.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BOUF_TAG = 'v0.6.6'
DEFAULT_CNG_TAG = 'cng-v1.1'
SIGNING_KEY = 'repo/.github/actions/windows-signing/prod-gc.crt'
KMS_KEY = (
    'projects/ci-signing/locations/global/keyRings/production/cryptoKeys/'
    'game-capture-release-sign-hsm/cryptoKeyVersions/1'
)
SIGNTOOL_DEFAULT = str(
    Path('C:/Program Files (x86)/Windows Kits/10/App Certification Kit/signtool.exe')
)


def workspace() -> Path:
    """`${{ github.workspace }}` on a runner, the checkout otherwise."""

    return Path(os.environ.get('GITHUB_WORKSPACE') or Path.cwd())


def action_dir() -> Path:
    """`${{ github.action_path }}` on a runner, the checkout otherwise."""

    configured = os.environ.get('GITHUB_ACTION_PATH')
    return Path(configured) if configured else ROOT / '.github/actions/windows-signing'


def run(command: list[str], cwd: Path | None = None) -> int:
    """Print the command, run it, and return its status (no shell involved)."""

    print('+ ' + ' '.join(command), flush=True)
    return subprocess.run(command, check=False, cwd=cwd).returncode


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def extract(archive: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as bundle:
        bundle.extractall(destination)


def extract_artifact(arguments: argparse.Namespace) -> int:
    build = workspace() / 'build'
    for archive in sorted(build.glob('*.zip')):
        extract(archive, build)
        archive.unlink()
    return 0


def setup_bouf(arguments: argparse.Namespace) -> int:
    tag = os.environ.get('BOUF_TAG') or DEFAULT_BOUF_TAG
    bouf_dir = workspace() / 'bouf'
    bouf_dir.mkdir(parents=True, exist_ok=True)
    windows_zip = f'bouf-windows-{tag}.zip'
    nsis_zip = f'bouf-nsis-{tag}.zip'
    status = run(
        [
            'gh',
            'release',
            'download',
            tag,
            '-R',
            'obsproject/bouf',
            '-p',
            windows_zip,
            '-p',
            nsis_zip,
        ],
        cwd=bouf_dir,
    )
    if status != 0:
        return status
    for archive, pin_name, destination in (
        (windows_zip, 'BOUF_HASH', 'bin'),
        (nsis_zip, 'BOUF_NSIS_HASH', 'nsis'),
    ):
        path = bouf_dir / archive
        expected = os.environ.get(pin_name, '')
        if expected:
            actual = sha256(path)
            if actual != expected:
                print(f'::error::{archive} hash does not match.', flush=True)
                return 1
        extract(path, bouf_dir / destination)
    return 0


def install_nsis(arguments: argparse.Namespace) -> int:
    return run(
        [
            'winget',
            'install',
            '--silent',
            '--accept-package-agreements',
            '--accept-source-agreements',
            '--disable-interactivity',
            '-e',
            '--id',
            'NSIS.NSIS',
        ]
    )


def setup_cng(arguments: argparse.Namespace) -> int:
    tag = os.environ.get('CNG_TAG') or DEFAULT_CNG_TAG
    cng_dir = workspace() / 'gcng'
    cng_dir.mkdir(parents=True, exist_ok=True)
    status = run(
        [
            'gh',
            'release',
            'download',
            tag,
            '-R',
            'GoogleCloudPlatform/kms-integrations',
            '-p',
            '*amd64.zip',
        ],
        cwd=cng_dir,
    )
    if status != 0:
        return status
    for archive in sorted(cng_dir.glob('*.zip')):
        extract(archive, cng_dir)
    signatures = sorted(cng_dir.rglob('*.sig'))
    installers = sorted(cng_dir.rglob('*.msi'))
    if not signatures or not installers:
        print('::error::the CNG provider release has no .sig/.msi pair.', flush=True)
        return 1
    key = action_dir() / 'cng-release-signing-key.pem'
    status = run(
        [
            'openssl',
            'dgst',
            '-sha384',
            '-verify',
            str(key),
            '-signature',
            str(signatures[0]),
            str(installers[0]),
        ]
    )
    if status != 0:
        return status
    return run(['msiexec', '/i', str(installers[0]), '/qn', '/norestart'])


def install_rclone(arguments: argparse.Namespace) -> int:
    return run(
        ['choco', 'install', 'rclone', '--version=1.69.3', '-y', '--no-progress']
    )


def download_previous(arguments: argparse.Namespace) -> int:
    old_builds = workspace() / 'old_builds'
    old_builds.mkdir(parents=True, exist_ok=True)
    remote = f':gcs:obs-latest/{arguments.architecture}/{arguments.channel}'
    return run(
        ['rclone', 'copy', '--transfers', '100', remote, '.'],
        cwd=old_builds,
    )


def sign_game_capture(arguments: argparse.Namespace) -> int:
    signtool = os.environ.get('SIGNTOOL') or SIGNTOOL_DEFAULT
    dlls = str(workspace() / 'build' / 'core' / 'win-capture' / 'data' / '*.dll')
    return run(
        [
            signtool,
            'sign',
            '/fd',
            'sha256',
            '/t',
            'http://timestamp.digicert.com',
            '/f',
            SIGNING_KEY,
            '/csp',
            'Google Cloud KMS Provider',
            '/kc',
            KMS_KEY,
            dlls,
        ]
    )


def run_bouf(arguments: argparse.Namespace) -> int:
    if not arguments.version:
        print('::error::--version is required for run-bouf.', flush=True)
        return 2
    bouf = workspace() / 'bouf' / 'bin' / 'bouf.exe'
    config = action_dir() / f'config_{arguments.architecture}.toml'
    return run(
        [
            str(bouf),
            '--config',
            str(config),
            '--version',
            arguments.version,
            '--branch',
            arguments.channel,
            '-i',
            str(workspace() / 'build'),
            '-p',
            str(workspace() / 'old_builds'),
            '-o',
            str(workspace() / 'output'),
            '--packaging-only',
        ]
    )


def sync_latest(arguments: argparse.Namespace) -> int:
    remote = f':gcs:obs-latest/{arguments.architecture}/{arguments.channel}'
    return run(
        [
            'rclone',
            'sync',
            '--delete-excluded',
            '--transfers',
            '100',
            str(workspace() / 'old_builds'),
            remote,
        ]
    )


def upload_archive(arguments: argparse.Namespace) -> int:
    remote = f':gcs:obs-builds/{arguments.architecture}'
    return run(
        [
            'rclone',
            'copy',
            '--transfers',
            '100',
            str(workspace() / 'old_builds'),
            remote,
        ]
    )


def parser() -> argparse.ArgumentParser:
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument('--architecture', default='x64')
    common.add_argument('--channel', default='stable')
    root = argparse.ArgumentParser(description=__doc__)
    steps = root.add_subparsers(dest='step', required=True)
    steps.add_parser('extract-artifact').set_defaults(handler=extract_artifact)
    steps.add_parser('setup-bouf').set_defaults(handler=setup_bouf)
    steps.add_parser('install-nsis').set_defaults(handler=install_nsis)
    steps.add_parser('setup-cng').set_defaults(handler=setup_cng)
    steps.add_parser('install-rclone').set_defaults(handler=install_rclone)
    steps.add_parser('download-previous', parents=[common]).set_defaults(
        handler=download_previous
    )
    steps.add_parser('sign-game-capture').set_defaults(handler=sign_game_capture)
    run_bouf_step = steps.add_parser('run-bouf', parents=[common])
    run_bouf_step.add_argument('--version', default='')
    run_bouf_step.set_defaults(handler=run_bouf)
    steps.add_parser('sync-latest', parents=[common]).set_defaults(
        handler=sync_latest
    )
    steps.add_parser('upload-archive', parents=[common]).set_defaults(
        handler=upload_archive
    )
    return root


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    return int(arguments.handler(arguments))


if __name__ == '__main__':
    sys.exit(main())
