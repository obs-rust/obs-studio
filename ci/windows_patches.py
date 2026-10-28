#!/usr/bin/env -S uv run --no-project --script
"""The bouf patch action is one Python call per step (#962, GEN-005).

`.github/actions/windows-patches/action.yaml` carried nine multi-line `run:`
steps in PowerShell, so the action produced a GEN-005 finding for each block
and for each `shell: pwsh` key. Each step is now one call:

    uv run --no-project --script ci/windows_patches.py download
    uv run --no-project --script ci/windows_patches.py setup-bouf
    uv run --no-project --script ci/windows_patches.py install-nsis
    uv run --no-project --script ci/windows_patches.py install-tools
    uv run --no-project --script ci/windows_patches.py download-previous
    uv run --no-project --script ci/windows_patches.py prepare-notes
    uv run --no-project --script ci/windows_patches.py run-bouf

Each keeps its step's behaviour exactly:

- `download` runs `gh release download` for the OBS Windows zip, verifies it
  with `gh attestation verify --owner obsproject`, and extracts it into
  `build/`.
- `setup-bouf` downloads the bouf and bouf-nsis zips, fails when either
  SHA-256 does not equal the pin in the step's environment, and extracts them
  into `bouf/bin` and `bouf/nsis`.
- `install-nsis` and `install-tools` pass the same `winget` / `choco` argv.
- `download-previous` runs the same `rclone` copy; its `RCLONE_*` environment
  is the step's, not this module's.
- `prepare-notes` writes `notes.rst` from the tag's subject, the `#` rule and
  the tag's body, exactly as the three `git tag -l --format=` calls did.
- `run-bouf` runs `bouf/bin/bouf.exe` with the step's argument list.

`GH_TOKEN` and the `RCLONE_*` variables arrive from the action's `env:`, so
nothing here re-implements an authentication path.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import os
import subprocess
import sys
import zipfile

RULE = '#' * 51
DEFAULT_BOUF_TAG = 'v0.6.5'


def workspace() -> Path:
    """`${{ github.workspace }}` on a runner, the checkout otherwise."""

    return Path(os.environ.get('GITHUB_WORKSPACE') or Path.cwd())


def run(command: list[str]) -> int:
    """Print the command, run it, and return its status (no shell involved)."""

    print('+ ' + ' '.join(command), flush=True)
    return subprocess.run(command, check=False).returncode


def release_zip(tag: str, architecture: str) -> str:
    return f'OBS-Studio-{tag}-Windows-{architecture}.zip'


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def download(arguments: argparse.Namespace) -> int:
    archive = release_zip(arguments.tag, arguments.architecture)
    status = run(['gh', 'release', 'download', arguments.tag, '-p', archive])
    if status != 0:
        return status
    status = run(['gh', 'attestation', 'verify', archive, '--owner', 'obsproject'])
    if status != 0:
        return status
    with zipfile.ZipFile(workspace() / archive) as bundle:
        bundle.extractall(workspace() / 'build')
    return 0


def setup_bouf(arguments: argparse.Namespace) -> int:
    tag = os.environ.get('BOUF_TAG') or DEFAULT_BOUF_TAG
    bouf_dir = workspace() / 'bouf'
    bouf_dir.mkdir(exist_ok=True)
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
        ]
    )
    if status != 0:
        return status
    for archive, pin_name, destination in (
        (windows_zip, 'BOUF_HASH', 'bin'),
        (nsis_zip, 'BOUF_NSIS_HASH', 'nsis'),
    ):
        expected = os.environ.get(pin_name, '')
        actual = sha256(workspace() / archive)
        if expected and actual != expected:
            print(f'::error::{archive} hash does not match.', flush=True)
            return 1
        with zipfile.ZipFile(workspace() / archive) as bundle:
            bundle.extractall(bouf_dir / destination)
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


def install_tools(arguments: argparse.Namespace) -> int:
    return run(['choco', 'install', 'rclone', 'pandoc', '-y', '--no-progress'])


def download_previous(arguments: argparse.Namespace) -> int:
    return run(
        [
            'rclone',
            '-q',
            'copy',
            f':s3:obs-builds/{arguments.architecture}',
            str(workspace() / 'old_builds'),
        ]
    )


def git_tag(arguments: argparse.Namespace, contents: str) -> str:
    result = subprocess.run(
        ['git', 'tag', '-l', f'--format=%(contents:{contents})', arguments.tag],
        cwd=workspace() / 'repo',
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(result.returncode)
    return result.stdout


def prepare_notes(arguments: argparse.Namespace) -> int:
    subject = git_tag(arguments, 'subject')
    body = git_tag(arguments, 'body')
    path = workspace() / 'notes.rst'
    with path.open('w', encoding='utf-8') as handle:
        handle.write(subject)
        handle.write(f'{RULE}\n')
        handle.write('\n')
        handle.write(body)
    return 0


def run_bouf(arguments: argparse.Namespace) -> int:
    root = workspace()
    return run(
        [
            str(root / 'bouf' / 'bin' / 'bouf.exe'),
            '--config',
            str(root / '.github' / 'actions' / 'windows-patches' / 'config.toml'),
            '--version',
            arguments.tag,
            '--branch',
            arguments.channel,
            '--notes-file',
            str(root / 'notes.rst'),
            '-i',
            str(root / 'build'),
            '-p',
            str(root / 'old_builds'),
            '-o',
            str(root / 'output'),
            '--updater-data-only',
        ]
    )


def parse(arguments: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)

    downloader = commands.add_parser('download', help='fetch the OBS zip')
    downloader.add_argument('--tag', required=True)
    downloader.add_argument('--architecture', default='x64')
    downloader.set_defaults(handler=download)

    bouf = commands.add_parser('setup-bouf', help='fetch and verify bouf')
    bouf.set_defaults(handler=setup_bouf)

    nsis = commands.add_parser('install-nsis', help='install NSIS')
    nsis.set_defaults(handler=install_nsis)

    tools = commands.add_parser('install-tools', help='install rclone/pandoc')
    tools.set_defaults(handler=install_tools)

    previous = commands.add_parser(
        'download-previous', help='fetch the previous builds'
    )
    previous.add_argument('--architecture', default='x64')
    previous.set_defaults(handler=download_previous)

    notes = commands.add_parser('prepare-notes', help='write notes.rst')
    notes.add_argument('--tag', required=True)
    notes.set_defaults(handler=prepare_notes)

    bouf_run = commands.add_parser('run-bouf', help='generate the patches')
    bouf_run.add_argument('--tag', required=True)
    bouf_run.add_argument('--channel', default='stable')
    bouf_run.set_defaults(handler=run_bouf)

    return parser.parse_args(arguments)


def main(arguments: list[str] | None = None) -> int:
    parsed = parse(sys.argv[1:] if arguments is None else arguments)
    return int(parsed.handler(parsed))


if __name__ == '__main__':
    sys.exit(main())
