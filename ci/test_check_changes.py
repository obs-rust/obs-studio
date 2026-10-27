#!/usr/bin/env -S uv run --script
"""The changed-file gate is one Python call, not a shell block (#962, GEN-005).

    uv run python -m unittest ci/test_check_changes.py

`.github/actions/check-changes/action.yaml`'s step ran a 50-line shell block that
resolved a base reference from the event and diffed it -- a GEN-005 finding whose
fix is "one call to a Python script". The step is now a single
`uv run --no-project --script ci/check_changes.py` line, the module below does the
work, and the action's GEN-005 `[[exceptions]]` block is gone.

The action is called from `ci.yml` (3 sites), `push.yaml` (3), `scheduled.yaml`
(1), and from the `qt-xml-validator` and `run-swift-format` composite actions, so
the cases here are the base-reference rules the shell implemented: an explicit
`baseRef`, a push's `before` commit, a dispatch's merge base with the default
branch, the `useFallback` warning path, the diff filter, and the empty glob.

RED first: committed while the step was still shell and the module did not
exist; every test here failed then (no module to import, the run step was a
`run: |` block, and the ci.toml exception was still present).
"""

from __future__ import annotations

import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'check_changes.py'
ACTION = ROOT / '.github' / 'actions' / 'check-changes' / 'action.yaml'
CI_TOML = ROOT / 'ci.toml'
ACTION_PATH = '.github/actions/check-changes/action.yaml'
CALL = 'uv run --no-project --script ci/check_changes.py'

FILES = {
    'docs/sphinx/conf.py': 'v1\n',
    'plugins/rtmp-services/data/services.json': '{}\n',
    'plugins/win-capture/data/compatibility.json': '{}\n',
    'frontend/forms/OBSBasic.ui': '<ui/>\n',
    'frontend/forms/sub/Other.ui': '<ui/>\n',
    'cmake/common/x.cmake': '#\n',
    'libobs/obs.c': '/* c */\n',
}


def load_module():
    spec = importlib.util.spec_from_file_location('check_changes', MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def git(*arguments: str, cwd: Path) -> str:
    result = subprocess.run(
        ['git', *arguments], cwd=cwd, capture_output=True, text=True, check=True
    )
    return result.stdout.strip()


class Repository:
    """Three commits: the base, a libobs change, then a mixed change."""

    def __init__(self, root: Path) -> None:
        self.path = root / 'repo'
        self.path.mkdir()
        git('init', '-q', '-b', 'master', cwd=self.path)
        git('config', 'user.email', 'test@example.com', cwd=self.path)
        git('config', 'user.name', 'test', cwd=self.path)
        for name, text in FILES.items():
            write(self.path / name, text)
        commit(self.path, 'one')
        write(self.path / 'libobs/obs.c', '/* changed */\n')
        commit(self.path, 'two')
        write(self.path / 'plugins/rtmp-services/data/services.json', '{"a":1}\n')
        write(self.path / 'frontend/forms/sub/Other.ui', '<ui/>\n<!-- x -->\n')
        write(self.path / 'cmake/common/x.cmake', '# changed\n')
        commit(self.path, 'three')
        git('update-ref', 'refs/remotes/origin/master', 'HEAD~2', cwd=self.path)

    def rev(self, revision: str) -> str:
        return git('rev-parse', revision, cwd=self.path)


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding='utf-8')


def commit(repo: Path, message: str) -> None:
    git('add', '-A', cwd=repo)
    git('commit', '-qm', message, cwd=repo)


class ChangedFilesTest(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.repo = Repository(Path(self.directory.name))
        self.module = load_module()

    def check(self, check_glob: str, **environment: str) -> dict[str, str]:
        settings = {'GIT_REF': 'HEAD', 'USE_FALLBACK': 'true', **environment}
        return self.module.check(
            check_glob=check_glob, cwd=self.repo.path, environment=settings
        )

    def test_explicit_base_ref_limits_to_the_glob(self):
        outputs = self.check(
            'plugins/rtmp-services/data/*.json', GIT_BASE_REF='HEAD~1'
        )
        self.assertEqual(outputs['hasChangedFiles'], 'true')
        self.assertEqual(
            outputs['changedFiles'],
            '["plugins/rtmp-services/data/services.json"]',
        )

    def test_no_changed_file_reports_an_empty_list(self):
        outputs = self.check('docs/nothing/*.json', GIT_BASE_REF='HEAD~1')
        self.assertEqual(outputs['hasChangedFiles'], 'false')
        self.assertEqual(outputs['changedFiles'], '[]')

    def test_empty_glob_means_every_path(self):
        outputs = self.check('', GIT_BASE_REF='HEAD~1')
        self.assertEqual(outputs['hasChangedFiles'], 'true')
        self.assertEqual(
            outputs['changedFiles'],
            '["cmake/common/x.cmake","frontend/forms/sub/Other.ui",'
            '"plugins/rtmp-services/data/services.json"]',
        )

    def test_push_compares_against_the_before_commit(self):
        outputs = self.check(
            'plugins/rtmp-services/data/*.json',
            GITHUB_EVENT_NAME='push',
            GITHUB_REF_BEFORE=self.repo.rev('HEAD~1'),
        )
        self.assertEqual(outputs['changedFiles'], '["plugins/rtmp-services/data/services.json"]')

    def test_forced_push_falls_back_to_head_parent(self):
        outputs = self.check(
            'plugins/rtmp-services/data/*.json',
            GITHUB_EVENT_NAME='push',
            GITHUB_EVENT_FORCED='true',
            GITHUB_REF_BEFORE=self.repo.rev('HEAD~2'),
        )
        self.assertEqual(outputs['changedFiles'], '["plugins/rtmp-services/data/services.json"]')

    def test_dispatch_compares_against_the_merge_base(self):
        outputs = self.check(
            'cmake/*', GITHUB_EVENT_NAME='workflow_dispatch', DEFAULT_BRANCH='master'
        )
        self.assertEqual(outputs['changedFiles'], '["cmake/common/x.cmake"]')

    def test_invalid_base_ref_warns_and_falls_back(self):
        with mock.patch('builtins.print') as printed:
            outputs = self.check('libobs/*', GIT_BASE_REF='no-such-ref')
        self.assertIn(
            '::warning::Provided base reference no-such-ref is invalid',
            [call.args[0] for call in printed.call_args_list],
        )
        self.assertEqual(outputs['changedFiles'], '[]')

    def test_invalid_base_ref_without_fallback_is_an_error(self):
        with mock.patch('builtins.print'):
            with self.assertRaises(SystemExit):
                self.check('libobs/*', GIT_BASE_REF='no-such-ref', USE_FALLBACK='false')

    def test_extglob_negation_excludes_the_matching_top_level_entries(self):
        outputs = self.check(
            '!(cmake*)',
            GITHUB_EVENT_NAME='push',
            GITHUB_REF_BEFORE=self.repo.rev('HEAD~1'),
        )
        self.assertEqual(
            outputs['changedFiles'],
            '["frontend/forms/sub/Other.ui",'
            '"plugins/rtmp-services/data/services.json"]',
        )

    def test_double_star_glob_matches_the_globs_the_callers_use(self):
        outputs = self.check('frontend/forms/**/*.ui', GIT_BASE_REF='HEAD~1')
        self.assertEqual(outputs['changedFiles'], '["frontend/forms/sub/Other.ui"]')

    def test_diff_filter_keeps_only_the_named_kind(self):
        outputs = self.check(
            'plugins/rtmp-services/data/*.json', GIT_BASE_REF='HEAD~1', DIFF_FILTER='A'
        )
        self.assertEqual(outputs['hasChangedFiles'], 'false')

    def test_files_with_spaces_stay_one_entry(self):
        write(self.repo.path / 'docs/sphinx/my notes.py', 'v2\n')
        commit(self.repo.path, 'four')
        outputs = self.check('docs/sphinx/*', GIT_BASE_REF='HEAD~1')
        self.assertEqual(outputs['changedFiles'], '["docs/sphinx/my notes.py"]')


class OutputsTest(unittest.TestCase):
    def test_outputs_are_written_to_the_step_file(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as raw:
            repo = Repository(Path(raw))
            output_file = Path(raw) / 'github_output'
            environment = {
                'GITHUB_OUTPUT': str(output_file),
                'GIT_REF': 'HEAD',
                'GIT_BASE_REF': 'HEAD~1',
                'USE_FALLBACK': 'true',
            }
            with mock.patch.dict(os.environ, environment, clear=True):
                module.main(
                    ['--check-glob', '', '--repository', str(repo.path)]
                )
            self.assertEqual(
                output_file.read_text(encoding='utf-8'),
                'hasChangedFiles=true\n'
                'changedFiles=["cmake/common/x.cmake",'
                '"frontend/forms/sub/Other.ui",'
                '"plugins/rtmp-services/data/services.json"]\n',
            )

    def test_write_outputs_ignores_a_missing_path(self):
        module = load_module()
        module.write_outputs({'hasChangedFiles': 'false'}, None)


class CheckChangesActionWiringTest(unittest.TestCase):
    def test_action_step_is_one_python_call(self):
        text = ACTION.read_text(encoding='utf-8')
        self.assertIn(CALL, text, 'the step must call the module through uv')
        check_step = text.split('id: checks', 1)[1]
        run_lines = [line for line in check_step.splitlines() if 'run:' in line]
        self.assertEqual(
            len(run_lines),
            1,
            'the step keeps exactly one run: line (GEN-005 shell budget)',
        )
        self.assertNotIn('run: |', check_step, 'the step must not be a shell block')

    def test_action_installs_uv_before_the_call(self):
        text = ACTION.read_text(encoding='utf-8')
        self.assertIn('astral-sh/setup-uv@', text)
        self.assertIn('cache-suffix:', text)
        self.assertLess(text.index('astral-sh/setup-uv@'), text.index(CALL))

    def test_no_gen_005_exception_left(self):
        data = tomllib.loads(CI_TOML.read_text(encoding='utf-8'))
        stale = [
            entry
            for entry in data.get('exceptions', [])
            if entry.get('rule') == 'GEN-005' and entry.get('path') == ACTION_PATH
        ]
        self.assertEqual(
            stale,
            [],
            'the GEN-005 exception for check-changes must be deleted with the port',
        )


if __name__ == '__main__':
    unittest.main()
