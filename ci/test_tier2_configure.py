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
        inventory = patch.object(self.mod, 'installed_packages', return_value='fixture=1')
        inventory.start()
        self.addCleanup(inventory.stop)
        self.env = patch.dict(os.environ, {'OBS_TIER2_KEEP_CONFIGURE': '1', 'OBS_TIER2_LAUNCHER': '', 'OBS_LIBOBS_MODULE_CACHE_DIR': ''})
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
        self.assertNotEqual(self.mod.fingerprint(['cmake', 'a b', 'c']), self.mod.fingerprint(['cmake', 'a', 'b c']))

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
        for key in ('CC', 'CXX', 'CFLAGS', 'LDFLAGS', 'OBS_TIER2_LAUNCHER', 'OBS_RUST_VERSION', 'CMAKE_PREFIX_PATH', 'PKG_CONFIG_PATH', 'RUSTUP_TOOLCHAIN', 'UV_PYTHON', 'PYTHONPATH', 'VIRTUAL_ENV', 'CPATH'):
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
        subprocess.run(['git', '-C', str(self.src), '-c', 'tag.gpgsign=false', 'tag', '33.0.0'], check=True)
        self.run_configure()
        self.assertFalse(self.run_configure())
        commit('second')
        self.assertTrue(self.run_configure())

    def test_unknown_inventory_and_module_cache_never_skip(self):
        self.run_configure()
        with patch.object(self.mod, 'installed_packages', side_effect=OSError('unavailable')):
            self.assertTrue(self.run_configure())
            self.assertFalse((self.build / self.mod.STAMP).exists())
        with patch.dict(os.environ, {'OBS_LIBOBS_MODULE_CACHE_DIR': '/cache'}):
            self.assertTrue(self.run_configure())
            self.assertTrue(self.run_configure())

    def test_package_and_toolchain_file_changes_configure(self):
        toolchain = self.root / 'toolchain.cmake'
        toolchain.write_text('# first\n')
        args = self.args + ['-DCMAKE_TOOLCHAIN_FILE:FILEPATH=' + str(toolchain)]
        self.run_configure(args)
        self.assertFalse(self.run_configure(args))
        toolchain.write_text('# second\n')
        self.assertTrue(self.run_configure(args))
        with patch.object(self.mod, 'installed_packages', return_value='fixture=2'):
            self.assertTrue(self.run_configure(args))

    def test_multiple_preload_scripts_and_includes_never_skip(self):
        first, second, included = [self.root / name for name in ('first.cmake', 'second.cmake', 'included.cmake')]
        included.write_text('set(PROBE first CACHE STRING "" FORCE)\n')
        first.write_text('include("' + str(included) + '")\n')
        second.write_text('# second\n')
        args = self.args + ['-C', str(first), '-C' + str(second)]
        self.assertTrue(self.run_configure(args))
        self.assertTrue(self.run_configure(args))
        included.write_text('set(PROBE second CACHE STRING "" FORCE)\n')
        self.assertTrue(self.run_configure(args))
        self.assertIn('PROBE:STRING=second', (self.build / 'CMakeCache.txt').read_text())

    def test_suppressed_ninja_regeneration_never_skips(self):
        args = self.args + ['-DCMAKE_SUPPRESS_REGENERATION=ON']
        self.assertTrue(self.run_configure(args))
        self.assertTrue(self.run_configure(args))

    def test_validate_shell_uses_exact_arguments_and_skip_decision(self):
        # Run the real shell block and helper against a small CMake project.
        (self.src / 'ci').symlink_to(ROOT / 'ci', target_is_directory=True)
        binary = self.root / 'bin'
        binary.mkdir()
        dpkg = binary / 'dpkg-query'
        dpkg.write_text('#!/bin/sh\nprintf "fixture=1\\n"\n')
        dpkg.chmod(0o755)
        source = (ROOT / 'rust/tools/linux-validate/validate.sh').read_text()
        setup = source.split('# Compiler cache:', 1)[1].split('# Read the same canonical', 1)[0]
        configure = source.split('  # Kept-tree configure:', 1)[1].split('  # -k 0:', 1)[0]
        prefix = 'build_number=(); runpath_flags=(); rustc_version=fixture\n'
        script = prefix + '# Compiler cache:' + setup + '# Kept-tree configure:' + configure
        env = dict(os.environ, SRC=str(self.src), BUILD=str(self.root), build=str(self.build),
                   PATH=str(binary) + os.pathsep + os.environ['PATH'])
        for expected in ('configure required', 'matching kept tree'):
            subprocess.run(['bash', '-euo', 'pipefail', '-c', script], env=env, check=True, capture_output=True, text=True)
            self.assertIn(expected, self.build.with_suffix('.configure.log').read_text())
        env['OBS_EXTRA_CMAKE_ARGS'] = '-DENABLE_UNIT_TESTS=OFF'
        # The later ON in this script still wins, but changed argv must configure.
        subprocess.run(['bash', '-euo', 'pipefail', '-c', script], env=env, check=True, capture_output=True, text=True)
        self.assertIn('configure required', self.build.with_suffix('.configure.log').read_text())


if __name__ == '__main__':
    unittest.main()
