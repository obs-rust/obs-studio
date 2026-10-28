#!/usr/bin/env -S uv run --script
"""The swift-format action is Python calls, not shell blocks (#962, GEN-005).

    uv run python -m unittest ci/test_run_swift_format.py

`.github/actions/run-swift-format/action.yaml` carried three multi-line `run:`
steps -- the Windows runner guard, the Linux Homebrew dependency step and the
`Run swift-format` step -- so `ci-lint precheck` reported a GEN-005 approved
exception for each. All three are one call into `ci/run_swift_format.py` now:

    guard         the action's own `if: runner.os == 'Windows'` selects it; it
                  prints the notice and exits 2, as the shell step did;
    install-deps  the Linux step's `brew update` / `brew install --quiet zsh`,
                  through `.github/scripts/retry` and with the linuxbrew bin
                  directories appended to `$GITHUB_PATH`;
    format        `brew install --quiet swift-format`, then the same
                  `build-aux/run-swift-format --fail-<c> --check <files>` argv,
                  over the same `CHANGED_FILES` split the zsh line did.

`Set Up uv` moved above the guard: the guard now runs through `uv`, and the
`swift-format` job on `macos-26` installs no toolchain of its own.

RED first: committed while the steps were still shell blocks and the module did
not exist, so every `test_module_*` case failed and the `ci.toml` assertion did
too.
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'run_swift_format.py'
ACTION = ROOT / '.github' / 'actions' / 'run-swift-format' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/run-swift-format/action.yaml'
CALL = 'uv run --no-project --script ${{ github.workspace }}/ci/run_swift_format.py'
SETUP_UV = 'astral-sh/setup-uv'
NOTICE = 'run-swift-format action requires a macOS-based or Linux-based runner.'


def load_module():
    """Import the module by path, the way the sibling `ci/test_*.py` loaders do."""

    if not MODULE.exists():
        raise FileNotFoundError(MODULE)
    spec = importlib.util.spec_from_file_location('run_swift_format', MODULE)
    if spec is None or spec.loader is None:
        raise ImportError(MODULE)
    module = importlib.util.module_from_spec(spec)
    sys.modules['run_swift_format'] = module
    spec.loader.exec_module(module)
    return module


def write_stub(path: Path, body: str) -> Path:
    """A `/bin/sh` stub that appends its argv to `path.argv` and exits 0."""

    path.write_text(f'#!/bin/sh\n{body}\n', encoding='utf-8')
    path.chmod(0o755)
    return path


class SplitChangedFilesTest(unittest.TestCase):
    def test_comma_separated_list(self) -> None:
        module = load_module()
        self.assertEqual(
            module.split_changed_files("['Sources/A.swift','Sources/B.swift']"),
            ['Sources/A.swift', 'Sources/B.swift'],
        )

    def test_quotes_and_brackets_are_stripped(self) -> None:
        module = load_module()
        self.assertEqual(
            module.split_changed_files('["a.swift","b.swift"]'),
            ['a.swift', 'b.swift'],
        )

    def test_empty_value_is_no_file(self) -> None:
        module = load_module()
        self.assertEqual(module.split_changed_files(''), [])
        self.assertEqual(module.split_changed_files('[]'), [])


class GuardTest(unittest.TestCase):
    def test_guard_prints_the_notice_and_fails(self) -> None:
        module = load_module()
        with tempfile.TemporaryDirectory() as scratch:
            completed = subprocess.run(
                [sys.executable, str(MODULE), 'guard', '--notice', NOTICE],
                capture_output=True,
                text=True,
                cwd=scratch,
                check=False,
            )
        self.assertEqual(completed.returncode, 2, completed.stderr)
        self.assertIn(NOTICE, completed.stdout)


class InstallDependenciesTest(unittest.TestCase):
    def test_brew_update_and_zsh_install(self) -> None:
        module = load_module()
        with tempfile.TemporaryDirectory() as scratch:
            directory = Path(scratch)
            argv_log = directory / 'argv.txt'
            retry = write_stub(
                directory / 'retry', f'echo "$@" >> "{argv_log}"\nexit 0'
            )
            github_path = directory / 'github_path'
            module.RETRY = retry
            previous = os.environ.get('GITHUB_PATH')
            os.environ['GITHUB_PATH'] = str(github_path)
            try:
                status = module.install_dependencies()
            finally:
                if previous is None:
                    os.environ.pop('GITHUB_PATH', None)
                else:
                    os.environ['GITHUB_PATH'] = previous

            self.assertEqual(status, 0)
            self.assertEqual(
                argv_log.read_text(encoding='utf-8').splitlines(),
                ['brew update', 'brew install --quiet zsh'],
            )
            exported = github_path.read_text(encoding='utf-8').splitlines()
            self.assertEqual(
                exported,
                [
                    '/home/linuxbrew/.linuxbrew/bin',
                    '/home/linuxbrew/.linuxbrew/sbin',
                ],
            )


class FormatTest(unittest.TestCase):
    def test_formatter_argv_and_status(self) -> None:
        module = load_module()
        with tempfile.TemporaryDirectory() as scratch:
            directory = Path(scratch)
            argv_log = directory / 'argv.txt'
            retry = write_stub(
                directory / 'retry', f'echo "$@" >> "{argv_log}"\nexit 0'
            )
            formatter = write_stub(
                directory / 'run-swift-format',
                'echo "$@" > "$FORMATTER_ARGV"\nexit 7',
            )
            formatter_argv = directory / 'formatter-argv.txt'
            module.RETRY = retry
            module.FORMATTER = formatter
            previous = os.environ.get('CHANGED_FILES')
            previous_argv = os.environ.get('FORMATTER_ARGV')
            os.environ['CHANGED_FILES'] = "['Sources/A.swift','Sources/B.swift']"
            os.environ['FORMATTER_ARGV'] = str(formatter_argv)
            try:
                status = module.format_swift('error')
            finally:
                for key, value in (
                    ('CHANGED_FILES', previous),
                    ('FORMATTER_ARGV', previous_argv),
                ):
                    if value is None:
                        os.environ.pop(key, None)
                    else:
                        os.environ[key] = value

            self.assertEqual(status, 7)
            self.assertEqual(
                argv_log.read_text(encoding='utf-8').splitlines(),
                ['brew update', 'brew install --quiet swift-format'],
            )
            self.assertEqual(
                formatter_argv.read_text(encoding='utf-8').split(),
                ['--fail-error', '--check', 'Sources/A.swift', 'Sources/B.swift'],
            )


class ActionYamlTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = ACTION.read_text(encoding='utf-8')

    def test_every_step_is_one_uv_call(self) -> None:
        calls = [
            line.strip().removeprefix('run: ')
            for line in self.text.splitlines()
            if line.strip().startswith('run: uv run')
        ]
        self.assertEqual(len(calls), 3, calls)
        for call in calls:
            self.assertTrue(call.startswith(CALL), call)
            self.assertNotIn('&&', call)
            self.assertNotIn('$()', call)

    def test_no_shell_override_is_left(self) -> None:
        self.assertNotIn('shell: zsh', self.text)
        self.assertNotIn('brew ', self.text)

    def test_uv_is_installed_above_the_guard(self) -> None:
        lines = self.text.splitlines()
        setup = next(
            index for index, line in enumerate(lines) if SETUP_UV in line
        )
        guard = next(
            index for index, line in enumerate(lines) if ' guard ' in line
        )
        self.assertLess(setup, guard)
        self.assertIn('save-cache:', self.text)
        self.assertIn('cache-suffix:', self.text)
        self.assertIn('pr-{0}', self.text)


class ExceptionTest(unittest.TestCase):
    def test_the_path_has_no_gen005_exception(self) -> None:
        document = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))
        remaining = [
            exception
            for exception in document.get('exceptions', [])
            if exception.get('rule') == 'GEN-005'
            and exception.get('path') == ACTION_PATH
        ]
        self.assertEqual(remaining, [])

    def test_the_formatter_script_keeps_its_own_exception(self) -> None:
        document = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))
        paths = [
            exception.get('path')
            for exception in document.get('exceptions', [])
            if exception.get('rule') == 'GEN-005'
        ]
        self.assertIn('build-aux/run-swift-format', paths)


if __name__ == '__main__':
    unittest.main()
