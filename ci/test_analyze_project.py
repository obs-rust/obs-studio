"""Pins the GEN-005 port of the macOS analysis steps (obs-rust/obs-studio#3040).

The workflow itself only runs from `scheduled.yaml` (`workflow_call`), so no PR
lane executes it: these tests are the offline evidence. They assert the shape
the port removed (no multi-line `run:` step, no `shell: pwsh`, no ci.toml
exception) and drive each subcommand on fixtures, including a differential run
of the old `jq -s` program when `jq` is on PATH.

Run: uv run --no-project python -m unittest ci/test_analyze_project.py
"""

import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / 'ci' / 'analyze_project.py'
WORKFLOW = ROOT / '.github' / 'workflows' / 'analyze-project.yaml'
CI_TOML = ROOT / 'ci.toml'

spec = importlib.util.spec_from_file_location('analyze_project', MODULE)
assert spec is not None and spec.loader is not None
analyze_project = importlib.util.module_from_spec(spec)
spec.loader.exec_module(analyze_project)

# The `jq -s` program the ported step ran, with every object value that is a
# compound expression parenthesized (#3051). jq 1.7 accepts only a bare term as
# an object value, so `"driver": (… ) + {…}` and `"artifacts": reduce(…) | unique`
# are compile errors there; jq 1.8 accepts both. The parentheses do not change
# the meaning, so the differential case below is the same test on every version.
JQ_PROGRAM = '''
{
  "$schema": first(.[]."$schema"),
  "version": first(.[].version),
  "runs": [{
    "tool": {
      "driver": ((first(.[].runs[].tool.driver) | del(.rules)) + {
        "rules": (reduce(.[].runs[].tool.driver.rules) as $obj ([]; . + $obj) | unique)
      })
    },
    "artifacts": (reduce(.[].runs[].artifacts) as $obj ([]; . + $obj) | unique),
    "results": ((reduce(.[].runs[].results) as $obj ([]; . + $obj))
      | del(
        .[].codeFlows[].threadFlows[].locations[].location.physicalLocation.region.endLine,
        .[].codeFlows[].threadFlows[].locations[].location.physicalLocation.region.endColumn
        | select(. == 0)
      ))
  }]
}
'''


def driver(name):
    return {'name': name, 'version': '20.1.0'}


def sarif_document(name, artifact, result):
    return {
        '$schema': 'https://json.schemastore.org/sarif-2.1.0.json',
        'version': '2.1.0',
        'runs': [
            {
                'tool': {'driver': dict(driver('clang'), rules=[{'id': 'core.NullDereference'}])},
                'artifacts': [{'location': {'uri': artifact}}],
                'results': [result],
            }
        ],
    }


def zero_region_result(entity):
    return {
        'ruleId': 'core.NullDereference',
        'message': {'text': f'warning in {entity}'},
        'locations': [{'physicalLocation': {'artifactLocation': {'uri': entity}, 'region': {'startLine': 3}}}],
        'codeFlows': [
            {
                'threadFlows': [
                    {
                        'locations': [
                            {
                                'location': {
                                    'physicalLocation': {
                                        'artifactLocation': {'uri': entity},
                                        'region': {'startLine': 1, 'endLine': 0, 'endColumn': 0},
                                    }
                                }
                            }
                        ]
                    }
                ]
            }
        ],
    }


def write_fixtures(root):
    """Two analyzer reports: a duplicated rule, a duplicated artifact, one zero region."""

    root.mkdir(parents=True, exist_ok=True)
    first = sarif_document('first', 'libobs/obs.c', zero_region_result('libobs/obs.c'))
    second = sarif_document('second', 'libobs/obs.c', zero_region_result('libobs/obs-video.c'))
    second['runs'][0]['tool']['driver']['rules'].append({'id': 'core.UndefinedBinaryOperatorResult'})
    (root / 'a.sarif').write_text(json.dumps(first), encoding='utf-8')
    (root / 'b.sarif').write_text(json.dumps(second), encoding='utf-8')
    return first, second


class WorkflowShape(unittest.TestCase):
    def test_no_multiline_run_step(self):
        text = WORKFLOW.read_text(encoding='utf-8')
        self.assertNotIn('run: |', text)
        self.assertNotIn('shell: pwsh', text)

    def test_each_step_is_one_module_call(self):
        text = WORKFLOW.read_text(encoding='utf-8')
        for command in ('setup', 'build', 'merge-sarif'):
            self.assertIn(f'uv run --no-project --script ci/analyze_project.py {command}', text)

    def test_ci_toml_has_no_gen_005_block_for_the_workflow(self):
        for block in CI_TOML.read_text(encoding='utf-8').split('[[exceptions]]'):
            if 'rule = "GEN-005"' in block:
                self.assertNotIn('.github/workflows/analyze-project.yaml', block)


class Recording:
    """A `run` stand-in that records argv instead of executing anything."""

    def __init__(self, status=0):
        self.calls = []
        self.status = status

    def __call__(self, argv, cwd):
        self.calls.append((list(argv), cwd))
        return self.status


class Setup(unittest.TestCase):
    def setUp(self):
        self.home = Path(tempfile.mkdtemp())
        self.output = self.home / 'github_output'
        self.environ = {'HOME': str(self.home), 'GITHUB_OUTPUT': str(self.output), 'HOMEBREW_PREFIX': str(self.home / 'brew')}

    def tearDown(self):
        shutil.rmtree(self.home, ignore_errors=True)

    def test_switches_xcode_and_writes_the_cache_path(self):
        recorder = Recording()
        status = analyze_project.setup(run=recorder, environ=self.environ, output=self.output)
        self.assertEqual(status, 0)
        self.assertEqual(recorder.calls[0][0], ['sudo', 'xcode-select', '--switch', analyze_project.XCODE])
        written = self.output.read_text(encoding='utf-8').splitlines()
        self.assertEqual(len(written), 1)
        key, value = written[0].split('=', 1)
        self.assertEqual(key, 'xcodeCasPath')
        self.assertTrue(Path(value).is_dir())
        self.assertEqual(Path(value).name, 'CompilationCache.noindex')

    def test_skips_brew_when_no_formula_is_unwanted(self):
        recorder = Recording()
        analyze_project.setup(run=recorder, environ=self.environ, output=self.output)
        self.assertFalse(any(call[0][0] == 'brew' for call in recorder.calls))

    def test_uninstalls_an_unwanted_formula_that_is_installed(self):
        formula = 'zsh'
        (self.home / 'brew' / 'Cellar' / formula).mkdir(parents=True)
        original = analyze_project.UNWANTED_FORMULAS
        setattr(analyze_project, 'UNWANTED_FORMULAS', (formula, 'ksh'))
        try:
            recorder = Recording()
            analyze_project.setup(run=recorder, environ=self.environ, output=self.output)
        finally:
            setattr(analyze_project, 'UNWANTED_FORMULAS', original)
        brew = [call for call in recorder.calls if call[0][0] == 'brew']
        self.assertEqual(brew, [(['brew', 'uninstall', '--ignore-dependencies', formula], None)])

    def test_stops_when_xcode_select_fails(self):
        recorder = Recording(status=1)
        status = analyze_project.setup(run=recorder, environ=self.environ, output=self.output)
        self.assertEqual(status, 1)
        self.assertFalse(self.output.exists())


class Build(unittest.TestCase):
    def test_refreshes_origin_then_runs_the_script(self):
        recorder = Recording()
        status = analyze_project.build(run=recorder, environ={})
        self.assertEqual(status, 0)
        self.assertEqual(recorder.calls[0], (['git', 'fetch', 'origin', '--no-tags', '--no-recurse-submodules', '-q'], ROOT))
        argv, cwd = recorder.calls[1]
        self.assertEqual(argv[0], str(analyze_project.BUILD_SCRIPT))
        self.assertEqual(argv[1:], ['--config', 'Debug', '--target', 'macos-arm64', '--codesign', '--analyze'])
        self.assertEqual(cwd, ROOT)

    def test_runner_debug_adds_the_debug_flag(self):
        recorder = Recording()
        analyze_project.build(run=recorder, environ={'RUNNER_DEBUG': '1'})
        self.assertEqual(recorder.calls[1][0][-1], '--debug')

    def test_skips_the_build_when_the_fetch_fails(self):
        recorder = Recording(status=1)
        status = analyze_project.build(run=recorder, environ={})
        self.assertEqual(status, 1)
        self.assertEqual(len(recorder.calls), 1)


class MergeSarif(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp())

    def tearDown(self):
        shutil.rmtree(self.root, ignore_errors=True)

    def test_flattens_plists_and_merges_them(self):
        source = self.root / 'StaticAnalyzer' / 'obs-studio' / 'arm64' / 'obs.c.plist'
        source.parent.mkdir(parents=True)
        source.write_text(json.dumps(sarif_document('obs', 'libobs/obs.c', zero_region_result('libobs/obs.c'))), encoding='utf-8')
        self.assertEqual(analyze_project.merge_sarif(self.root), 0)
        self.assertEqual(sorted(path.name for path in self.root.glob('*.sarif')), ['merged.sarif', 'obs.c.sarif'])
        merged = json.loads((self.root / 'merged.sarif').read_text(encoding='utf-8'))
        self.assertEqual(merged['runs'][0]['tool']['driver']['rules'], [{'id': 'core.NullDereference'}])
        self.assertNotIn('endLine', merged['runs'][0]['results'][0]['codeFlows'][0]['threadFlows'][0]['locations'][0]['location']['physicalLocation']['region'])

    def test_without_reports_is_an_error(self):
        self.assertEqual(analyze_project.merge_sarif(self.root), 1)
        self.assertFalse((self.root / 'merged.sarif').exists())

    def test_matches_the_old_jq_program(self):
        jq = shutil.which('jq')
        if jq is None:
            self.skipTest('jq is not on PATH')
        module_side = self.root / 'module'
        shell_root = self.root / 'shell'
        write_fixtures(module_side)
        write_fixtures(shell_root)
        self.assertEqual(analyze_project.merge_sarif(module_side), 0)
        with (shell_root / 'merged.sarif').open('w', encoding='utf-8') as handle:
            completed = subprocess.run(
                [jq, '-s', JQ_PROGRAM, *sorted(path.name for path in shell_root.glob('*.sarif'))],
                cwd=shell_root,
                stdout=handle,
                stderr=subprocess.PIPE,
                text=True,
                check=False,
            )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        merged = json.loads((module_side / 'merged.sarif').read_text(encoding='utf-8'))
        expected = json.loads((shell_root / 'merged.sarif').read_text(encoding='utf-8'))
        self.assertEqual(merged, expected)
        self.assertEqual(len(merged['runs'][0]['results']), 2)
        self.assertEqual(len(merged['runs'][0]['tool']['driver']['rules']), 2)
        self.assertEqual(len(merged['runs'][0]['artifacts']), 1)


if __name__ == '__main__':
    unittest.main()
