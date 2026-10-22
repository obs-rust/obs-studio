"""Execute the manual scope and evaluate the real native job selectors."""

import ast
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = (ROOT / '.github/workflows/build-project.yaml').read_text()


def job(name):
    match = re.search(rf'(?ms)^  {re.escape(name)}:\n.*?(?=^  [a-z][\w-]*:|\Z)', WORKFLOW)
    assert match, name
    return match.group()


def selected(expression, measurement, full, targets=''):
    expression = expression.strip().removeprefix('${{').removesuffix('}}').strip()
    for before, after in [
        ('fromJSON(needs.check-event.outputs.full)', 'full'),
        ('needs.check-event.outputs.full', 'full_text'),
        ('needs.check-event.outputs.linux_targets', 'targets'),
        ('inputs.flatpak-measurement-only', 'measurement'),
        ('always()', 'always'),
        ('cancelled()', 'cancelled'),
        ('needs.rust-tests-linux.result', 'fast_result'),
        ('needs.check-event.outputs.deferred', 'deferred'),
        ('needs.check-event.outputs.macos', 'macos'),
        ('needs.check-event.outputs.windows', 'windows'),
        ('needs.ci-mode.outputs.skip_rust-cross-lint', 'skip_cross'),
        ('needs.ci-mode.outputs.cross_hit', 'cross_hit'),
    ]:
        expression = expression.replace(before, after)
    expression = re.sub(r'!(?!=)', ' not ', expression.replace('&&', ' and ').replace('||', ' or '))
    values = dict(measurement=measurement, full=full, full_text=str(full).lower(), targets=targets,
                  always=True, cancelled=False, fast_result='success', deferred='false',
                  macos='false', windows='false', skip_cross='false', cross_hit='false')

    def evaluate(node):
        if isinstance(node, ast.Expression):
            return evaluate(node.body)
        if isinstance(node, ast.Constant):
            return node.value
        if isinstance(node, ast.Name):
            return values[node.id]
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.Not):
            return not evaluate(node.operand)
        if isinstance(node, ast.BoolOp):
            reducer = all if isinstance(node.op, ast.And) else any
            return reducer(evaluate(value) for value in node.values)
        if isinstance(node, ast.Compare) and len(node.ops) == 1:
            left, right = evaluate(node.left), evaluate(node.comparators[0])
            if isinstance(node.ops[0], ast.Eq):
                return left == right
            if isinstance(node.ops[0], ast.NotEq):
                return left != right
        raise AssertionError(f'Unsupported job selector: {ast.dump(node)}')

    return evaluate(ast.parse(expression.strip(), mode='eval'))


class FlatpakMeasurementScopeTest(unittest.TestCase):
    def scope(self, event, measurement, full_input=False):
        step = job('check-event').split('      - name: Check Change Scope 🧭\n', 1)[1]
        body = step.split('        run: |\n', 1)[1]
        lines = []
        for line in body.splitlines():
            if line and not line.startswith('          '):
                break
            lines.append(line[10:])
        script = re.sub(r'\$\{\{(.*?)\}\}',
                        lambda match: str(full_input).lower() if match[1].strip() == 'inputs.full' else 'false',
                        '\n'.join(lines))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'output'
            env = dict(os.environ, GITHUB_EVENT_NAME=event, GITHUB_REF_TYPE='branch',
                       GITHUB_REF_NAME='master', GITHUB_BASE_REF='master', GITHUB_OUTPUT=str(output),
                       FLATPAK_MEASUREMENT_ONLY=str(measurement).lower())
            result = subprocess.run(['bash', '-euo', 'pipefail', '-c', script], cwd=ROOT,
                                    env=env, text=True, capture_output=True, timeout=15)
            values = dict(line.split('=', 1) for line in output.read_text().splitlines()) if output.exists() else {}
            return result, values

    def test_default_manual_and_release_full_scope_keep_every_lane(self):
        for event in ['workflow_dispatch', 'schedule', 'push', 'pull_request', 'merge_group']:
            with self.subTest(event=event):
                result, values = self.scope(event, False, full_input=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                for key in ['full', 'core', 'macos', 'windows', 'qt', 'tier2']:
                    self.assertEqual(values[key], 'true', (event, key, values))

    def test_measurement_keeps_flatpak_and_removes_unrelated_lanes(self):
        result, values = self.scope('workflow_dispatch', True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(values['full'], 'true')
        for key in ['core', 'macos', 'windows', 'qt', 'tier2']:
            self.assertEqual(values[key], 'false', (key, values))
        self.assertEqual(values['platform_os'], '[]')
        self.assertEqual(values['linux_targets'], '')
        self.assertNotIn('flatpak-measurement-only', job('flatpak-build'))

    def test_reusable_workflow_rejects_measurement_outside_manual_dispatch(self):
        for event in ['pull_request', 'merge_group', 'push', 'schedule']:
            with self.subTest(event=event):
                result, _ = self.scope(event, True, full_input=True)
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_measurement_cannot_override_an_explicit_release_gate(self):
        result, _ = self.scope('workflow_dispatch', True, full_input=True)
        self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_native_app_and_release_cross_jobs_respect_manual_selector(self):
        for name in ['macos-build', 'ubuntu-build', 'windows-build', 'rust-cross-lint-release']:
            expression = re.search(r'^    if: (.*)$', job(name), re.MULTILINE).group(1)
            with self.subTest(job=name):
                self.assertTrue(selected(expression, False, True))
                self.assertFalse(selected(expression, True, True))
                self.assertFalse(selected(expression, False, False))
        expression = re.search(r'^    if: (.*)$', job('ubuntu-build'), re.MULTILINE).group(1)
        self.assertFalse(selected(expression, False, False, targets='obs-browser'))
        self.assertFalse(selected(expression, True, False, targets='obs-browser'))

    def test_default_off_inputs_and_caller_are_manual_only(self):
        caller = (ROOT / '.github/workflows/ci.yml').read_text()
        for workflow in [caller, WORKFLOW]:
            self.assertIsNotNone(re.search(r'flatpak-measurement-only:\n\s+description: [^\n]+\n\s+type: boolean\n\s+default: false', workflow),
                                 'measurement input must be boolean and default off')
        self.assertIn("flatpak-measurement-only: ${{ github.event_name == 'workflow_dispatch' && inputs.flatpak-measurement-only }}", caller)

    def test_regular_cross_lint_keeps_default_and_skips_measurement(self):
        expression = re.search(r'^    if: (.*)$', job('rust-cross-lint'), re.MULTILINE).group(1)
        for full in [False, True]:
            with self.subTest(full=full):
                self.assertTrue(selected(expression, False, full))
                self.assertFalse(selected(expression, True, full))


if __name__ == '__main__':
    unittest.main()
