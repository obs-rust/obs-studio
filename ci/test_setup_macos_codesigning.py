#!/usr/bin/env -S uv run --script
"""The macOS code-signing action is Python calls, not zsh blocks (#962, GEN-005).

    uv run python -m unittest ci/test_setup_macos_codesigning.py

`.github/actions/setup-macos-codesigning/action.yaml` carried four multi-line
`run:` steps -- the non-macOS guard, the keychain/certificate import, the
provisioning-profile import and the notarization flag -- so the action produced
a GEN-005 finding for each. Each step is now one call into
`ci/setup_macos_codesigning.py`:

    guard         the action's own `if: runner.os != 'macOS'` selects it; it
                  prints the reason and exits 2, as the bash step did;
    codesign      decodes the PKCS12, creates and unlocks the temporary
                  keychain, imports the certificate, and writes
                  `haveCodesignIdent` / `codesignIdent` / `codesignTeam` plus
                  `MACOS_KEYCHAIN_PASSWORD` for the later steps;
    provisioning  decodes the `.provisionprofile`, reads its UUID and team with
                  `plutil`, copies it into the user's profiles directory and
                  writes `provisioningProfileUUID`;
    notarization  writes `haveNotarizationUser` from the credentials present.

The `security` and `plutil` calls are recorded by stubs on `PATH`, so the argv
and the outputs each branch writes are asserted on Linux; macOS is not needed.
The action's `Set Up uv` step must stay above the guard, which now calls `uv`.

RED first: committed while all four steps were still shell blocks, the module
did not exist and the GEN-005 `[[exceptions]]` block was still in `ci.toml`.
"""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'setup_macos_codesigning.py'
ACTION = ROOT / '.github' / 'actions' / 'setup-macos-codesigning' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/setup-macos-codesigning/action.yaml'
CALL = (
    'uv run --no-project --script'
    ' ${{ github.workspace }}/ci/setup_macos_codesigning.py'
)

IDENTITY = 'Developer ID Application: Example Corp (ABCDE12345)'
TEAM = 'ABCDE12345'
PROFILE_UUID = 'A1B2C3D4-E5F6-7890-ABCD-EF1234567890'
CERTIFICATE = 'Y2VydGlmaWNhdGU='
PROFILE = 'cHJvZmlsZQ=='

SECURITY_STUB = """#!/bin/sh
printf '%s\\n' "security $*" >> "$STUB_LOG"
exit 0
"""

PLUTIL_STUB = """#!/bin/sh
printf '%s\\n' "plutil $*" >> "$STUB_LOG"
case "$2" in
  UUID) printf '%s\\n' "$STUB_UUID" ;;
  TeamIdentifier.0) printf '%s\\n' "$STUB_TEAM" ;;
esac
exit 0
"""


def output_values(path: Path) -> dict[str, str]:
    """Parse a `$GITHUB_OUTPUT` / `$GITHUB_ENV` file into a mapping."""

    values: dict[str, str] = {}
    for line in path.read_text(encoding='utf-8').splitlines():
        if not line:
            continue
        name, _, value = line.partition('=')
        values[name] = value
    return values


class StubbedRun(unittest.TestCase):
    """Run the module with stub `security` / `plutil` on PATH."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base = Path(directory.name)
        self.bin = self.base / 'bin'
        self.bin.mkdir()
        self.write_stub('security', SECURITY_STUB)
        self.write_stub('plutil', PLUTIL_STUB)
        self.stub_log = self.base / 'stub.log'
        self.stub_log.write_text('', encoding='utf-8')
        self.runner_temp = self.base / 'runner-temp'
        self.runner_temp.mkdir()
        self.home = self.base / 'home'
        self.home.mkdir()
        self.outputs = self.base / 'github-output'
        self.outputs.write_text('', encoding='utf-8')
        self.env_file = self.base / 'github-env'
        self.env_file.write_text('', encoding='utf-8')

    def write_stub(self, name: str, body: str) -> None:
        path = self.bin / name
        path.write_text(body, encoding='utf-8')
        path.chmod(0o755)

    def run_module(
        self, *arguments: str, **variables: str
    ) -> subprocess.CompletedProcess[str]:
        environment = {
            'PATH': f'{self.bin}{os.pathsep}{os.environ["PATH"]}',
            'HOME': str(self.home),
            'RUNNER_TEMP': str(self.runner_temp),
            'GITHUB_OUTPUT': str(self.outputs),
            'GITHUB_ENV': str(self.env_file),
            'STUB_LOG': str(self.stub_log),
            'STUB_UUID': PROFILE_UUID,
            'STUB_TEAM': TEAM,
        }
        environment.update(variables)
        return subprocess.run(
            [sys.executable, str(MODULE), *arguments],
            capture_output=True,
            text=True,
            check=False,
            env=environment,
        )

    def security_calls(self) -> list[str]:
        return [
            line
            for line in self.stub_log.read_text(encoding='utf-8').splitlines()
            if line.startswith('security ')
        ]

    def recorded_outputs(self) -> dict[str, str]:
        return output_values(self.outputs)


class GuardTests(unittest.TestCase):
    """The guard step: the reason, then a failing exit, on a non-macOS runner."""

    def test_guard_prints_the_reason_and_exits_2(self) -> None:
        result = subprocess.run(
            [sys.executable, str(MODULE), 'guard'],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn(
            'setup-macos-codesigning action requires a macOS-based runner.',
            result.stdout,
        )


class CodesignTests(StubbedRun):
    """The keychain step: certificate import and the outputs later steps read."""

    def test_without_credentials_it_only_sets_the_flags(self) -> None:
        result = self.run_module('codesign')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.recorded_outputs(),
            {
                'haveCodesignIdent': 'false',
                'haveProvisioningProfile': 'false',
                'haveNotarizationUser': 'false',
            },
        )
        self.assertEqual(self.security_calls(), [])

    def test_with_credentials_it_imports_the_certificate_into_a_new_keychain(
        self,
    ) -> None:
        result = self.run_module(
            'codesign',
            MACOS_SIGNING_IDENTITY=IDENTITY,
            MACOS_SIGNING_CERT=CERTIFICATE,
            MACOS_SIGNING_CERT_PASSWORD='cert-password',
            MACOS_KEYCHAIN_PASSWORD='keychain-password',
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        certificate = self.runner_temp / 'build_certificate.p12'
        self.assertEqual(certificate.read_text(encoding='utf-8'), 'certificate')
        keychain = str(self.runner_temp / 'app-signing.keychain-db')
        self.assertEqual(
            self.security_calls(),
            [
                f'security create-keychain -p keychain-password {keychain}',
                f'security set-keychain-settings -lut 21600 {keychain}',
                f'security unlock-keychain -p keychain-password {keychain}',
                f'security import {certificate} -P cert-password -A -t cert'
                f' -f pkcs12 -k {keychain} -T /usr/bin/codesign'
                ' -T /usr/bin/security -T /usr/bin/xcrun',
                'security set-key-partition-list -S apple-tool:,apple:'
                f' -k keychain-password {keychain}',
                f'security list-keychain -d user -s {keychain} login-keychain',
            ],
        )
        self.assertIn('::group::Keychain setup', result.stdout)

    def test_the_outputs_carry_the_identity_and_the_parenthesis_free_team(
        self,
    ) -> None:
        self.run_module(
            'codesign',
            MACOS_SIGNING_IDENTITY=IDENTITY,
            MACOS_SIGNING_CERT=CERTIFICATE,
            MACOS_SIGNING_CERT_PASSWORD='cert-password',
            MACOS_KEYCHAIN_PASSWORD='keychain-password',
        )
        outputs = self.recorded_outputs()
        self.assertEqual(outputs['haveCodesignIdent'], 'true')
        self.assertEqual(outputs['codesignIdent'], IDENTITY)
        self.assertEqual(outputs['codesignTeam'], TEAM)
        self.assertEqual(
            output_values(self.env_file),
            {'MACOS_KEYCHAIN_PASSWORD': 'keychain-password'},
        )

    def test_a_missing_keychain_password_is_generated_and_recorded(self) -> None:
        self.run_module(
            'codesign',
            MACOS_SIGNING_IDENTITY=IDENTITY,
            MACOS_SIGNING_CERT=CERTIFICATE,
            MACOS_SIGNING_CERT_PASSWORD='cert-password',
        )
        generated = output_values(self.env_file)['MACOS_KEYCHAIN_PASSWORD']
        self.assertEqual(len(generated), 32)
        self.assertRegex(generated, r'^[0-9a-f]{32}$')
        self.assertIn(f'-p {generated}', self.security_calls()[0])


class ProvisioningTests(StubbedRun):
    """The profile step: decode, read the UUID/team, install into the home."""

    def test_without_a_profile_it_only_sets_the_flag(self) -> None:
        result = self.run_module('provisioning', '--codesign-team', TEAM)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.recorded_outputs(), {'haveProvisioningProfile': 'false'}
        )

    def test_a_matching_profile_is_copied_and_its_uuid_recorded(self) -> None:
        result = self.run_module(
            'provisioning',
            '--codesign-team',
            TEAM,
            MACOS_SIGNING_PROVISIONING_PROFILE=PROFILE,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.recorded_outputs(),
            {
                'haveProvisioningProfile': 'true',
                'provisioningProfileUUID': PROFILE_UUID,
            },
        )
        installed = (
            self.home
            / 'Library'
            / 'MobileDevice'
            / 'Provisioning Profiles'
            / f'{PROFILE_UUID}.provisionprofile'
        )
        self.assertEqual(installed.read_text(encoding='utf-8'), 'profile')
        self.assertNotIn(
            'does not match certificate', result.stdout
        )

    def test_a_profile_from_another_team_is_a_notice_not_a_failure(self) -> None:
        result = self.run_module(
            'provisioning',
            '--codesign-team',
            'ZZZZZ99999',
            MACOS_SIGNING_PROVISIONING_PROFILE=PROFILE,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn(
            '::notice::Code Signing team in provisioning profile does not'
            ' match certificate.',
            result.stdout,
        )


class NotarizationTests(StubbedRun):
    """The notarization step is a flag from the credentials present."""

    def test_both_credentials_set_the_flag(self) -> None:
        self.run_module(
            'notarization',
            MACOS_NOTARIZATION_USERNAME='apple-id@example.com',
            MACOS_NOTARIZATION_PASSWORD='app-password',
        )
        self.assertEqual(
            self.recorded_outputs(), {'haveNotarizationUser': 'true'}
        )

    def test_a_missing_password_clears_the_flag(self) -> None:
        self.run_module(
            'notarization', MACOS_NOTARIZATION_USERNAME='apple-id@example.com'
        )
        self.assertEqual(
            self.recorded_outputs(), {'haveNotarizationUser': 'false'}
        )


class ActionTests(unittest.TestCase):
    """The action itself: one line per step, no zsh block left."""

    def setUp(self) -> None:
        self.text = ACTION.read_text(encoding='utf-8')

    def test_the_action_calls_the_module_for_all_four_steps(self) -> None:
        for command in ('guard', 'codesign', 'provisioning', 'notarization'):
            with self.subTest(command=command):
                self.assertIn(f'{CALL} {command}', self.text)

    def test_no_run_step_is_a_shell_block(self) -> None:
        self.assertNotIn('run: |', self.text)
        for line in self.text.splitlines():
            if 'run: ' in line:
                command = line.split('run: ', 1)[1]
                self.assertNotRegex(command, r'[;&|`]|\$\(', line)

    def test_the_step_ids_and_outputs_survive(self) -> None:
        for entry in (
            'id: codesign',
            'id: provisioning',
            'id: notarization',
            'value: ${{ steps.codesign.outputs.haveCodesignIdent }}',
            'value: ${{ steps.codesign.outputs.codesignTeam }}',
        ):
            with self.subTest(entry=entry):
                self.assertIn(entry, self.text)

    def test_uv_is_installed_before_the_guard(self) -> None:
        setup_at = self.text.index('astral-sh/setup-uv@')
        self.assertIn('cache-suffix:', self.text)
        self.assertIn("save-cache: ${{ github.ref == 'refs/heads/master' }}", self.text)
        self.assertLess(setup_at, self.text.index(f'{CALL} guard'))

    def test_the_profile_step_keeps_the_codesign_team_argument(self) -> None:
        self.assertIn(
            '--codesign-team "${{ steps.codesign.outputs.codesignTeam }}"',
            self.text,
        )


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
            'the GEN-005 exception for setup-macos-codesigning must be deleted'
            ' with the port',
        )


class ModuleTests(unittest.TestCase):
    def test_the_module_is_a_uv_script(self) -> None:
        self.assertTrue(MODULE.is_file(), f'{MODULE} must exist')
        first_line = MODULE.read_text(encoding='utf-8').splitlines()[0]
        self.assertEqual(
            first_line, '#!/usr/bin/env -S uv run --no-project --script'
        )

    def test_the_module_imports_only_the_standard_library(self) -> None:
        text = MODULE.read_text(encoding='utf-8')
        for name in ('import argparse', 'import base64', 'import subprocess', 'import sys'):
            self.assertIn(name, text)


if __name__ == '__main__':
    unittest.main()
