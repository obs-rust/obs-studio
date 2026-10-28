#!/usr/bin/env -S uv run --script
"""The ci.yml workflow's four multi-line steps are one Python call each (#2851).

    uv run ci/test_ci_gate_steps.py

GEN-005 gives a workflow `run:` step a one-line shell budget. Four steps in
`.github/workflows/ci.yml` were over it, and all four are now
`ci/ci_gate_steps.py`:

- `flatpak-runtime-reuse`'s `Record Export Provenance`: the numeric run-id
  check, the `gh api ... --jq .head_sha` lookup, the 40-hex check, then
  `FLATPAK_ARTIFACT_SOURCE`/`FLATPAK_ARTIFACT_RUN` into `$GITHUB_ENV`.
- `flatpak-runtime-reuse`'s `Install Runtime Test Infrastructure`: the mirror
  preference under `sudo -E`, then the two `apt-get` invocations.
- `flatpak-runtime-reuse`'s `Run Installed Flatpak with Saved Capture Profile`:
  the `tar -xf` of the downloaded OSTree repo, then `ci/flatpak_runtime.py`.
- `tier2-setup-consumer`'s `Validate Prepared OFF and ON Suites`: the
  `tar --zstd` extraction, the `bundle.json` `image_id` read, and the
  `docker run` of `rust/tools/linux-validate/validate.sh`.

RED first: committed while all four steps were still shell blocks and the
module did not exist, so every test here failed then (nothing to import, `run: |`
still in the workflow, and the GEN-005 ``[[exceptions]]`` block still in
`ci.toml`).
"""

from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'ci_gate_steps.py'
WORKFLOW = ROOT / '.github' / 'workflows' / 'ci.yml'
CI_TOML = ROOT / 'ci.toml'
WORKFLOW_PATH = '.github/workflows/ci.yml'
CALL = 'uv run --no-project --script ${{ github.workspace }}/ci/ci_gate_steps.py'
STEPS = {
    'flatpak-runtime-reuse': (
        'record-export-provenance',
        'install-runtime-deps',
        'run-saved-capture-profile',
    ),
    'tier2-setup-consumer': ('validate-prepared-suites',),
}
PACKAGES = [
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
]
APT_OPTIONS = [
    '-o',
    'Acquire::Retries=1',
    '-o',
    'Acquire::http::Timeout=15',
    '-o',
    'Acquire::https::Timeout=15',
]
SHA = 'a' * 40

# One stub for every tool the module shells out to: it records one argv per
# `--END--` record (NUL-separated, so an argument with a space stays one
# argument) and prints canned stdout.
STUB = """#!/bin/sh
for argument in "$@"; do printf '%s\\0' "$argument"; done >> "$CALL_LOG"
printf '%s\\0' --END-- >> "$CALL_LOG"
cat "$CALL_STDOUT" 2>/dev/null
exit "${CALL_STATUS:-0}"
"""
TOOLS = ('gh', 'sudo', 'apt-get', 'tar', 'docker', 'uv')


def load_module():
    """ci/ci_gate_steps.py by path: no file here edits sys.path."""

    spec = importlib.util.spec_from_file_location('ci_gate_steps', MODULE)
    assert spec is not None and spec.loader is not None, f'cannot load {MODULE}'
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class Recorded:
    """One run of the module: its status, `$GITHUB_ENV`, argv list and tmp root."""

    def __init__(
        self,
        status: int,
        stdout: str,
        stderr: str,
        env: str,
        calls: list[list[str]],
        root: Path,
    ):
        self.status = status
        self.stdout = stdout
        self.stderr = stderr
        self.env = env
        self.calls = calls
        self.root = root


def record(
    command: str,
    stdout: str = '',
    status: int = 0,
    files: dict[str, str] | None = None,
    **environment: str,
) -> Recorded:
    """Run one subcommand with the stub tools first on `PATH`."""

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        bin_directory = root / 'bin'
        bin_directory.mkdir()
        for tool in TOOLS:
            stub = bin_directory / tool
            stub.write_text(STUB, encoding='utf-8')
            stub.chmod(0o755)
        recorded = root / 'calls'
        recorded.touch()
        canned = root / 'stdout'
        canned.write_text(stdout, encoding='utf-8')
        env_file = root / 'github-env'
        env_file.touch()
        for name, content in (files or {}).items():
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(content, encoding='utf-8')

        env = dict(os.environ)
        env.update(
            {
                'PATH': f'{bin_directory}{os.pathsep}{env.get("PATH", "")}',
                'CALL_LOG': str(recorded),
                'CALL_STDOUT': str(canned),
                'CALL_STATUS': str(status),
                'GITHUB_ENV': str(env_file),
                'RUNNER_TEMP': str(root),
                'GITHUB_WORKSPACE': str(root / 'workspace'),
            }
        )
        env.update(environment)
        done = subprocess.run(
            [sys.executable, str(MODULE), command],
            capture_output=True,
            text=True,
            env=env,
            check=False,
        )

        calls: list[list[str]] = []
        current: list[str] = []
        for field in recorded.read_text(encoding='utf-8').split('\0'):
            if field == '--END--':
                calls.append(current)
                current = []
            elif field:
                current.append(field)
        return Recorded(
            done.returncode,
            done.stdout,
            done.stderr,
            env_file.read_text(encoding='utf-8'),
            calls,
            root,
        )


class RecordExportProvenanceTest(unittest.TestCase):
    """`record-export-provenance` publishes the same two variables."""

    def test_valid_run_id_publishes_the_source_sha(self):
        run = record(
            'record-export-provenance',
            f'{SHA}\n',
            SOURCE_RUN='12345',
            GITHUB_REPOSITORY='obs-rust/obs-studio',
        )
        self.assertEqual(run.status, 0, run.stderr)
        self.assertEqual(
            run.calls,
            [['api', 'repos/obs-rust/obs-studio/actions/runs/12345', '--jq', '.head_sha']],
        )
        self.assertEqual(
            run.env,
            f'FLATPAK_ARTIFACT_SOURCE={SHA}\nFLATPAK_ARTIFACT_RUN=12345\n',
        )

    def test_non_numeric_run_id_fails_before_the_lookup(self):
        run = record('record-export-provenance', SOURCE_RUN='run-7', GITHUB_REPOSITORY='o/r')
        self.assertEqual(run.status, 1)
        self.assertEqual(run.calls, [])
        self.assertIn('Expected a numeric same-repository run ID', run.stdout + run.stderr)
        self.assertEqual(run.env, '')

    def test_short_sha_fails_without_publishing(self):
        run = record('record-export-provenance', 'abc\n', SOURCE_RUN='7', GITHUB_REPOSITORY='o/r')
        self.assertEqual(run.status, 1)
        self.assertEqual(run.env, '')

    def test_failing_gh_fails_the_step(self):
        run = record(
            'record-export-provenance',
            status=1,
            SOURCE_RUN='7',
            GITHUB_REPOSITORY='o/r',
        )
        self.assertEqual(run.status, 1)
        self.assertEqual(run.env, '')


class InstallRuntimeDepsTest(unittest.TestCase):
    """`install-runtime-deps` runs the mirror preference, then both apt calls."""

    def test_mirrors_then_update_then_install(self):
        run = record('install-runtime-deps')
        self.assertEqual(run.status, 0, run.stderr)
        self.assertEqual(len(run.calls), 3, run.calls)
        mirror, update, install = run.calls
        self.assertEqual(mirror[0], '-E')
        self.assertTrue(mirror[1].endswith('/uv'), mirror)
        self.assertEqual(
            mirror[2:],
            ['run', '--no-project', str(ROOT / 'ci' / 'host_apt_mirrors.py')],
        )
        self.assertEqual(update, ['apt-get', *APT_OPTIONS, 'update', '-qq'])
        self.assertEqual(
            install,
            ['apt-get', *APT_OPTIONS, 'install', '-y', '--no-install-recommends', *PACKAGES],
        )

    def test_a_failing_mirror_step_stops_the_packages(self):
        run = record('install-runtime-deps', status=2)
        self.assertEqual(run.status, 2)
        self.assertEqual(len(run.calls), 1, run.calls)

    def test_package_list_is_unchanged(self):
        self.assertEqual(list(load_module().PACKAGES), PACKAGES)


class RunSavedCaptureProfileTest(unittest.TestCase):
    """`run-saved-capture-profile` extracts the repo, then runs the harness."""

    def test_tar_then_flatpak_runtime(self):
        run = record('run-saved-capture-profile')
        self.assertEqual(run.status, 0, run.stderr)
        input_directory = run.root / 'flatpak-runtime-input'
        self.assertEqual(
            run.calls,
            [
                ['-xf', str(input_directory / 'flatpak-runtime-repo.tar'), '-C', str(input_directory)],
                [
                    'run',
                    '--no-project',
                    str(ROOT / 'ci' / 'flatpak_runtime.py'),
                    '--repo',
                    str(input_directory / 'repo'),
                    '--output',
                    str(run.root / 'flatpak-runtime-result'),
                ],
            ],
        )

    def test_failing_tar_stops_before_the_harness(self):
        run = record('run-saved-capture-profile', status=2)
        self.assertEqual(run.status, 2)
        self.assertEqual(len(run.calls), 1, run.calls)


class ValidatePreparedSuitesTest(unittest.TestCase):
    """`validate-prepared-suites` reads `bundle.json` and runs one container."""

    def test_docker_argv(self):
        run = record(
            'validate-prepared-suites',
            files={'setup-bundle/bundle.json': json.dumps({'image_id': 'sha256:deadbeef'})},
        )
        self.assertEqual(run.status, 0, run.stderr)
        self.assertEqual(len(run.calls), 2, run.calls)
        workspace = run.root / 'workspace'
        self.assertEqual(
            run.calls[0],
            [
                '--zstd',
                '-xf',
                str(run.root / 'tier2-input' / 'tier2-tree.tar.zst'),
                '-C',
                str(run.root / 'tier2'),
            ],
        )
        docker = run.calls[1]
        self.assertEqual(docker[:2], ['run', '--cpus'])
        self.assertIn('4', docker)
        self.assertIn(
            f'type=bind,src={workspace},dst=/__w/obs-studio/obs-studio,readonly', docker
        )
        self.assertIn(
            f'type=bind,src={run.root / "tier2"},dst=/__w/_temp/tier2', docker
        )
        self.assertIn('OBS_TIER2_MODES=OFF ON', docker)
        self.assertIn('OBS_TIER2_STAGE=test', docker)
        self.assertIn('OBS_CTEST_JOBS=8', docker)
        self.assertEqual(
            docker[-3:],
            [
                'sha256:deadbeef',
                'bash',
                str(workspace / 'rust' / 'tools' / 'linux-validate' / 'validate.sh'),
            ],
        )

    def test_missing_bundle_fails_before_docker(self):
        run = record('validate-prepared-suites')
        self.assertEqual(run.status, 1)
        self.assertEqual(len(run.calls), 1, run.calls)


class WorkflowTest(unittest.TestCase):
    """Every step is one line, and the ci.toml GEN-005 block is gone."""

    def setUp(self):
        self.text = WORKFLOW.read_text(encoding='utf-8')

    def job(self, name: str) -> str:
        block = self.text.split(f'\n  {name}:', 1)
        self.assertEqual(len(block), 2, f'no {name!r} job')
        return re.split(r'\n  (?=\S)', block[1], maxsplit=1)[0]

    def runs(self, name: str) -> list[str]:
        return [
            line.strip()
            for line in self.job(name).splitlines()
            if line.strip().startswith('run:')
        ]

    def test_no_block_run_step_left(self):
        self.assertNotIn('run: |', self.text)

    def test_each_step_is_one_call(self):
        for job, subcommands in STEPS.items():
            with self.subTest(job=job):
                runs = self.runs(job)
                for subcommand in subcommands:
                    with self.subTest(subcommand=subcommand):
                        self.assertIn(f'run: {CALL} {subcommand}', runs)

    def test_no_shell_block_left(self):
        # `Measure Setup` keeps its folded `>-` line: it is one logical command
        # (a single `uv run` call), so GEN-005 already accepts it. A block `|`
        # is what this pins out of both jobs.
        for job in STEPS:
            for line in self.runs(job):
                with self.subTest(job=job, line=line):
                    self.assertFalse(line.endswith('|'), line)

    def test_no_gen_005_exception_left(self):
        exceptions = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))['exceptions']
        left = [
            entry
            for entry in exceptions
            if entry.get('path') == WORKFLOW_PATH and entry.get('rule') == 'GEN-005'
        ]
        self.assertEqual(left, [])


class CopyTest(unittest.TestCase):
    """The module is a real port, not a shell wrapper."""

    def test_module_has_no_shell_control_flow(self):
        source = MODULE.read_text(encoding='utf-8')
        for fragment in ('shell=True', 'bash -c', 'set -e'):
            with self.subTest(fragment=fragment):
                self.assertNotIn(fragment, source)


if __name__ == '__main__':
    unittest.main()
