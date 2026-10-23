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


def job(name, workflow=WORKFLOW):
    match = re.search(rf'(?ms)^  {re.escape(name)}:\n.*?(?=^  [a-z][\w-]*:|\Z)', workflow)
    assert match, name
    return match.group()


def selected(expression, measurement, full, targets='', native=False, input_values=None,
             event='workflow_dispatch', build_result='success'):
    expression = expression.strip().removeprefix('${{').removesuffix('}}').strip()
    for before, after in [
        ('fromJSON(needs.check-event.outputs.full)', 'full'),
        ('needs.check-event.outputs.full', 'full_text'),
        ('needs.check-event.outputs.linux_targets', 'targets'),
        ('inputs.flatpak-measurement-only', 'measurement'),
        ('inputs.native-cache-measurement-only', 'native'),
        ('always()', 'always'),
        ('cancelled()', 'cancelled'),
        ('needs.rust-tests-linux.result', 'fast_result'),
        ('needs.check-event.outputs.deferred', 'deferred'),
        ('needs.check-event.outputs.macos', 'macos'),
        ('needs.check-event.outputs.windows', 'windows'),
        ('needs.ci-mode.outputs.skip_rust-cross-lint', 'skip_cross'),
        ('needs.ci-mode.outputs.cross_hit', 'cross_hit'),
        ('github.event_name', 'event_name'),
        ('github.ref', 'ref'),
        ('needs.build-project.result', 'build_result'),
    ]:
        expression = expression.replace(before, after)
    expression = re.sub(r'!(?!=)', ' not ', expression.replace('&&', ' and ').replace('||', ' or '))
    values = dict(measurement=measurement, native=native, full=full, full_text=str(full).lower(), targets=targets,
                  always=True, cancelled=False, fast_result='success', deferred='false',
                  macos='false', windows='false', skip_cross='false', cross_hit='false',
                  event_name=event, ref='refs/heads/master', build_result=build_result)
    inputs = input_values or {}
    values['measurement'] = inputs.get('flatpak-measurement-only', measurement)

    def replace_input(match):
        name = 'input_' + match[1].replace('-', '_')
        values[name] = inputs.get(match[1], '' if match[1].endswith('reuse-run') else False)
        return name

    expression = re.sub(r'inputs\.([\w-]+)', replace_input, expression)

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
    def scope(self, event, measurement, full_input=False, native=False,
              transport=False, source='', conflict=False, budget=False,
              repository='obs-rust/obs-studio', ref='refs/heads/master'):
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
                       GITHUB_REPOSITORY=repository, GITHUB_REF=ref,
                       FLATPAK_MEASUREMENT_ONLY=str(measurement).lower(),
                       NATIVE_MEASUREMENT_ONLY=str(native).lower(),
                       NATIVE_MEASUREMENT_CONFLICT=str(conflict).lower(),
                       NATIVE_TRANSPORT_PROBE=str(transport).lower(),
                       NATIVE_REUSE_SOURCE=source, NATIVE_BUDGET_PROBE=str(budget).lower())
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


class NativeMeasurementScopeTest(unittest.TestCase):
    scope = FlatpakMeasurementScopeTest.scope

    def test_native_modes_keep_native_apps_without_unrelated_scope(self):
        for protocol in [dict(transport=True), dict(source='a' * 40)]:
            with self.subTest(protocol=protocol):
                result, values = self.scope('workflow_dispatch', False, native=True, **protocol)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(values['full'], 'true')
                for key in ['core', 'macos', 'windows', 'qt', 'tier2']:
                    self.assertEqual(values[key], 'false', (key, values))
                self.assertEqual(values['platform_os'], '[]')
                self.assertEqual(values['linux_targets'], '')

    def test_native_modes_reject_untrusted_or_ambiguous_requests(self):
        cases = [
            dict(event=event) for event in ['pull_request', 'merge_group', 'push', 'schedule']
        ] + [
            dict(repository='someone/obs-studio'), dict(ref='refs/heads/feature'),
            dict(full_input=True), dict(measurement=True), dict(conflict=True),
            dict(budget=True), dict(transport=False), dict(source='a' * 40),
            dict(transport=False, source='not-a-sha'),
        ]
        for overrides in cases:
            parameters = dict(event='workflow_dispatch', measurement=False, native=True, transport=True)
            parameters.update(overrides)
            with self.subTest(parameters=parameters):
                result, _ = self.scope(**parameters)
                self.assertNotEqual(result.returncode, 0, result.stdout)

    def test_real_native_and_unrelated_app_selectors(self):
        for name in ['macos-build', 'windows-build', 'ubuntu-build', 'flatpak-build',
                     'rust-cross-lint', 'rust-cross-lint-release']:
            expression = re.search(r'^    if: (.*)$', job(name), re.MULTILINE).group(1)
            with self.subTest(job=name):
                self.assertTrue(selected(expression, False, True))
                self.assertEqual(selected(expression, False, True, native=True),
                                 name in ['macos-build', 'windows-build'])

    def test_native_input_is_default_off_and_forwarded_only_on_manual(self):
        caller = (ROOT / '.github/workflows/ci.yml').read_text()
        for workflow in [caller, WORKFLOW]:
            self.assertIsNotNone(re.search(
                r'native-cache-measurement-only:\n\s+description: [^\n]+\n\s+type: boolean\n\s+default: false',
                workflow))
        self.assertIn("native-cache-measurement-only: ${{ github.event_name == 'workflow_dispatch' && inputs.native-cache-measurement-only }}", caller)
        self.assertIn('native-cache-measurement-conflict:', job('build-project', caller))
        for name in ['flatpak-runtime-reuse', 'tier2-setup-producer', 'tier2-setup-consumer']:
            expression = re.search(r'^    if: (.*)$', job(name, caller), re.MULTILINE).group(1)
            self.assertIn('!inputs.native-cache-measurement-only', expression, name)
        build = job('build-project', caller)
        for conflict in ['inputs.flatpak-runtime-reuse-run', 'inputs.tier2-setup-probe',
                         'inputs.flatpak-runtime-test', 'inputs.flatpak-cache-probe']:
            self.assertIn(conflict, build)
        self.assertNotIn('native-cache-measurement-only', (ROOT / '.github/workflows/release-rust.yaml').read_text())

    def test_each_conflict_is_computed_from_the_actual_workflow_expression(self):
        caller = (ROOT / '.github/workflows/ci.yml').read_text()
        expressions = [
            (re.search(r'^      native-cache-measurement-conflict: (.*)$',
                       job('build-project', caller), re.MULTILINE).group(1),
             ['flatpak-runtime-reuse-run', 'tier2-setup-probe']),
            (re.search(r'^          NATIVE_MEASUREMENT_CONFLICT: (.*)$',
                       job('check-event'), re.MULTILINE).group(1),
             ['native-cache-measurement-conflict', 'flatpak-measurement-only',
              'flatpak-compiler-diagnostics', 'flatpak-runtime-test', 'flatpak-cold-build',
              'flatpak-dependency-transfer-probe', 'flatpak-compiler-transfer-probe',
              'flatpak-cache-probe', 'flatpak-pruned-build-probe']),
        ]
        for expression, flags in expressions:
            self.assertFalse(selected(expression, False, True))
            for flag in flags:
                with self.subTest(flag=flag):
                    value = '123' if flag.endswith('reuse-run') else True
                    self.assertTrue(selected(expression, False, True, input_values={flag: value}))
        self.assertFalse(selected(expressions[0][0], False, True, event='pull_request',
                                  input_values={'tier2-setup-probe': True}))

    def test_conflicting_caller_modes_cannot_skip_validation_or_start_setup(self):
        caller = (ROOT / '.github/workflows/ci.yml').read_text()
        inputs = {'flatpak-runtime-reuse-run': '123', 'tier2-setup-probe': True}
        for name in ['build-project', 'flatpak-runtime-reuse', 'tier2-setup-producer',
                     'tier2-setup-consumer']:
            expression = re.search(r'^    if: (.*)$', job(name, caller), re.MULTILINE).group(1)
            self.assertEqual(selected(expression, False, True, native=True, input_values=inputs),
                             name == 'build-project', name)
        expression = re.search(r'^    if: (.*)$', job('native-cache-consumer', caller),
                               re.MULTILINE).group(1)
        inputs = {'native-cache-transport-probe': True}
        self.assertTrue(selected(expression, False, True, native=True, input_values=inputs))
        self.assertFalse(selected(expression, False, True, native=True, input_values=inputs,
                                  build_result='failure'))
        self.assertTrue(selected(expression, False, True, native=False, input_values=inputs,
                                 build_result='failure'))


if __name__ == '__main__':
    unittest.main()
