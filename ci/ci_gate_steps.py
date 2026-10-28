#!/usr/bin/env -S uv run --no-project --script
"""The CI Gate workflow's four multi-line steps are one Python call each (#2851).

`.github/workflows/ci.yml` carried four `run:` blocks, so `ci-lint precheck`
reported a GEN-005 finding for the path. Each step is now a single call:

    uv run --no-project --script ci/ci_gate_steps.py record-export-provenance
    uv run --no-project --script ci/ci_gate_steps.py install-runtime-deps
    uv run --no-project --script ci/ci_gate_steps.py run-saved-capture-profile
    uv run --no-project --script ci/ci_gate_steps.py validate-prepared-suites

The first three are `flatpak-runtime-reuse`'s steps, in order:

- `record-export-provenance` is `Record Export Provenance`: it rejects a
  `SOURCE_RUN` that is not all digits with the step's own message and exit 1,
  asks `gh api repos/$GITHUB_REPOSITORY/actions/runs/$SOURCE_RUN --jq .head_sha`
  for the commit, rejects anything that is not 40 lower-case hex characters, and
  then appends `FLATPAK_ARTIFACT_SOURCE` and `FLATPAK_ARTIFACT_RUN` to
  `$GITHUB_ENV`. `GH_TOKEN` stays in the step's `env:`.
- `install-runtime-deps` is `Install Runtime Test Infrastructure`: the host apt
  mirror preference under `sudo -E` (through the runner's own uv), then the two
  `apt-get` invocations with their retry and timeout options and the same
  package list, run as root.
- `run-saved-capture-profile` is `Run Installed Flatpak with Saved Capture
  Profile`: `tar -xf` the downloaded OSTree repo into `$RUNNER_TEMP`, then
  `ci/flatpak_runtime.py --repo … --output …` from the runner's uv.

The fourth is `tier2-setup-consumer`'s `Validate Prepared OFF and ON Suites`:
extract the prepared tree with `tar --zstd`, read the image id out of the
downloaded `bundle.json`, and run one container of
`rust/tools/linux-validate/validate.sh` with `OBS_TIER2_MODES=OFF ON`. The uv
bind mount is resolved on the host, exactly as `$(command -v uv)` was.

Every argv is the step's argv and each step's exit status is the module's.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI = ROOT / 'ci'
APT_OPTIONS = (
    '-o',
    'Acquire::Retries=1',
    '-o',
    'Acquire::http::Timeout=15',
    '-o',
    'Acquire::https::Timeout=15',
)
PACKAGES = (
    'flatpak',
    'xvfb',
    'dbus-daemon',
    'pipewire',
    'wireplumber',
    'pipewire-pulse',
    'pipewire-bin',
    'pulseaudio-utils',
    'gstreamer1.0-tools',
    'gstreamer1.0-plugins-base',
    'gstreamer1.0-pipewire',
    'libgl1-mesa-dri',
    'gcc',
    'pkg-config',
    'libglib2.0-dev',
    'libpipewire-0.3-dev',
    'libpulse-dev',
    'libcurl4-openssl-dev',
    'ffmpeg',
    'gdb',
    'binutils',
)
RUN_ID = re.compile(r'[0-9]+')
SHA = re.compile(r'[0-9a-f]{40}')
IMAGE_SOURCE = '/__w/obs-studio/obs-studio'
BUILD_DIR = '/__w/_temp/tier2'


def announce(argv: Sequence[str]) -> None:
    """The steps run under `set -x` semantics when `RUNNER_DEBUG` is set."""

    if os.environ.get('RUNNER_DEBUG'):
        print(f'::debug::{shlex.join(argv)}', flush=True)


def run(argv: Sequence[str]) -> int:
    """One child, its exit status returned to the step unchanged."""

    announce(argv)
    return subprocess.run(argv, check=False).returncode


def fail(message: str) -> int:
    print(message, file=sys.stderr, flush=True)
    return 1


def required(name: str) -> str | None:
    value = os.environ.get(name)
    if not value:
        return None
    return value


def append_environment(pairs: Sequence[str]) -> None:
    """`>> $GITHUB_ENV`, the way each shell step wrote it."""

    path = os.environ.get('GITHUB_ENV')
    if not path:
        return
    with open(path, 'a', encoding='utf-8') as handle:
        for pair in pairs:
            handle.write(f'{pair}\n')


def record_export_provenance() -> int:
    source_run = os.environ.get('SOURCE_RUN', '')
    if not RUN_ID.fullmatch(source_run):
        print('Expected a numeric same-repository run ID')
        return 1
    repository = required('GITHUB_REPOSITORY')
    if repository is None:
        return fail('GITHUB_REPOSITORY is not set')
    argv = [
        'gh',
        'api',
        f'repos/{repository}/actions/runs/{source_run}',
        '--jq',
        '.head_sha',
    ]
    announce(argv)
    done = subprocess.run(argv, capture_output=True, text=True, check=False)
    if done.returncode:
        sys.stderr.write(done.stderr)
        sys.stderr.flush()
        return done.returncode
    source_sha = done.stdout.strip()
    if not SHA.fullmatch(source_sha):
        return 1
    append_environment(
        [f'FLATPAK_ARTIFACT_SOURCE={source_sha}', f'FLATPAK_ARTIFACT_RUN={source_run}']
    )
    return 0


def install_runtime_deps() -> int:
    uv = shutil.which('uv')
    if uv is None:
        return fail('uv is not on PATH')
    for argv in (
        ['sudo', '-E', uv, 'run', '--no-project', str(CI / 'host_apt_mirrors.py')],
        ['sudo', 'apt-get', *APT_OPTIONS, 'update', '-qq'],
        [
            'sudo',
            'apt-get',
            *APT_OPTIONS,
            'install',
            '-y',
            '--no-install-recommends',
            *PACKAGES,
        ],
    ):
        status = run(argv)
        if status:
            return status
    return 0


def run_saved_capture_profile() -> int:
    runner_temp = required('RUNNER_TEMP')
    if runner_temp is None:
        return fail('RUNNER_TEMP is not set')
    directory = Path(runner_temp) / 'flatpak-runtime-input'
    status = run(['tar', '-xf', str(directory / 'flatpak-runtime-repo.tar'), '-C', str(directory)])
    if status:
        return status
    uv = shutil.which('uv') or 'uv'
    return run(
        [
            uv,
            'run',
            '--no-project',
            str(CI / 'flatpak_runtime.py'),
            '--repo',
            str(directory / 'repo'),
            '--output',
            str(Path(runner_temp) / 'flatpak-runtime-result'),
        ]
    )


def validate_prepared_suites() -> int:
    runner_temp = required('RUNNER_TEMP')
    workspace = required('GITHUB_WORKSPACE')
    if runner_temp is None:
        return fail('RUNNER_TEMP is not set')
    if workspace is None:
        return fail('GITHUB_WORKSPACE is not set')
    tier2 = Path(runner_temp) / 'tier2'
    tier2.mkdir(parents=True, exist_ok=True)
    status = run(
        [
            'tar',
            '--zstd',
            '-xf',
            str(Path(runner_temp) / 'tier2-input' / 'tier2-tree.tar.zst'),
            '-C',
            str(tier2),
        ]
    )
    if status:
        return status
    bundle = Path(runner_temp) / 'setup-bundle' / 'bundle.json'
    try:
        image = json.loads(bundle.read_text(encoding='utf-8'))['image_id']
    except (OSError, ValueError, KeyError) as error:
        return fail(f'cannot read {bundle}: {error}')
    uv = shutil.which('uv') or 'uv'
    return run(
        [
            'docker',
            'run',
            '--cpus',
            '4',
            '--mount',
            f'type=bind,src={workspace},dst={IMAGE_SOURCE},readonly',
            '--mount',
            f'type=bind,src={tier2},dst={BUILD_DIR}',
            '--mount',
            f'type=bind,src={uv},dst=/usr/local/bin/uv,readonly',
            '--env',
            f'OBS_TIER2_SRC={IMAGE_SOURCE}',
            '--env',
            f'OBS_TIER2_BUILD={BUILD_DIR}',
            '--env',
            'OBS_TIER2_STAGE=test',
            '--env',
            'OBS_TIER2_MODES=OFF ON',
            '--env',
            'OBS_CTEST_JOBS=8',
            image,
            'bash',
            str(Path(workspace) / 'rust' / 'tools' / 'linux-validate' / 'validate.sh'),
        ]
    )


COMMANDS = {
    'record-export-provenance': record_export_provenance,
    'install-runtime-deps': install_runtime_deps,
    'run-saved-capture-profile': run_saved_capture_profile,
    'validate-prepared-suites': validate_prepared_suites,
}


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=sorted(COMMANDS))
    arguments = parser.parse_args(argv)
    return COMMANDS[arguments.command]()


if __name__ == '__main__':
    sys.exit(main())
