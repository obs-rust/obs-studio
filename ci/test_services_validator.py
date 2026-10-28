#!/usr/bin/env -S uv run --script
"""The services validator is Python calls, not shell blocks (#962, GEN-005).

    uv run python -m unittest ci/test_services_validator.py

`.github/actions/services-validator/action.yaml` carried three multi-line
`run:` steps -- the runner guard, the JSON-schema check and the defunct-service
check -- so the action produced a GEN-005 finding for each, and the `ci.toml`
exception is path-scoped: it leaves only when all three are gone. Each step is
now one call into `ci/services_validate.py`:

    guard     the action's own `if: runner.os == 'Windows'` selects it; it prints
              the notice and exits 2, exactly as the shell step did;
    schema    runs `.github/scripts/utils.py/check-jsonschema.py` through
              `uv run --no-project --with jsonschema --with json_source_map` and
              returns its status, bracketing the output in `::group::` annotations;
    defunct   runs `.github/scripts/utils.py/check-services.py` through
              `uv run --no-project --with requests --with aiohttp`, inheriting the
              step's environment (tokens, repository, run id, check API) and
              returning its status, including the `GITHUB_OUTPUT` it writes.

`guard` runs after `Set Up uv` now: the guard step used to run first, so a port
that calls `uv` needed the toolchain installed before it (a Windows runner has
no `uv` of its own).

RED first: committed while all three steps were still shell blocks, the module
did not exist and the GEN-005 `[[exceptions]]` block was still in `ci.toml`.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'services_validate.py'
ACTION = ROOT / '.github' / 'actions' / 'services-validator' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/services-validator/action.yaml'
CALL = 'uv run --no-project --script ${{ github.workspace }}/ci/services_validate.py'
NOTICE = 'services-validation action requires a macOS-based or Linux-based runner.'
SERVICES = ROOT / 'plugins' / 'rtmp-services' / 'data'

SCHEMA = {
    '$schema': 'http://json-schema.org/draft-07/schema#',
    'type': 'object',
    'required': ['name'],
    'properties': {'name': {'type': 'string'}},
}


def output(result: subprocess.CompletedProcess[str]) -> str:
    """The checker logs through `logging`, so its report is on stderr."""

    return result.stdout + result.stderr


def run_module(
    *args: str,
    cwd: str | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(MODULE), *args],
        capture_output=True,
        text=True,
        check=False,
        cwd=cwd,
        env=env,
    )


class GuardTests(unittest.TestCase):
    """The runner guard step: one notice, then a failing exit."""

    def test_guard_prints_the_notice_and_exits_2(self) -> None:
        result = run_module('guard', '--notice', NOTICE)
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertIn(f'::notice::{NOTICE}', result.stdout)

    def test_guard_takes_the_notice_as_its_only_argument(self) -> None:
        result = run_module('guard')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('--notice', result.stderr)


class SchemaTests(unittest.TestCase):
    """The schema step: the checker's status, in the annotations CI expects."""

    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        base = Path(self.directory.name)
        self.schema = base / 'schema.json'
        self.schema.write_text(json.dumps(SCHEMA), encoding='utf-8')
        self.valid = self.document('valid.json', {'name': 'ok'})
        # A property-level violation: the checker only records one when the
        # error has a path into the document, which is what `absolute_path` is
        # non-empty for.
        self.invalid = self.document('invalid.json', {'name': 5})

    def document(self, name: str, data: dict[str, object]) -> Path:
        data['$schema'] = 'schema.json'
        path = Path(self.directory.name) / name
        path.write_text(json.dumps(data), encoding='utf-8')
        return path

    def test_a_valid_file_passes(self) -> None:
        result = run_module('schema', str(self.valid))
        self.assertEqual(result.returncode, 0, output(result))

    def test_a_violation_fails_and_writes_the_annotation_input(self) -> None:
        result = run_module('schema', str(self.invalid), cwd=self.directory.name)
        self.assertEqual(result.returncode, 1, output(result))
        reported = Path(self.directory.name) / 'validation_errors.json'
        self.assertTrue(reported.is_file(), output(result))
        self.assertIn('Validation Error', reported.read_text(encoding='utf-8'))

    def test_a_missing_file_is_a_failure(self) -> None:
        result = run_module('schema', str(Path(self.directory.name) / 'absent.json'))
        self.assertEqual(result.returncode, 2, output(result))

    def test_the_run_is_bracketed_by_annotations(self) -> None:
        result = run_module('schema', str(self.valid))
        self.assertIn('::group::Schema Validation', result.stdout)
        self.assertIn('::endgroup::', result.stdout)

    def test_the_checked_in_service_files_pass(self) -> None:
        result = run_module(
            'schema', str(SERVICES / 'services.json'), str(SERVICES / 'package.json')
        )
        self.assertEqual(result.returncode, 0, output(result))


class UvStub:
    """A `uv` stand-in on PATH that records its argv and exits with a set code."""

    def __init__(self, directory: str, exit_code: int = 0) -> None:
        self.path = Path(directory) / 'uv'
        self.arguments = Path(directory) / 'argv.json'
        self.path.write_text(
            '#!/bin/sh\n' f'printf "%s\\n" "$@" > {self.arguments}\n' f'exit {exit_code}\n',
            encoding='utf-8',
        )
        self.path.chmod(0o755)

    def environment(self) -> dict[str, str]:
        return dict(os.environ, PATH=f'{self.path.parent}{os.pathsep}{os.environ["PATH"]}')

    def argv(self) -> list[str]:
        return self.arguments.read_text(encoding='utf-8').splitlines()


class DefunctTests(unittest.TestCase):
    """The defunct-service step: one script call, the step's env, its status."""

    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)

    def test_the_step_runs_the_checked_in_script_through_uv(self) -> None:
        stub = UvStub(self.directory.name)
        environment = dict(stub.environment(), REPOSITORY='obs-rust/obs-studio')
        result = run_module('defunct', env=environment)
        self.assertEqual(result.returncode, 0, output(result))
        argv = stub.argv()
        self.assertEqual(argv[:3], ['run', '--no-project', '--with'])
        self.assertIn('requests', argv)
        self.assertIn('aiohttp', argv)
        self.assertTrue(
            any(argument.endswith('check-services.py') for argument in argv), argv
        )

    def test_the_checkers_status_is_returned(self) -> None:
        stub = UvStub(self.directory.name, exit_code=1)
        result = run_module('defunct', env=stub.environment())
        self.assertEqual(result.returncode, 1, output(result))


class ActionTests(unittest.TestCase):
    """The action itself: one line per step, no shell block left."""

    def setUp(self) -> None:
        self.text = ACTION.read_text(encoding='utf-8')

    def test_the_action_calls_the_module_for_all_three_steps(self) -> None:
        for command in ('guard', 'schema', 'defunct'):
            self.assertIn(f'{CALL} {command}', self.text)

    def test_no_run_step_is_a_shell_block(self) -> None:
        self.assertNotIn('run: |', self.text)
        for line in self.text.splitlines():
            if 'run: ' in line:
                command = line.split('run: ', 1)[1]
                self.assertNotRegex(command, r'[;&|`]|\$\(', line)

    def test_uv_is_installed_before_every_call(self) -> None:
        setup_at = self.text.index('astral-sh/setup-uv@')
        self.assertIn('cache-suffix:', self.text)
        for command in ('guard', 'schema', 'defunct'):
            self.assertLess(setup_at, self.text.index(f'{CALL} {command}'))

    def test_the_module_path_is_absolute_for_a_custom_working_directory(self) -> None:
        self.assertIn('${{ github.workspace }}/ci/services_validate.py', self.text)

    def test_the_schema_step_keeps_its_working_directory(self) -> None:
        self.assertIn('working-directory: ${{ inputs.workingDirectory }}', self.text)

    def test_the_schema_step_still_names_both_data_files(self) -> None:
        self.assertIn('plugins/rtmp-services/data/services.json', self.text)
        self.assertIn('plugins/rtmp-services/data/package.json', self.text)

    def test_the_defunct_step_keeps_its_inputs_in_the_environment(self) -> None:
        for name in ('GITHUB_TOKEN', 'WORKFLOW_RUN_ID', 'REPOSITORY', 'API_KEY'):
            self.assertIn(name, self.text)


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
            'the GEN-005 exception for services-validator must be deleted with the port',
        )


class ModuleTests(unittest.TestCase):
    def test_the_module_is_a_uv_script(self) -> None:
        self.assertTrue(MODULE.is_file(), f'{MODULE} must exist')
        first_line = MODULE.read_text(encoding='utf-8').splitlines()[0]
        self.assertEqual(first_line, '#!/usr/bin/env -S uv run --no-project --script')


if __name__ == '__main__':
    unittest.main()
