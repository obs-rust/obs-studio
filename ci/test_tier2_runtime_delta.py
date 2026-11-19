"""A prepared Tier 2 runtime survives small Ubuntu updates (#1592, #2072).

Usage:
    uv run python -m unittest ci/test_tier2_runtime_delta.py

The prepared image matched the closure exactly or was not used. One zlib
security update (1ubuntu3.1 -> 1ubuntu3.2, one of 224 packages) sent every
Tier 2 test job to the stock path, 75 s of apt instead of 20-30 s
(2026-10-24). A closure that differs in a few packages now loads the prepared
image and installs exactly those versions on top; the installed inventory is
still verified against the whole closure. A larger difference still falls back
to stock.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / 'rust/tools/linux-validate'


def load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


selection = load('runtime_selection', 'runtime-selection.py')
runtime = load('ci_runtime', 'ci-runtime.py')

OLD = '1:1.3.dfsg+really1.3.1-1ubuntu3.1'
NEW = '1:1.3.dfsg+really1.3.1-1ubuntu3.2'


class DeltaTest(unittest.TestCase):
    def test_delta_lists_only_missing_or_changed_packages(self):
        requirements = f'a=1\nzlib1g={NEW}\nc=3\n'
        inventory = f'a:amd64\t1\nzlib1g:amd64\t{OLD}\nb:amd64\t2\n'
        self.assertEqual(selection.delta(requirements, inventory), [f'c:amd64=3', f'zlib1g:amd64={NEW}'])

    def test_matching_closure_has_no_delta(self):
        self.assertEqual(selection.delta('a=1\n', 'a:amd64\t1\n'), [])


class AptInstallTest(unittest.TestCase):
    packages = ['libc6:amd64=2.43-2ubuntu2.7', f'zlib1g:amd64={NEW}']
    prefix = ('docker', 'exec', 'container', 'timeout', '--kill-after=10s', '180s',
              'env', 'LC_ALL=C', 'apt-get', '-o', 'Acquire::Retries=3',
              '-o', 'Acquire::http::Timeout=15', '-o', 'Acquire::https::Timeout=15')

    def install(self, failures=()):
        calls = []
        errors = iter(failures)

        def command(*argv, **kwargs):
            calls.append(argv)
            if 'install' in argv or 'update' in argv:
                error = next(errors, None)
                if error:
                    raise subprocess.CalledProcessError(error[0], argv, output=error[1])
            return ''

        with patch.object(runtime, 'command', side_effect=command):
            try:
                runtime.install_packages('container', 'prepared', '', self.packages)
            finally:
                self.calls = [c for c in calls if 'apt-get' in c]

    def test_delta_commands_use_native_acquire_bounds_and_timeout(self):
        self.install()
        self.assertEqual(self.calls, [(*self.prefix, 'update', '-qq'),
            (*self.prefix, 'install', '-y', '--no-install-recommends', *self.packages)])

    def test_unavailable_pin_refreshes_once_and_preserves_exact_versions(self):
        self.install([None, (100, "E: Version '2.43-2ubuntu2.7' for 'libc6:amd64' was not found")])
        self.assertEqual(self.calls, [(*self.prefix, 'update', '-qq'),
            (*self.prefix, 'install', '-y', '--no-install-recommends', *self.packages)] * 2)

    def test_unavailable_pin_after_refresh_fails_with_delta_packages(self):
        missing = (100, "E: Version '2.43-2ubuntu2.7' for 'libc6:amd64' was not found")
        with self.assertRaises(RuntimeError) as error:
            self.install([None, missing, None, missing])
        self.assertEqual(len(self.calls), 4)
        for package in self.packages:
            self.assertIn(package, str(error.exception))
        self.assertIn(missing[1], str(error.exception))

    def test_other_failures_do_not_refresh(self):
        for failures, count in (([(100, 'archive unavailable')], 1),
                                ([None, (100, 'dependency conflict')], 2),
                                ([None, (124, 'timed out')], 2),
                                ([None, (137, 'killed')], 2)):
            with self.subTest(failures=failures), self.assertRaises(RuntimeError) as error:
                self.install(failures)
            self.assertEqual(len(self.calls), count)
            self.assertIn(self.packages[0], str(error.exception))

    def test_refresh_failure_stops_before_second_install(self):
        with self.assertRaises(RuntimeError):
            self.install([None, (100, "E: Version '2' for 'libc6' was not found"),
                          (124, 'refresh timed out')])
        self.assertEqual(len(self.calls), 3)

    def test_empty_delta_runs_no_commands(self):
        with patch.object(runtime, 'command') as command:
            runtime.install_packages('container', 'prepared', 'a=1\n', [])
        command.assert_not_called()

    def test_apt_diagnostics_capture_stderr(self):
        with patch.object(runtime.subprocess, 'check_output', return_value='apt error\n') as output:
            self.assertEqual(runtime.command('apt-get', 'update', capture_stderr=True), 'apt error')
        output.assert_called_once_with(('apt-get', 'update'), text=True, stderr=subprocess.STDOUT)


class ExecuteTest(unittest.TestCase):
    def descriptor(self, inventory: str) -> dict:
        return {'base': runtime.BASE, 'platform': 'linux/amd64', 'inventory': inventory,
                'image_id': 'sha256:' + 'a' * 64, 'compressed_bytes': 4,
                'archive_sha256': hashlib.sha256(b'good').hexdigest(),
                'url': 'https://github.com/obs-rust/obs-studio-libs/releases/download/runtime-1/image.tar.zst'}

    def run_execute(self, requirements: str, image_inventory: str, installed: str):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'descriptor.json').write_text(json.dumps(self.descriptor(image_inventory)))
            (root / 'packages.txt').write_text(requirements)
            args = argparse.Namespace(packages=root / 'packages.txt', descriptor=root / 'descriptor.json',
                                      state=root / 'state', cost=root / 'cost', source=root, build=root,
                                      uv=root / 'uv', archives=root, mode='ON', part='suite')
            calls = []

            def command(*argv, **_kwargs):
                calls.append(argv)
                if argv[:2] == ('docker', 'create'):
                    return 'b' * 64
                if 'dpkg-query' in argv:
                    return installed
                return ''

            with patch.object(runtime, 'download_prepared', return_value='sha256:' + 'a' * 64), \
                    patch.object(runtime, 'command', side_effect=command), \
                    patch.object(runtime.subprocess, 'run'):
                runtime.execute(args)
            report = json.loads((root / 'state/runtime-measurement.json').read_text())
            return calls, report

    def test_small_drift_installs_the_delta_on_the_prepared_image(self):
        calls, report = self.run_execute(f'a=1\nzlib1g={NEW}\n', f'a\t1\nzlib1g\t{OLD}\n',
                                         f'a\t1\nzlib1g\t{NEW}\n')
        self.assertEqual(report['variant'], 'prepared')
        self.assertEqual(report['delta'], [f'zlib1g:amd64={NEW}'])
        installs = [c for c in calls if 'install' in c]
        self.assertEqual(len(installs), 1)
        self.assertIn(f'zlib1g:amd64={NEW}', installs[0])
        self.assertNotIn(('docker', 'pull', runtime.BASE), calls)

    def test_delta_that_does_not_install_fails(self):
        with self.assertRaises(ValueError):
            self.run_execute(f'a=1\nzlib1g={NEW}\n', f'a\t1\nzlib1g\t{OLD}\n', f'a\t1\nzlib1g\t{OLD}\n')

    def test_large_drift_falls_back_to_stock(self):
        count = runtime.MAX_DELTA + 1
        requirements = ''.join(f'p{i}=2\n' for i in range(count))
        image = ''.join(f'p{i}\t1\n' for i in range(count))
        installed = ''.join(f'p{i}\t2\n' for i in range(count))
        calls, report = self.run_execute(requirements, image, installed)
        self.assertEqual(report['variant'], 'stock')
        self.assertIn(('docker', 'pull', runtime.BASE), calls)


if __name__ == '__main__':
    unittest.main()
