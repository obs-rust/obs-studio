"""Dell-reachable zstd policy belongs to setup-soldr and caller environment."""
import importlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from host_platform import is_linux

ROOT = Path(__file__).resolve().parent.parent


class DellCompressionTest(unittest.TestCase):
    def test_dell_reachable_pack_has_no_explicit_zstd_level(self):
        text = (ROOT / 'ci/tier2_pack.py').read_text()
        self.assertNotRegex(text, r"zstd\s+-(?:\d|I)|['\"]-\d+['\"]|--zstd-level|--fast=")
        self.assertIn("'--zstd'", text)
        self.assertNotIn('OBS_TIER2_PERSISTENT', text)

    def test_workflows_and_cache_wrapper_do_not_select_runner_levels(self):
        for name in ('.github/actions/cache/action.yml', '.github/workflows/build-project.yaml', '.github/workflows/ci.yml'):
            text = (ROOT / name).read_text()
            with self.subTest(path=name):
                self.assertNotRegex(text, r"runner\.environment == 'self-hosted'.*(?:compression-level|&& '(?:0|1)')")
                self.assertIsNone(re.search(r'^\s+ZSTD_(?:CLEVEL|NBTHREADS):', text, re.M), name)
                self.assertNotIn('--zstd-level', text)
        cache = (ROOT / '.github/actions/cache/action.yml').read_text()
        self.assertNotIn('ZSTD_CLEVEL:', cache)
        self.assertNotIn('ZSTD_NBTHREADS:', cache)

    def test_pack_and_dell_cache_saves_follow_setup_soldr(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        for job_name in ('rust-tests-linux', 'rust-linux-tier2-build'):
            job = re.search(rf'^  {job_name}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)', workflow, re.M | re.S)[1]
            setup = job.index('uses: zackees/setup-soldr@')
            for step in re.split(r'(?m)^      - ', job)[1:]:
                if 'uses: ./.github/actions/cache' in step and 'mode: save' in step:
                    if job.index(step) < setup:
                        # Dell routing is merge_group-only. This early apt save
                        # is a hosted master push, not a Dell compression site.
                        self.assertIn("github.ref == 'refs/heads/master'", step)
                    else:
                        self.assertLess(setup, job.index(step))
            if job_name == 'rust-linux-tier2-build':
                self.assertLess(setup, job.index('- name: Build Tier 2 (libobs'))
                self.assertNotIn('Prepare Tier 2 Upload', job)
                self.assertNotIn('OBS_TIER2_PERSISTENT', job)

    def test_library_backed_ccache_routes_zstd_environment_with_current_fallback(self):
        text = (ROOT / '.github/workflows/build-project.yaml').read_text()
        for name in ('Build Touched Linux Targets 🧱', 'Set Qt Cache Limit 🗄️'):
            step = text.split(f'      - name: {name}', 1)[1].split('\n      - ', 1)[0]
            with self.subTest(step=name):
                self.assertIn('ccache --set-config=compression_level="${ZSTD_CLEVEL:-1}"', step)
                self.assertNotIn('compression=false', step)

    @unittest.skipUnless(is_linux(), 'Qt cache workflow shell runs on Linux')
    def test_ccache_config_command_uses_caller_level_and_unset_default(self):
        fixture = importlib.import_module('test_qt_cache_budget').QtCacheBudgetTest()
        fixture.setUp()
        for level in (None, '8'):
            env = {k: v for k, v in os.environ.items() if k != 'ZSTD_CLEVEL'}
            if level:
                env['ZSTD_CLEVEL'] = level
            with self.subTest(level=level), patch.dict(os.environ, env, clear=True):
                _, calls = fixture.run_cache_steps()
            self.assertIn(f'--set-config=compression_level={level or "1"}', calls)

    def test_setup_bundle_remains_hosted_and_retains_its_existing_level(self):
        text = (ROOT / '.github/workflows/ci.yml').read_text()
        producer = text.split('  tier2-setup-producer:', 1)[1].split('  tier2-setup-consumer:', 1)[0]
        self.assertIn('runs-on: ubuntu-26.04', producer)
        self.assertIn("'zstd', '-T2', '-3'", (ROOT / 'ci/tier2_setup_probe.py').read_text())

    def test_readers_keep_accepting_compressed_and_plain_tar(self):
        text = (ROOT / '.github/workflows/build-project.yaml').read_text()
        unpack = text.split('      - name: Unpack Tier 2 Tree', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('tar -xf', unpack)
        prepared = (ROOT / 'ci/ci_gate_steps.py').read_text().split('def validate_prepared_suites()', 1)[1]
        self.assertNotIn("'--zstd'", prepared)

    @unittest.skipUnless(is_linux() and shutil.which('zstd'), 'Linux pack requires GNU tar and zstd')
    def test_actual_pack_inherits_default_and_overridden_cli_levels(self):
        pack = importlib.import_module('tier2_pack')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'tree/cargo').mkdir(parents=True)
            (root / 'tree/test').write_text('payload\n' * 1000)
            (root / 'tree/test.o').write_text('excluded object')
            (root / 'tree/cargo/excluded').write_text('excluded Rust output')
            (root / 'tree/link').symlink_to('test')
            for label, level in (('hosted-default', None), ('dell-auto', '1'), ('override', '8')):
                env = {k: v for k, v in os.environ.items() if k not in ('ZSTD_CLEVEL', 'ZSTD_NBTHREADS')}
                if level:
                    env.update(ZSTD_CLEVEL=level, ZSTD_NBTHREADS='0')
                output = root / label
                with self.subTest(level=level), patch.dict(os.environ, env, clear=True):
                    with patch.object(pack.subprocess, 'run', wraps=subprocess.run) as run:
                        pack.pack(root, output)
                    argv = run.call_args.args[0]
                    self.assertEqual(argv[:2], ['tar', '--zstd'])
                    self.assertFalse(any(re.fullmatch(r'-\d+|--zstd-level|--fast=.*', arg) for arg in argv))
                    actual = output / 'tier2-tree.tar.zst'
                    expected = output / 'expected.tar.zst'
                    # Compare bytes to an independent explicit-level compression
                    # of the same tar. This fails if the implementation shadows
                    # the caller's ZSTD_CLEVEL, even if roundtrip still succeeds.
                    reference_env = {k: v for k, v in env.items() if k != 'ZSTD_CLEVEL'}
                    reference = [arg if arg != str(actual) else str(expected) for arg in argv]
                    reference[1:2] = ['-I', f'zstd -{level or "3"}']
                    subprocess.run(reference, check=True, env=reference_env)
                    self.assertEqual(actual.read_bytes(), expected.read_bytes())
                    dest = output / 'extract'
                    dest.mkdir()
                    subprocess.run(['tar', '-xf', str(actual), '-C', str(dest)], check=True)
                    self.assertEqual((dest / 'tree/test').read_bytes(), (root / 'tree/test').read_bytes())
                    self.assertTrue((dest / 'tree/link').is_symlink())
                    self.assertFalse((dest / 'tree/test.o').exists())
                    self.assertFalse((dest / 'tree/cargo').exists())


if __name__ == '__main__':
    unittest.main()
