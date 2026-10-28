#!/usr/bin/env -S uv run --script
"""The bouf patch action's steps are Python calls (#962, GEN-005).

    uv run python -m unittest ci/test_windows_patches.py

`.github/actions/windows-patches/action.yaml` carried nine multi-line `run:`
steps in PowerShell -- twelve GEN-005 approved exceptions for that path once the
`shell: pwsh` keys are counted. Each step is now one call into
`ci/windows_patches.py`; this test pins the argv and the exit status of every
subcommand against stubs on `PATH`, and pins the action itself (one line per
step, no banned shell, the `Set Up uv` step first, the `ci.toml` block gone).

RED first: committed while the action was still nine PowerShell blocks, the
module did not exist and the GEN-005 `[[exceptions]]` block was in `ci.toml`.
"""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
import zipfile

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'windows_patches.py'
ACTION = ROOT / '.github' / 'actions' / 'windows-patches' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/windows-patches/action.yaml'
CALL = (
    'uv run --no-project --script'
    ' ${{ github.workspace }}/ci/windows_patches.py'
)
TAG = 'v0.1.0-beta'
CHANNEL = 'stable'

STUB = """#!{python}
import os, sys
with open(os.environ['STUB_LOG'], 'a', encoding='utf-8') as handle:
    handle.write(os.path.basename(sys.argv[0]) + ' ' + ' '.join(sys.argv[1:]) + '\\n')
sys.exit(int(os.environ.get('STUB_STATUS', '0')))
"""


class StubbedRun(unittest.TestCase):
    """Run the module with `gh`, `rclone`, `winget`, `choco` and `git` stubs."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base = Path(directory.name)
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        for name in ('gh', 'rclone', 'winget', 'choco', 'git'):
            self.write_stub(name)
        self.stub_log = self.base / 'stub.log'
        self.stub_log.write_text('', encoding='utf-8')
        self.workspace = self.base / 'workspace'
        (self.workspace / 'repo').mkdir(parents=True)
        (self.workspace / 'bouf' / 'bin').mkdir(parents=True)

    def write_stub(self, name: str) -> Path:
        path = self.bin / name
        path.write_text(STUB.format(python=sys.executable), encoding='utf-8')
        path.chmod(0o755)
        return path

    def run_module(
        self,
        *arguments: str,
        status: int = 0,
        extra: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        environment = {
            'PATH': f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
            'GITHUB_WORKSPACE': str(self.workspace),
            'STUB_LOG': str(self.stub_log),
            'STUB_STATUS': str(status),
        }
        environment.update(extra or {})
        return subprocess.run(
            [sys.executable, str(MODULE), *arguments],
            capture_output=True,
            text=True,
            check=False,
            env=environment,
        )

    def calls(self) -> list[str]:
        return self.stub_log.read_text(encoding='utf-8').splitlines()


class DownloadTests(StubbedRun):
    def test_the_release_zip_is_downloaded_verified_and_extracted(self) -> None:
        archive = self.workspace / f'OBS-Studio-{TAG}-Windows-x64.zip'
        with zipfile.ZipFile(archive, 'w') as bundle:
            bundle.writestr('bin/64bit/obs64.exe', 'binary')
        result = self.run_module('download', '--tag', TAG)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                f'gh release download {TAG} -p {archive.name}',
                f'gh attestation verify {archive.name} --owner obsproject',
            ],
        )
        self.assertEqual(
            (self.workspace / 'build' / 'bin' / '64bit' / 'obs64.exe').read_text(
                encoding='utf-8'
            ),
            'binary',
        )

    def test_a_failed_attestation_stops_before_the_extract(self) -> None:
        result = self.run_module('download', '--tag', TAG, status=1)
        self.assertEqual(result.returncode, 1)


class SetupBoufTests(StubbedRun):
    def setUp(self) -> None:
        super().setUp()
        self.tag = 'v0.6.5'
        self.windows_zip = self.workspace / f'bouf-windows-{self.tag}.zip'
        self.nsis_zip = self.workspace / f'bouf-nsis-{self.tag}.zip'
        for archive, member in (
            (self.windows_zip, 'bouf.exe'),
            (self.nsis_zip, 'makensis.exe'),
        ):
            with zipfile.ZipFile(archive, 'w') as bundle:
                bundle.writestr(member, 'binary')

    def test_both_archives_are_downloaded_and_extracted_after_the_hashes_match(
        self,
    ) -> None:
        import hashlib

        hashes = {
            name: hashlib.sha256(path.read_bytes()).hexdigest()
            for name, path in (
                ('BOUF_HASH', self.windows_zip),
                ('BOUF_NSIS_HASH', self.nsis_zip),
            )
        }
        result = self.run_module(
            'setup-bouf', extra={**hashes, 'BOUF_TAG': self.tag}
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                f'gh release download {self.tag} -R obsproject/bouf'
                f' -p {self.windows_zip.name} -p {self.nsis_zip.name}'
            ],
        )
        self.assertTrue((self.workspace / 'bouf' / 'bin' / 'bouf.exe').is_file())
        self.assertTrue(
            (self.workspace / 'bouf' / 'nsis' / 'makensis.exe').is_file()
        )

    def test_a_hash_mismatch_fails_the_step_before_extracting(self) -> None:
        result = self.run_module(
            'setup-bouf',
            extra={
                'BOUF_TAG': self.tag,
                'BOUF_HASH': '0' * 64,
                'BOUF_NSIS_HASH': '0' * 64,
            },
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('hash does not match', result.stdout)
        self.assertFalse((self.workspace / 'bouf' / 'nsis').exists())


class InstallTests(StubbedRun):
    def test_winget_gets_the_nsis_arguments(self) -> None:
        result = self.run_module('install-nsis')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                'winget install --silent --accept-package-agreements'
                ' --accept-source-agreements --disable-interactivity'
                ' -e --id NSIS.NSIS'
            ],
        )

    def test_chocolatey_gets_rclone_and_pandoc(self) -> None:
        result = self.run_module('install-tools')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.calls(), ['choco install rclone pandoc -y --no-progress'])

    def test_rclone_copies_the_architecture_prefix(self) -> None:
        result = self.run_module('download-previous', '--architecture', 'arm64')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                f'rclone -q copy :s3:obs-builds/arm64'
                f' {self.workspace}/old_builds'
            ],
        )


class PrepareNotesTests(StubbedRun):
    def test_the_subject_rule_and_body_are_written_in_order(self) -> None:
        result = self.run_module('prepare-notes', '--tag', TAG)
        self.assertEqual(result.returncode, 0, result.stderr)
        # The stub writes nothing, so the file is the subject (empty), the rule
        # and the body (empty) -- the shape the PowerShell step produced.
        self.assertEqual(
            (self.workspace / 'notes.rst').read_text(encoding='utf-8'),
            '#' * 51 + '\n\n',
        )
        self.assertEqual(
            self.calls(),
            [
                f'git tag -l --format=%(contents:subject) {TAG}',
                f'git tag -l --format=%(contents:body) {TAG}',
            ],
        )


class RunBoufTests(StubbedRun):
    def setUp(self) -> None:
        super().setUp()
        stub = self.workspace / 'bouf' / 'bin' / 'bouf.exe'
        stub.write_text(STUB.format(python=sys.executable), encoding='utf-8')
        stub.chmod(0o755)

    def test_the_bouf_argument_list_is_unchanged(self) -> None:
        result = self.run_module(
            'run-bouf', '--tag', TAG, '--channel', CHANNEL
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                f'bouf.exe --config'
                f' {self.workspace}/.github/actions/windows-patches/config.toml'
                f' --version {TAG} --branch {CHANNEL}'
                f' --notes-file {self.workspace}/notes.rst'
                f' -i {self.workspace}/build'
                f' -p {self.workspace}/old_builds'
                f' -o {self.workspace}/output --updater-data-only'
            ],
        )

    def test_a_failing_bouf_fails_the_step(self) -> None:
        result = self.run_module('run-bouf', '--tag', TAG, status=2)
        self.assertEqual(result.returncode, 2)


class ActionTests(unittest.TestCase):
    """The action itself: one line per step, no PowerShell block left."""

    def setUp(self) -> None:
        self.text = ACTION.read_text(encoding='utf-8')

    def test_every_step_calls_the_module(self) -> None:
        for command in (
            'download',
            'setup-bouf',
            'install-nsis',
            'install-tools',
            'download-previous',
            'prepare-notes',
            'run-bouf',
        ):
            with self.subTest(command=command):
                self.assertIn(f'{CALL} {command}', self.text)

    def test_no_run_step_is_a_shell_block(self) -> None:
        self.assertNotIn('run: |', self.text)
        self.assertNotIn('shell: pwsh', self.text)
        for line in self.text.splitlines():
            if 'run: ' in line:
                command = line.split('run: ', 1)[1]
                self.assertNotRegex(command, r'[;&|`]|\$\(', line)

    def test_the_inputs_reach_the_module(self) -> None:
        for entry in (
            '--tag "${{ inputs.tagName }}"',
            '--channel "${{ inputs.channel }}"',
            '--architecture "${{ inputs.architecture }}"',
        ):
            with self.subTest(entry=entry):
                self.assertIn(entry, self.text)

    def test_the_upload_step_survives(self) -> None:
        self.assertIn('actions/upload-artifact@', self.text)

    def test_uv_is_installed_before_the_first_call(self) -> None:
        setup_at = self.text.index('astral-sh/setup-uv@')
        self.assertIn('cache-suffix:', self.text)
        self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", self.text)
        self.assertLess(setup_at, self.text.index(f'{CALL} download'))


class ExceptionTests(unittest.TestCase):
    def test_no_gen_005_exception_left(self) -> None:
        data = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))
        stale = [
            entry
            for entry in data.get('exceptions', [])
            if entry.get('rule') == 'GEN-005' and entry.get('path') == ACTION_PATH
        ]
        self.assertEqual(
            stale,
            [],
            'the GEN-005 exception for windows-patches must be deleted with the port',
        )


class ModuleTests(unittest.TestCase):
    def test_the_module_is_a_uv_script(self) -> None:
        self.assertTrue(MODULE.is_file(), f'{MODULE} must exist')
        first_line = MODULE.read_text(encoding='utf-8').splitlines()[0]
        self.assertEqual(first_line, '#!/usr/bin/env -S uv run --no-project --script')

    def test_the_module_imports_only_the_standard_library(self) -> None:
        text = MODULE.read_text(encoding='utf-8')
        for name in ('import argparse', 'import hashlib', 'import subprocess'):
            self.assertIn(name, text)


if __name__ == '__main__':
    unittest.main()
