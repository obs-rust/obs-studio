"""Fail-closed reuse of the Dell's configured Ninja tree (Refs #4954)."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SPEC = importlib.util.spec_from_file_location('tier2_configure', ROOT / 'ci/tier2_configure.py')


class ConfigureTest(unittest.TestCase):
    def setUp(self):
        self.mod = importlib.util.module_from_spec(SPEC)
        SPEC.loader.exec_module(self.mod)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.src = self.root / 'src'
        self.src.mkdir()
        (self.src / 'CMakeLists.txt').write_text('cmake_minimum_required(VERSION 3.24)\nproject(reuse NONE)\nadd_custom_target(probe ALL COMMAND ${CMAKE_COMMAND} -E echo first)\n')
        self.build = self.root / 'build'
        self.args = ['cmake', '-S', str(self.src), '-B', str(self.build), '-G', 'Ninja', '-DENABLE_UNIT_TESTS=ON', '-DCMAKE_BUILD_TYPE=RelWithDebInfo']
        self.env = patch.dict(os.environ, {'OBS_TIER2_KEEP_CONFIGURE': '1', 'OBS_TIER2_LAUNCHER': ''})
        self.env.start()
        self.addCleanup(self.env.stop)

    def run_configure(self, args=None):
        return self.mod.configure(self.build, args or self.args)

    def test_same_args_skip_and_every_argument_change_configures(self):
        self.assertTrue(self.run_configure())
        self.assertFalse(self.run_configure())
        for flag in ('-DCMAKE_BUILD_TYPE=Debug', '-DENABLE_UNIT_TESTS=OFF', '-DCMAKE_C_COMPILER_LAUNCHER=ccache', '-DOBS_BUILD_C_REFERENCE=OFF'):
            with self.subTest(flag=flag):
                self.assertTrue(self.run_configure(self.args + [flag]))
                self.assertFalse(self.run_configure(self.args + [flag]))
        self.assertNotEqual(self.mod.fingerprint(['a b', 'c']), self.mod.fingerprint(['a', 'b c']))

    def test_missing_or_modified_tree_and_stamp_configure(self):
        self.assertTrue(self.run_configure())
        for name in (self.mod.STAMP, 'CMakeCache.txt', 'build.ninja'):
            with self.subTest(name=name):
                path = self.build / name
                path.rename(path.with_name(name + '.saved'))
                self.assertTrue(self.run_configure())
                path.write_text(path.read_text() + '\n# changed\n')
                self.assertTrue(self.run_configure())

    def test_environment_changes_configure(self):
        self.run_configure()
        for key in ('CC', 'CXX', 'CFLAGS', 'LDFLAGS', 'OBS_TIER2_LAUNCHER', 'OBS_RUST_VERSION', 'CMAKE_PREFIX_PATH', 'PKG_CONFIG_PATH', 'RUSTUP_TOOLCHAIN'):
            with self.subTest(key=key), patch.dict(os.environ, {key: 'changed'}):
                self.assertTrue(self.run_configure())
                self.assertFalse(self.run_configure())

    def test_hosted_always_configures(self):
        with patch.dict(os.environ, {'OBS_TIER2_KEEP_CONFIGURE': ''}):
            self.assertTrue(self.run_configure())
            self.assertTrue(self.run_configure())

    def test_failed_configure_invalidates_previous_stamp(self):
        self.run_configure()
        with self.assertRaises(subprocess.CalledProcessError):
            self.run_configure(self.args + ['-G', 'invalid-generator'])
        self.assertFalse((self.build / self.mod.STAMP).exists())
        self.assertTrue(self.run_configure())

    def test_ninja_still_regenerates_when_cmakelists_changes(self):
        self.run_configure()
        path = self.src / 'CMakeLists.txt'
        path.write_text(path.read_text().replace('echo first', 'echo second'))
        self.assertFalse(self.run_configure())
        result = subprocess.run(['cmake', '--build', str(self.build)], capture_output=True, text=True, check=True)
        self.assertIn('second', result.stdout)

    def test_git_version_changes_configure(self):
        subprocess.run(['git', 'init', '-q', str(self.src)], check=True)
        def commit(message):
            subprocess.run(['git', '-C', str(self.src), '-c', 'user.name=Test', '-c', 'user.email=test@example.com', '-c', 'commit.gpgsign=false', 'commit', '--allow-empty', '-qm', message], check=True)
        commit('first')
        subprocess.run(['git', '-C', str(self.src), 'tag', '33.0.0'], check=True)
        self.run_configure()
        self.assertFalse(self.run_configure())
        commit('second')
        self.assertTrue(self.run_configure())


if __name__ == '__main__':
    unittest.main()
