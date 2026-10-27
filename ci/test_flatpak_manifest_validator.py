#!/usr/bin/env -S uv run --script
"""The Flatpak manifest validator is Python calls, not shell blocks (#962, GEN-005).

    uv run python -m unittest ci/test_flatpak_manifest_validator.py

`.github/actions/flatpak-manifest-validator/action.yaml` carried two multi-line
`run:` steps -- the Windows runner guard and the manifest order check -- so the
action produced a GEN-005 finding for each. Both are now one call into
`ci/flatpak_manifest_validate.py`:

    guard     the action's own `if: runner.os == 'Windows'` selects it; it prints
              the notice and exits 2, exactly as the shell step did;
    validate  runs `build-aux/format-manifest.py --check` (standard library only)
              and returns its status, except under `failCondition: never`, where
              the old step's `set +e` made the report advisory.

`guard` runs after `Set Up uv` now: the guard step used to run first, so a
port that calls `uv` needed the toolchain installed before it (a Windows runner
has no `uv` of its own).

RED first: committed while both steps were still shell blocks, the module did
not exist and the GEN-005 `[[exceptions]]` block was still in `ci.toml`.
"""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'flatpak_manifest_validate.py'
ACTION = ROOT / '.github' / 'actions' / 'flatpak-manifest-validator' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/flatpak-manifest-validator/action.yaml'
CALL = 'uv run --no-project --script ${{ github.workspace }}/ci/flatpak_manifest_validate.py'
NOTICE = (
    'services-validation action requires a macOS-based or Linux-based runner.'
)

FORMATTED = '{\n    "a": 1\n}\n'
UNFORMATTED = '{"a":1,"b":[2,3]}'


def output(result: subprocess.CompletedProcess[str]) -> str:
    """The formatter logs through `logging`, so its report is on stderr."""

    return result.stdout + result.stderr


def run_module(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(MODULE), *args],
        capture_output=True,
        text=True,
        check=False,
    )


class GuardTests(unittest.TestCase):
    """The Windows guard step: one notice, then a failing exit."""

    def test_guard_prints_the_notice_and_exits_2(self) -> None:
        result = run_module('guard', '--notice', NOTICE)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn(f'::notice::{NOTICE}', result.stdout)

    def test_guard_takes_the_notice_as_its_only_argument(self) -> None:
        result = run_module('guard')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('--notice', result.stderr)


class ValidateTests(unittest.TestCase):
    """The manifest step: the formatter's status, unless the report is advisory."""

    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.formatted = self.manifest('formatted.json', FORMATTED)
        self.unformatted = self.manifest('unformatted.json', UNFORMATTED)

    def manifest(self, name: str, text: str) -> Path:
        path = Path(self.directory.name) / name
        path.write_text(text, encoding='utf-8')
        return path

    def test_a_formatted_manifest_passes(self) -> None:
        result = run_module(
            'validate', '--manifest', str(self.formatted), '--fail-condition', 'error'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('passed order validation', output(result))

    def test_an_unformatted_manifest_fails_when_the_report_is_enforced(self) -> None:
        result = run_module(
            'validate', '--manifest', str(self.unformatted), '--fail-condition', 'error'
        )
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn('not correctly formatted', output(result))

    def test_an_unformatted_manifest_passes_when_the_report_is_never(self) -> None:
        result = run_module(
            'validate', '--manifest', str(self.unformatted), '--fail-condition', 'never'
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('not correctly formatted', output(result))

    def test_the_report_is_advisory_by_default(self) -> None:
        result = run_module('validate', '--manifest', str(self.unformatted))
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_missing_manifest_is_a_failure(self) -> None:
        result = run_module(
            'validate',
            '--manifest',
            str(Path(self.directory.name) / 'absent.json'),
            '--fail-condition',
            'error',
        )
        self.assertEqual(result.returncode, 2, result.stderr)

    def test_the_default_manifest_is_the_checked_in_one(self) -> None:
        result = run_module('validate', '--fail-condition', 'error')
        self.assertEqual(result.returncode, 0, output(result))
        self.assertIn('passed order validation', output(result))

    def test_the_run_is_bracketed_by_annotations(self) -> None:
        result = run_module('validate', '--manifest', str(self.formatted))
        self.assertIn('::group::Run Validation', result.stdout)
        self.assertIn('::endgroup::', result.stdout)


class ActionTests(unittest.TestCase):
    """The action itself: one line per step, no shell block left."""

    def setUp(self) -> None:
        self.text = ACTION.read_text(encoding='utf-8')

    def test_the_action_calls_the_module_for_both_steps(self) -> None:
        self.assertIn(f'{CALL} guard', self.text)
        self.assertIn(f'{CALL} validate', self.text)

    def test_no_run_step_is_a_shell_block(self) -> None:
        self.assertNotIn('run: |', self.text)
        for line in self.text.splitlines():
            if 'run: ' in line:
                command = line.split('run: ', 1)[1]
                self.assertNotRegex(command, r'[;&|`]|\$\(', line)

    def test_uv_is_installed_before_both_calls(self) -> None:
        setup_at = self.text.index('astral-sh/setup-uv@')
        self.assertIn('cache-suffix:', self.text)
        self.assertLess(setup_at, self.text.index(f'{CALL} guard'))
        self.assertLess(setup_at, self.text.index(f'{CALL} validate'))

    def test_the_module_path_is_absolute_for_a_custom_working_directory(self) -> None:
        self.assertIn('${{ github.workspace }}/ci/flatpak_manifest_validate.py', self.text)


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
            'the GEN-005 exception for flatpak-manifest-validator must be deleted'
            ' with the port',
        )


class ModuleTests(unittest.TestCase):
    def test_the_module_is_a_uv_script(self) -> None:
        self.assertTrue(MODULE.is_file(), f'{MODULE} must exist')
        first_line = MODULE.read_text(encoding='utf-8').splitlines()[0]
        self.assertEqual(first_line, '#!/usr/bin/env -S uv run --no-project --script')

    def test_the_module_imports_only_the_standard_library(self) -> None:
        text = MODULE.read_text(encoding='utf-8')
        for name in ('import argparse', 'import subprocess', 'import sys'):
            self.assertIn(name, text)


if __name__ == '__main__':
    unittest.main()
