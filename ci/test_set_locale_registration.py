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
        expected = sorted((ROOT / 'libobs' / source).resolve()
                          for source in json.loads(MANIFEST.read_text()))
        self.assertIn(ROOT / 'test/cmocka/core_set_locale_fixture.c', expected)
        fragment = ROOT / 'test/cmocka/tests.d/test_set_locale.cmake'
        with tempfile.TemporaryDirectory() as directory:
            source = ROOT / 'ci/fixtures/locale_registration'
            for host in ('Linux', 'Darwin', 'Windows'):
                with self.subTest(host=host):
                    build = Path(directory) / host
                    result = subprocess.run(
                        ['cmake', '-S', str(source), '-B', str(build),
                         f'-DCMAKE_SYSTEM_NAME={host}', f'-DFRAGMENT={fragment}'],
                        text=True, capture_output=True, check=False)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    for library in ('libobs', 'libobs-c-reference'):
                        actual = sorted(Path(line).resolve() for line in
                                        (build / f'{library}.sources').read_text().splitlines())
                        self.assertEqual(actual, expected)


if __name__ == '__main__':
    unittest.main()
