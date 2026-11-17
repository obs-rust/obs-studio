"""Runtime delivery must preserve integrity and complete-suite execution paths."""
import hashlib
import importlib.util
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('ci_runtime', ROOT / 'rust/tools/linux-validate/ci-runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class RuntimeTest(unittest.TestCase):
    def descriptor(self):
        return {'base': runtime.BASE, 'platform': 'linux/amd64', 'inventory': 'a\t1\n',
                'image_id': 'sha256:' + 'a' * 64, 'compressed_bytes': 4,
                'archive_sha256': hashlib.sha256(b'good').hexdigest(),
                'url': 'https://github.com/obs-rust/obs-studio-libs/releases/download/runtime-1/image.tar.zst'}

    def test_changed_closure_falls_back_without_download_or_docker(self):
        with patch.object(runtime.urllib.request, 'urlopen') as download, patch.object(runtime, 'command') as command:
            # A few changed packages install on top (ci/test_tier2_runtime_delta.py).
            changed = ''.join(f'p{i}=1\n' for i in range(runtime.MAX_DELTA + 1))
            self.assertIsNone(runtime.prepared_image(self.descriptor(), changed, Path('/unused')))
            download.assert_not_called()
            command.assert_not_called()

    def test_unavailable_asset_falls_back_without_docker(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(runtime.urllib.request, 'urlopen',
                side_effect=urllib.error.URLError('offline')), patch.object(runtime, 'command') as command:
            self.assertIsNone(runtime.prepared_image(self.descriptor(), 'a=1\n', Path(tmp)))
            command.assert_not_called()

    def test_corrupt_archive_fails_before_docker_load(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / 'runtime.tar.zst'
            archive.write_bytes(b'evil')
            with self.assertRaisesRegex(ValueError, 'checksum'):
                runtime.verify_archive(archive, self.descriptor())

    def test_mounts_preserve_original_paths_and_writable_timing_history(self):
        paths = [Path('/host') / name for name in ('source', 'build', 'cost', 'uv', 'apt')]
        mounts = runtime.mounts(*paths)
        self.assertIn('type=bind,src=/host/source,dst=/__w/obs-studio/obs-studio,readonly', mounts)
        self.assertIn('type=bind,src=/host/build,dst=/__w/_temp/tier2', mounts)
        self.assertIn('type=bind,src=/host/cost,dst=/__w/_temp/tier2-cost', mounts)

    def test_complete_suite_failure_propagates_and_container_stops(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            descriptor = self.descriptor()
            (root / 'descriptor.json').write_text(json.dumps(descriptor))
            (root / 'packages.txt').write_text('a=1\n')
            args = argparse.Namespace(packages=root / 'packages.txt', descriptor=root / 'descriptor.json',
                                      state=root / 'state', cost=root / 'cost', source=root,
                                      build=root, uv=root / 'uv', archives=root, mode='ON', part='suite')
            ident = 'b' * 64

            def command(*argv):
                if argv[:2] == ('docker', 'create'):
                    return ident
                if 'dpkg-query' in argv:
                    return 'a\t1'
                return ''

            with patch.object(runtime, 'prepared_image', return_value=descriptor['image_id']), \
                    patch.object(runtime, 'command', side_effect=command) as commands, \
                    patch.object(runtime.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'test')) as run:
                with self.assertRaises(subprocess.CalledProcessError):
                    runtime.execute(args)
                invocation = run.call_args.args[0]
                self.assertIn('OBS_TIER2_MODES=ON', invocation)
                self.assertIn('OBS_TIER2_STAGE=test', invocation)
                self.assertIn('OBS_CTEST_JOBS=8', invocation)
                self.assertIn('OBS_CTEST_PART=suite', invocation)
                self.assertIn('OBS_TIER2_COST_DIR=' + runtime.COST, invocation)
                self.assertEqual(commands.call_args.args, ('docker', 'stop', '--timeout', '1', ident))
                report = json.loads((root / 'state/runtime-measurement.json').read_text())
                self.assertEqual(report['variant'], 'prepared')


class PrebakedRuntimeTest(unittest.TestCase):
    def test_prebaked_runtime_keeps_exact_install_and_inventory_check_without_update(self):
        for matching in (True, False):
            with self.subTest(matching=matching), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                (root / 'packages').write_text('a=1\n')
                args = argparse.Namespace(packages=root / 'packages', descriptor=root / 'unused',
                    state=root / 'state', cost=root / 'cost', source=root, build=root,
                    uv=root / 'uv', archives=root, mode='OFF', part='all',
                    prebaked_image='ghcr.io/obs-rust/tier2-linux@sha256:' + 'c' * 64)

                def command(*argv):
                    if argv[:2] == ('docker', 'create'):
                        return 'd' * 64
                    if 'dpkg-query' in argv:
                        return 'a\t' + ('1' if matching else '2')
                    return ''

                with patch.object(runtime, 'prepared_image') as prepared, \
                        patch.object(runtime, 'command', side_effect=command) as commands, \
                        patch.object(runtime.subprocess, 'run') as run:
                    if matching:
                        runtime.execute(args)
                        self.assertEqual(json.loads((args.state / 'runtime-measurement.json').read_text())['variant'], 'prebaked')
                        run.assert_called_once()
                    else:
                        with self.assertRaisesRegex(ValueError, 'package closure'):
                            runtime.execute(args)
                        run.assert_not_called()
                    prepared.assert_not_called()
                    calls = [call.args for call in commands.call_args_list]
                    self.assertFalse(any('update' in call for call in calls))
                    self.assertTrue(any('install' in call and 'a=1' in call for call in calls))
                    self.assertEqual(calls[-1], ('docker', 'stop', '--timeout', '1', 'd' * 64))


if __name__ == '__main__':
    unittest.main()
