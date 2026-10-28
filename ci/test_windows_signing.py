#!/usr/bin/env -S uv run --script
"""The signing action's steps are Python calls (#962, GEN-005).

    uv run python -m unittest ci/test_windows_signing.py

`.github/actions/windows-signing/action.yaml` carried ten multi-line `run:`
steps in PowerShell, so `ci-lint precheck` reported a GEN-005 approved
exception for the action (and three more for the helper `.ps1` files only this
action sourced). Each step is now one call into `ci/windows_signing.py`; this
test pins the argv and the exit status of every subcommand against stubs on
`PATH`, and pins the action itself (one Python call per step, no banned shell,
the `Set Up uv` step first, the `ci.toml` block gone).
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
MODULE = ROOT / 'ci' / 'windows_signing.py'
ACTION = ROOT / '.github' / 'actions' / 'windows-signing' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/windows-signing/action.yaml'
CALL = (
    'uv run --no-project --script'
    ' ${{ github.workspace }}/ci/windows_signing.py'
)

STUB = """#!{python}
import os, sys
with open(os.environ['STUB_LOG'], 'a', encoding='utf-8') as handle:
    handle.write(os.path.basename(sys.argv[0]) + ' ' + ' '.join(sys.argv[1:]) + '\\n')
sys.exit(int(os.environ.get('STUB_STATUS', '0')))
"""

TOOLS = (
    'gh',
    'rclone',
    'winget',
    'choco',
    'openssl',
    'msiexec',
)


class StubbedRun(unittest.TestCase):
    """Run the module with the Windows toolchain replaced by stubs."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base = Path(directory.name)
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        for name in TOOLS:
            self.write_stub(name)
        self.stub_log = self.base / 'stub.log'
        self.stub_log.write_text('', encoding='utf-8')
        self.workspace = self.base / 'workspace'
        (self.workspace / 'repo').mkdir(parents=True)
        (self.workspace / 'bouf' / 'bin').mkdir(parents=True)

    def write_stub(self, name: str, path: Path | None = None) -> Path:
        target = path if path is not None else self.bin / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(STUB.format(python=sys.executable), encoding='utf-8')
        target.chmod(0o755)
        return target

    def run_module(
        self,
        *arguments: str,
        status: int = 0,
        extra: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        environment = {
            'PATH': f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
            'GITHUB_WORKSPACE': str(self.workspace),
            'GITHUB_ACTION_PATH': str(ACTION.parent),
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


class ExtractArtifactTests(StubbedRun):
    def test_the_build_zips_are_extracted_and_removed(self) -> None:
        build = self.workspace / 'build'
        build.mkdir()
        with zipfile.ZipFile(build / 'OBS-Studio-x64.zip', 'w') as bundle:
            bundle.writestr('bin/64bit/obs64.exe', 'binary')
        result = self.run_module('extract-artifact')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (build / 'bin' / '64bit' / 'obs64.exe').read_text(encoding='utf-8'),
            'binary',
        )
        self.assertEqual(list(build.glob('*.zip')), [])


class SetupBoufTests(StubbedRun):
    def test_both_archives_are_downloaded_verified_and_extracted(self) -> None:
        windows = self.workspace / 'bouf' / 'bouf-windows-v0.6.6.zip'
        nsis = self.workspace / 'bouf' / 'bouf-nsis-v0.6.6.zip'
        with zipfile.ZipFile(windows, 'w') as bundle:
            bundle.writestr('bouf.exe', 'binary')
        with zipfile.ZipFile(nsis, 'w') as bundle:
            bundle.writestr('makensis.exe', 'binary')
        # The scripts write the files, the module only verifies and extracts.
        self.write_stub('gh', self.bin / 'gh')
        result = self.run_module('setup-bouf')
        # No pin in the environment: the hash check is skipped, as before when
        # the action's env: supplied them.
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_hash_mismatch_fails(self) -> None:
        (self.workspace / 'bouf' / 'bouf-windows-v0.6.6.zip').write_bytes(b'not a zip')
        (self.workspace / 'bouf' / 'bouf-nsis-v0.6.6.zip').write_bytes(b'not a zip')
        result = self.run_module('setup-bouf', extra={'BOUF_HASH': '0' * 64})
        self.assertEqual(result.returncode, 1)
        self.assertIn('hash does not match', result.stdout + result.stderr)


class InstallTests(StubbedRun):
    def test_nsis_goes_through_winget(self) -> None:
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

    def test_rclone_goes_through_choco(self) -> None:
        result = self.run_module('install-rclone')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            ['choco install rclone --version=1.69.3 -y --no-progress'],
        )

    def test_a_failing_installer_status_is_returned(self) -> None:
        result = self.run_module('install-nsis', status=3)
        self.assertEqual(result.returncode, 3)


class SetupCngTests(StubbedRun):
    def setUp(self) -> None:
        super().setUp()
        provider = self.workspace / 'gcng'
        provider.mkdir()
        (provider / 'GoogleCloudKMS-amd64.sig').write_bytes(b'signature')
        (provider / 'GoogleCloudKMS-amd64.msi').write_bytes(b'installer')

    def test_the_msi_is_verified_then_installed(self) -> None:
        result = self.run_module('setup-cng')
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.calls()
        self.assertEqual(
            calls[0],
            'gh release download cng-v1.1 -R'
            ' GoogleCloudPlatform/kms-integrations -p *amd64.zip',
        )
        self.assertTrue(calls[1].startswith('openssl dgst -sha384 -verify '))
        self.assertIn('cng-release-signing-key.pem', calls[1])
        self.assertTrue(calls[1].endswith('.msi'))
        self.assertTrue(calls[2].startswith('msiexec /i '))
        self.assertTrue(calls[2].endswith(' /qn /norestart'))

    def test_a_bad_signature_stops_before_the_install(self) -> None:
        result = self.run_module('setup-cng', status=1)
        self.assertEqual(result.returncode, 1)
        self.assertEqual(len(self.calls()), 1)


class RcloneTests(StubbedRun):
    def test_the_previous_build_is_fetched_into_old_builds(self) -> None:
        result = self.run_module('download-previous', '--architecture', 'arm64')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            ['rclone copy --transfers 100 :gcs:obs-latest/arm64/stable .'],
        )

    def test_the_channel_is_honoured(self) -> None:
        result = self.run_module(
            'download-previous', '--architecture', 'x64', '--channel', 'beta'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            ['rclone copy --transfers 100 :gcs:obs-latest/x64/beta .'],
        )

    def test_the_latest_build_is_synced_back(self) -> None:
        result = self.run_module('sync-latest', '--architecture', 'x64')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                'rclone sync --delete-excluded --transfers 100'
                f' {self.workspace}/old_builds :gcs:obs-latest/x64/stable'
            ],
        )

    def test_the_build_is_uploaded_to_the_archive(self) -> None:
        result = self.run_module('upload-archive', '--architecture', 'x64')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                'rclone copy --transfers 100'
                f' {self.workspace}/old_builds :gcs:obs-builds/x64'
            ],
        )


class SigningTests(StubbedRun):
    def test_signtool_gets_the_kms_arguments(self) -> None:
        signtool = self.write_stub('signtool', self.bin / 'signtool')
        result = self.run_module('sign-game-capture', extra={'SIGNTOOL': str(signtool)})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                'signtool sign /fd sha256 /t http://timestamp.digicert.com'
                ' /f repo/.github/actions/windows-signing/prod-gc.crt'
                ' /csp Google Cloud KMS Provider'
                ' /kc projects/ci-signing/locations/global/keyRings/production'
                '/cryptoKeys/game-capture-release-sign-hsm/cryptoKeyVersions/1'
                f' {self.workspace}/build/core/win-capture/data/*.dll'
            ],
        )

    def test_a_failed_signature_is_returned(self) -> None:
        signtool = self.write_stub('signtool', self.bin / 'signtool')
        result = self.run_module(
            'sign-game-capture', status=1, extra={'SIGNTOOL': str(signtool)}
        )
        self.assertEqual(result.returncode, 1)


class RunBoufTests(StubbedRun):
    def test_bouf_gets_the_packaging_arguments(self) -> None:
        bouf = self.workspace / 'bouf' / 'bin' / 'bouf.exe'
        self.write_stub('bouf', bouf)
        result = self.run_module(
            'run-bouf', '--version', '30.0.0-rc1', '--architecture', 'x64'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.calls(),
            [
                'bouf.exe'
                f' --config {ACTION.parent}/config_x64.toml --version 30.0.0-rc1'
                f' --branch stable -i {self.workspace}/build'
                f' -p {self.workspace}/old_builds -o {self.workspace}/output'
                ' --packaging-only'
            ],
        )


class ActionShapeTests(unittest.TestCase):
    """The action file itself: one Python call per step, no shell blocks."""

    def setUp(self) -> None:
        self.text = ACTION.read_text(encoding='utf-8')

    def test_the_action_has_no_powershell(self) -> None:
        self.assertNotIn('pwsh', self.text)
        self.assertNotIn('PowerShell', self.text)

    def test_every_step_is_one_module_call(self) -> None:
        for step in (
            'extract-artifact',
            'setup-bouf',
            'install-nsis',
            'setup-cng',
            'install-rclone',
            'download-previous',
            'sign-game-capture',
            'run-bouf',
            'sync-latest',
            'upload-archive',
        ):
            self.assertIn(f'{CALL} {step}', self.text)

    def test_the_uv_setup_step_comes_first(self) -> None:
        steps = self.text.split('steps:', 1)[1]
        self.assertLess(
            steps.index('astral-sh/setup-uv'), steps.index(f'{CALL} extract-artifact')
        )

    def test_the_uv_setup_carries_the_save_cache_expression(self) -> None:
        self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", self.text)
        self.assertIn('cache-suffix:', self.text)

    def test_the_gcp_auth_step_is_untouched(self) -> None:
        self.assertIn('google-github-actions/auth@', self.text)


class CiTomlTests(unittest.TestCase):
    def test_the_path_has_no_gen_005_exception(self) -> None:
        with CI_TOML.open('rb') as handle:
            data = tomllib.load(handle)
        paths = [
            entry.get('path')
            for entry in data.get('exceptions', [])
            if entry.get('rule') == 'GEN-005'
        ]
        self.assertNotIn(ACTION_PATH, paths)
        self.assertNotIn(
            '.github/actions/windows-signing/Ensure-Location.ps1', paths
        )
        self.assertNotIn('.github/actions/windows-signing/Invoke-External.ps1', paths)
        self.assertNotIn('.github/actions/windows-signing/Logger.ps1', paths)


if __name__ == '__main__':
    unittest.main()
