#!/usr/bin/env -S uv run --script
"""push.yaml's block `run:` steps are Python calls (#962, GEN-005).

    uv run python -m unittest ci/test_push_release.py

`.github/workflows/push.yaml` carried five multi-line `run:` steps -- the two
`deploy-documentation` shell blocks and the three `create-release` ones -- so
`ci-lint precheck` reported a GEN-005 approved exception for the path. Each
step is now one call into `ci/push_release.py`:

    commit-hash       `commitHash` from `GITHUB_SHA` into `$GITHUB_OUTPUT`;
    set-redirects     the two `/previous/...` lines appended to
                      `docs/_redirects`;
    check-tag         the tag forms written as `validTag`, `prerelease` and
                      `version`;
    rename-artifacts  the twelve artifact moves, globbed as `mv` did;
    checksums         `CHECKSUMS.txt` with one sha256 line per artifact.

RED first: committed while all five steps were still shell blocks, the module
did not exist and the GEN-005 `[[exceptions]]` block was still in `ci.toml`.
"""

from __future__ import annotations

import hashlib
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'push_release.py'
WORKFLOW = ROOT / '.github' / 'workflows' / 'push.yaml'
CI_TOML = ROOT / 'ci.toml'
WORKFLOW_PATH = '.github/workflows/push.yaml'
CALL = 'uv run --no-project --script ${{ github.workspace }}/ci/push_release.py'
SHA = 'abcdef1234567890abcdef1234567890abcdef12'
SHORT = SHA[:9]
VERSION = '31.0.0'
PINNED_UV = 'astral-sh/setup-uv@bec219d24cd3e171d82865faccec33120bb574f4'
REDIRECTS = (
    '/previous/27.2 https://obsproject.com/docs/27.2 302',
    '/previous/:major.:minor https://:major-:minor.obs-docs.pages.dev 302',
)

ARTIFACTS = (
    (
        f'obs-studio-macos-arm64-{SHORT}',
        'obs-studio-31.0.0-macos-apple.dmg',
        'OBS-Studio-31.0.0-macOS-Apple.dmg',
    ),
    (
        f'obs-studio-macos-arm64-{SHORT}-dSYMs',
        'obs-studio-31.0.0-macos-apple-dSYMs.tar.xz',
        'OBS-Studio-31.0.0-macOS-Apple-dSYMs.tar.xz',
    ),
    (
        f'obs-studio-macos-x86_64-{SHORT}',
        'obs-studio-31.0.0-macos-intel.dmg',
        'OBS-Studio-31.0.0-macOS-Intel.dmg',
    ),
    (
        f'obs-studio-macos-x86_64-{SHORT}-dSYMs',
        'obs-studio-31.0.0-macos-intel-dSYMs.tar.xz',
        'OBS-Studio-31.0.0-macOS-Intel-dSYMs.tar.xz',
    ),
    (
        f'obs-studio-ubuntu-26.04-x86_64-{SHORT}',
        'obs-studio-31.0.0-x86_64-ubuntu-gnu.deb',
        'OBS-Studio-31.0.0-Ubuntu-26.04-x86_64.deb',
    ),
    (
        f'obs-studio-ubuntu-26.04-x86_64-{SHORT}-dbgsym',
        'obs-studio-31.0.0-x86_64-ubuntu-gnu-dbgsym.ddeb',
        'OBS-Studio-31.0.0-Ubuntu-26.04-x86_64-dbsym.ddeb',
    ),
    (
        f'obs-studio-ubuntu-26.04-sources-{SHORT}',
        'obs-studio-31.0.0-sources.tar.gz',
        'OBS-Studio-31.0.0-Sources.tar.gz',
    ),
    (
        f'obs-studio-windows-x64-{VERSION}-signed',
        'OBS-Studio-31.0.0-x64.zip',
        'OBS-Studio-31.0.0-Windows-x64.zip',
    ),
    (
        f'obs-studio-windows-x64-{VERSION}-installer',
        'OBS-Studio-31.0.0-x64.exe',
        'OBS-Studio-31.0.0-Windows-x64-Installer.exe',
    ),
    (
        f'obs-studio-windows-x64-{VERSION}-pdbs',
        'OBS-Studio-31.0.0-x64-pdbs.zip',
        'OBS-Studio-31.0.0-Windows-x64-PDBs.zip',
    ),
    (
        f'obs-studio-windows-arm64-{VERSION}-signed',
        'OBS-Studio-31.0.0-arm64.zip',
        'OBS-Studio-31.0.0-Windows-arm64.zip',
    ),
    (
        f'obs-studio-windows-arm64-{VERSION}-pdbs',
        'OBS-Studio-31.0.0-arm64-pdbs.zip',
        'OBS-Studio-31.0.0-Windows-arm64-PDBs.zip',
    ),
)


class StepRun(unittest.TestCase):
    """Run one subcommand against a temporary `$GITHUB_WORKSPACE`."""

    def setUp(self) -> None:
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.base = Path(directory.name)
        self.workspace = self.base / 'workspace'
        self.workspace.mkdir()
        self.output = self.base / 'output'
        self.output.write_text('', encoding='utf-8')

    def run_module(
        self,
        *arguments: str,
        extra: dict[str, str] | None = None,
    ) -> subprocess.CompletedProcess[str]:
        environment = {
            'PATH': os.environ['PATH'],
            'GITHUB_WORKSPACE': str(self.workspace),
            'GITHUB_OUTPUT': str(self.output),
            'GITHUB_SHA': SHA,
            'GITHUB_REF_NAME': VERSION,
        }
        environment.update(extra or {})
        return subprocess.run(
            [sys.executable, str(MODULE), *arguments],
            capture_output=True,
            text=True,
            check=False,
            env=environment,
        )

    def outputs(self) -> list[str]:
        return self.output.read_text(encoding='utf-8').splitlines()


class CommitHashTests(StepRun):
    def test_the_short_sha_reaches_the_step_output(self) -> None:
        result = self.run_module('commit-hash')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.outputs(), [f'commitHash={SHORT}'])

    def test_a_missing_sha_fails_the_step(self) -> None:
        result = self.run_module('commit-hash', extra={'GITHUB_SHA': ''})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.outputs(), [])


class SetRedirectsTests(StepRun):
    def test_both_previous_redirects_are_appended(self) -> None:
        docs = self.workspace / 'docs'
        docs.mkdir()
        (docs / '_redirects').write_text('/existing 301\n', encoding='utf-8')
        result = self.run_module('set-redirects', '--project', 'obs-docs')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (docs / '_redirects').read_text(encoding='utf-8').splitlines(),
            ['/existing 301', *REDIRECTS],
        )

    def test_the_file_is_created_when_it_is_missing(self) -> None:
        (self.workspace / 'docs').mkdir()
        result = self.run_module('set-redirects', '--project', 'obs-docs')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            (self.workspace / 'docs' / '_redirects')
            .read_text(encoding='utf-8')
            .splitlines(),
            list(REDIRECTS),
        )

    def test_an_empty_project_fails_the_step(self) -> None:
        (self.workspace / 'docs').mkdir()
        result = self.run_module('set-redirects', '--project', '')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.workspace / 'docs' / '_redirects').exists())


class CheckTagTests(StepRun):
    def test_release_tags(self) -> None:
        result = self.run_module('check-tag', extra={'GITHUB_REF_NAME': '31.0.0'})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            self.outputs(),
            ['validTag=true', 'prerelease=false', 'version=31.0.0'],
        )

    def test_prerelease_tags(self) -> None:
        for tag in ('31.0.0-beta1', '31.0.0-beta', '31.0.0-rc5', '31.0.0-rc'):
            with self.subTest(tag=tag):
                self.output.write_text('', encoding='utf-8')
                result = self.run_module('check-tag', extra={'GITHUB_REF_NAME': tag})
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    self.outputs(),
                    ['validTag=true', 'prerelease=true', f'version={tag}'],
                )

    def test_anything_else_is_not_a_release_tag(self) -> None:
        for tag in ('v31.0.0', '31.0', '31.0.0.preview', '31.0.0-beta.1', 'nightly'):
            with self.subTest(tag=tag):
                self.output.write_text('', encoding='utf-8')
                result = self.run_module('check-tag', extra={'GITHUB_REF_NAME': tag})
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.outputs(), ['validTag=false'])


class RenameArtifactsTests(StepRun):
    def setUp(self) -> None:
        super().setUp()
        for directory, source, _ in ARTIFACTS:
            (self.workspace / directory).mkdir()
            (self.workspace / directory / source).write_text(
                directory, encoding='utf-8'
            )

    def test_every_artifact_lands_under_its_release_name(self) -> None:
        result = self.run_module('rename-artifacts', '--version', VERSION)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('::group::Renaming Artifacts', result.stdout)
        self.assertIn('::endgroup::', result.stdout)
        for directory, source, target in ARTIFACTS:
            with self.subTest(directory=directory):
                self.assertFalse((self.workspace / directory / source).exists())
                self.assertEqual(
                    (self.workspace / target).read_text(encoding='utf-8'),
                    directory,
                )

    def test_a_missing_artifact_fails_the_step(self) -> None:
        directory, _, _ = ARTIFACTS[0]
        for entry in (self.workspace / directory).iterdir():
            entry.unlink()
        result = self.run_module('rename-artifacts', '--version', VERSION)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('no file matches', result.stdout)

    def test_two_matches_fail_the_step_like_mv_did(self) -> None:
        directory, _, _ = ARTIFACTS[-1]
        (self.workspace / directory / 'OBS-Studio-31.0.0-arm64-extra-pdbs.zip').write_text(
            'second', encoding='utf-8'
        )
        result = self.run_module('rename-artifacts', '--version', VERSION)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('more than one file matches', result.stdout)


class ChecksumTests(StepRun):
    def test_one_line_per_artifact_in_glob_order(self) -> None:
        names = (
            'OBS-Studio-31.0.0-Ubuntu-26.04-x86_64.deb',
            'OBS-Studio-31.0.0-macOS-Apple.dmg',
            'OBS-Studio-31.0.0-Windows-x64.zip',
            'OBS-Studio-31.0.0-Sources.tar.gz',
            'not-an-artifact.txt',
        )
        for name in names:
            (self.workspace / name).write_text(name, encoding='utf-8')
        result = self.run_module('checksums')
        self.assertEqual(result.returncode, 0, result.stderr)
        expected = ['### Checksums']
        for name in sorted(names[:-1]):
            digest = hashlib.sha256(name.encode()).hexdigest()
            expected.append(f'    {name}: {digest}')
        self.assertEqual(
            (self.workspace / 'CHECKSUMS.txt').read_text(encoding='utf-8').splitlines(),
            expected,
        )


class WorkflowTests(unittest.TestCase):
    """The workflow itself: one line per step, no shell block left."""

    def setUp(self) -> None:
        self.text = WORKFLOW.read_text(encoding='utf-8')

    def section(self, job: str) -> str:
        match = re.search(rf'\n  {job}:.*?(?=\n  \S|\Z)', self.text, re.S)
        if match is None:
            self.fail(f'{job} must stay in push.yaml')
        return match.group(0)

    def test_every_step_calls_the_module(self) -> None:
        for call in (
            f'{CALL} commit-hash',
            f'{CALL} set-redirects --project "${{{{ vars.CF_PAGES_PROJECT }}}}"',
            f'{CALL} check-tag',
            f'{CALL} rename-artifacts --version "${{{{ steps.check.outputs.version }}}}"',
            f'{CALL} checksums',
        ):
            with self.subTest(call=call):
                self.assertTrue(call in self.text, call)

    def test_no_run_step_is_a_shell_block(self) -> None:
        self.assertNotIn('run: |', self.text)
        for line in self.text.splitlines():
            if 'run: ' in line:
                self.assertNotRegex(line.split('run: ', 1)[1], r'[;&|`]|\$\(')

    def test_both_jobs_can_see_the_module(self) -> None:
        for job in ('deploy-documentation', 'create-release'):
            with self.subTest(job=job):
                section = self.section(job)
                self.assertIn('uses: actions/checkout@', section)
                self.assertIn(PINNED_UV, section)
                self.assertLess(
                    section.index(PINNED_UV), section.index(f'{CALL} '), job
                )

    def test_the_uses_steps_survive(self) -> None:
        for step in (
            'cloudflare/wrangler-action@',
            'actions/download-artifact@',
            'softprops/action-gh-release@',
        ):
            with self.subTest(step=step):
                self.assertIn(step, self.text)


class ExceptionTests(unittest.TestCase):
    def test_no_gen_005_exception_left(self) -> None:
        data = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))
        stale = [
            entry
            for entry in data.get('exceptions', [])
            if entry.get('rule') == 'GEN-005' and entry.get('path') == WORKFLOW_PATH
        ]
        self.assertEqual(
            stale,
            [],
            'the GEN-005 exception for push.yaml must be deleted with the port',
        )


class ModuleTests(unittest.TestCase):
    def test_the_module_is_a_uv_script(self) -> None:
        self.assertTrue(MODULE.is_file(), f'{MODULE} must exist')
        first_line = MODULE.read_text(encoding='utf-8').splitlines()[0]
        self.assertEqual(first_line, '#!/usr/bin/env -S uv run --no-project --script')

    def test_the_module_imports_only_the_standard_library(self) -> None:
        text = MODULE.read_text(encoding='utf-8')
        for name in ('import argparse', 'import hashlib', 'import shutil'):
            self.assertIn(name, text)


if __name__ == '__main__':
    unittest.main()
