"""Hosted Tier 2 deduplicates identical fixture compiles without persisting new state."""
from pathlib import Path
import importlib.util
import os
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'ci/tier2_front_cache.py'


def load():
    spec = importlib.util.spec_from_file_location('tier2_front_cache', SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FrontCacheTest(unittest.TestCase):
    def test_only_the_zccache_launcher_gets_a_front_cache(self):
        mod = load()
        for launcher in ('', 'custom-launcher'):
            with self.subTest(launcher=launcher), patch.dict(os.environ, OBS_TIER2_LAUNCHER=launcher):
                with patch.object(mod, 'install') as install, patch.object(mod.subprocess, 'call', return_value=7) as call:
                    self.assertEqual(mod.main(['bash', 'validate.sh']), 7)
                    install.assert_not_called()
                    call.assert_called_once_with(['bash', 'validate.sh'])

    def test_hosted_build_gets_an_ephemeral_front_cache_and_keeps_its_exit_status(self):
        mod = load()
        with patch.dict(os.environ, OBS_TIER2_LAUNCHER='zccache', ZCCACHE_CACHE_DIR='/backing'):
            with patch.object(mod, 'install', return_value=Path('/tools/ccache')):
                with patch.object(mod.subprocess, 'call', return_value=9) as call:
                    self.assertEqual(mod.main(['bash', 'validate.sh']), 9)
                    args, kwargs = call.call_args_list[0]
                    self.assertEqual(args[0], ['bash', 'validate.sh'])
                    env = kwargs['env']
                    self.assertEqual(env['CCACHE_PREFIX'], 'zccache')
                    self.assertEqual(env['OBS_TIER2_LAUNCHER'], '/tools/ccache')
                    self.assertEqual(env['ZCCACHE_CACHE_DIR'], '/backing')
                    self.assertEqual(env['CCACHE_MAXSIZE'], '512M')
                    self.assertNotIn('CCACHE_SLOPPINESS', env)
                    self.assertNotIn('CCACHE_IGNOREHEADERS', env)
                    self.assertIn(Path(env['CCACHE_DIR']).name, ('objects',))
                    self.assertEqual(call.call_args_list[-2].args[0], ['/tools/ccache', '--show-stats'])
                    self.assertEqual(call.call_args_list[-1].args[0], ['zccache', 'stop'])

    def test_bad_download_is_rejected_before_extraction(self):
        mod = load()
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            def download(_url, path):
                Path(path).write_bytes(b'bad archive')
            with patch.object(mod.urllib.request, 'urlretrieve', side_effect=download):
                with patch.object(mod.tarfile, 'open') as extract:
                    with self.assertRaisesRegex(RuntimeError, 'SHA-256'):
                        mod.install(work)
                    extract.assert_not_called()

    def test_installs_only_the_verified_binary(self):
        mod = load()
        import hashlib
        with tempfile.TemporaryDirectory() as directory:
            work = Path(directory)
            binary = work / 'ccache'
            binary.write_bytes(b'version-pinned fixture')
            archive = work / 'fixture.tar.xz'
            with tarfile.open(archive, 'w:xz') as tar:
                tar.add(binary, arcname=mod.NAME + '/ccache')
            data = archive.read_bytes()
            def download(_url, path):
                Path(path).write_bytes(data)
            with patch.object(mod, 'SHA256', hashlib.sha256(data).hexdigest()):
                with patch.object(mod.urllib.request, 'urlretrieve', side_effect=download):
                    target = mod.install(work / 'install')
            self.assertEqual(target.read_bytes(), binary.read_bytes())
            self.assertTrue(os.access(target, os.X_OK))

    def test_workflow_wraps_only_the_build(self):
        job = (ROOT / '.github/workflows/build-project.yaml').read_text().split('  rust-linux-tier2-build:', 1)[1].split('\n  rust-linux-tier2:', 1)[0]
        self.assertIn('ci/tier2_front_cache.py -- bash rust/tools/linux-validate/validate.sh', job)
        self.assertEqual(job.count('ci/tier2_front_cache.py'), 1)
        self.assertIn('compression-level: 0', job)


if __name__ == '__main__':
    unittest.main()
