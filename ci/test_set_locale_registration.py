"""The test-enabled CMake libraries use the same fixture set as obs-build."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'libobs/build/source-sets.d/LIBOBS_TEST_SOURCES.json'


class SetLocaleRegistrationTest(unittest.TestCase):
    @unittest.skipUnless(shutil.which('cmake'), 'cmake is required')
    def test_fixture_sources_match_on_every_host(self):
        expected = sorted(str((ROOT / 'libobs' / source).resolve())
                          for source in json.loads(MANIFEST.read_text()))
        self.assertIn(str(ROOT / 'test/cmocka/core_set_locale_fixture.c'), expected)
        fragment = ROOT / 'test/cmocka/tests.d/test_set_locale.cmake'
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / 'CMakeLists.txt').write_text('''
cmake_minimum_required(VERSION 3.28)
project(locale_fixture NONE)
# Capture actual target_sources calls without requiring any platform SDK.
add_custom_target(libobs)
add_custom_target(libobs-c-reference)
function(target_sources target)
  foreach(source IN LISTS ARGN)
    if(NOT source STREQUAL "PRIVATE")
      cmake_path(ABSOLUTE_PATH source NORMALIZE OUTPUT_VARIABLE absolute)
      file(APPEND "${CMAKE_BINARY_DIR}/${target}.sources" "${absolute}\\n")
    endif()
  endforeach()
endfunction()
function(target_link_libraries)
endfunction()
function(add_executable)
  add_custom_target(${ARGV0})
endfunction()
function(target_include_directories)
endfunction()
add_custom_target(cmocka-tests)
include("${FRAGMENT}")
''')
            for host in ('Linux', 'Darwin', 'Windows'):
                with self.subTest(host=host):
                    build = source / host
                    result = subprocess.run(
                        ['cmake', '-S', str(source), '-B', str(build),
                         f'-DCMAKE_SYSTEM_NAME={host}', f'-DFRAGMENT={fragment}'],
                        text=True, capture_output=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    for library in ('libobs', 'libobs-c-reference'):
                        actual = sorted((build / f'{library}.sources').read_text().splitlines())
                        self.assertEqual(actual, expected)


if __name__ == '__main__':
    unittest.main()
