#!/usr/bin/env -S uv run --script
"""The stale-source regression must also run without a compiler (#5009)."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / 'test/cmocka/check_core_module_manifest_source.cmake'


class FixtureTest(unittest.TestCase):
    def invoke(self, directory, *extra, prepared=False):
        return subprocess.run([
            'cmake', f'-DOBS_CORE_MODULES_CMAKE={ROOT}/libobs/cmake/obs-core-modules.cmake',
            f'-DTEST_FIXTURE_ROOT={directory}', '-DTEST_GENERATOR=Ninja', *extra,
            '-P', str(SCRIPT),
        ], env={**os.environ, 'OBS_TIER2_FIXTURE_PREPARED': '1' if prepared else ''},
            capture_output=True, text=True, check=False)

    def test_preparation_and_compiler_free_assertions(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture = Path(directory) / 'fixture'
            result = self.invoke(fixture, '-DTEST_PREPARE_ONLY=ON')
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            evidence = fixture / 'evidence.cmake'
            self.assertTrue(evidence.is_file())
            # A poison compiler/toolchain would fail if assertions configured again.
            result = self.invoke(fixture, '-DTEST_C_COMPILER=/no/compiler', prepared=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            (fixture / 'source/obs-core-modules.c').write_text('int wrong_source;\n')
            result = self.invoke(fixture, prepared=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('changed prepared fixture', result.stdout + result.stderr)

    def test_missing_preparation_fails_closed(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.invoke(directory, prepared=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('missing prepared fixture', result.stdout + result.stderr)


class NativeSplitTest(unittest.TestCase):
    def test_contract_has_explicit_assert_only_receipt(self):
        source = (ROOT / 'rust/obs-build/tests/libobs.rs').read_text()
        self.assertIn('OBS_BUILD_MACOS_ASSERT_ONLY', source)
        self.assertIn('prepared.finish()', source)
        self.assertIn('prepared.get("cores"', source)
        self.assertIn('prepared.get("capture-probe"', source)
        self.assertIn('prepared.get("graphics-probe"', source)

    def test_shared_runtime_script_retains_all_negative_controls(self):
        script = ROOT / 'rust/tools/tier2-validate/macos-assertions.sh'
        self.assertTrue(script.is_file())
        source = script.read_text()
        self.assertEqual(source.count('"$status" -eq 14'), 2)
        self.assertEqual(source.count('"$status" -eq 12'), 2)
        self.assertIn('diff -r "$capture_data" "$reference_data"', source)
        self.assertIn('DYLD_PRINT_LIBRARIES=1', source)
        self.assertNotIn('soldr cargo', source)


if __name__ == '__main__':
    unittest.main()
