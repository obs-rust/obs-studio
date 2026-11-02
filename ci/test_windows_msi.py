#!/usr/bin/env -S uv run --script
"""The Windows BETA MSI installer's packaging settings and release wiring (#3476).

    uv run python -m unittest ci/test_windows_msi.py

The MSI itself can only be built on Windows; the release's `windows_msi`
job builds, signs and round-trips it. These checks run anywhere: the CPack
per-generator file is evaluated by real CMake for both generators, and the
release workflow is checked to build, validate and publish the MSI.
"""

from __future__ import annotations

import importlib.util
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
GENERATOR_CONFIG = ROOT / 'cmake/windows/cpack-wix-generator.cmake'
RELEASE = ROOT / '.github/workflows/release-rust.yaml'

spec = importlib.util.spec_from_file_location('windows_msi', ROOT / 'ci/windows_msi.py')
windows_msi = importlib.util.module_from_spec(spec)
spec.loader.exec_module(windows_msi)


class GeneratorConfig(unittest.TestCase):
    def evaluate(self, generator: str) -> dict[str, str]:
        if not shutil.which('cmake'):
            self.skipTest('cmake is unavailable')
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / 'probe.cmake'
            script.write_text(
                'set(CPACK_PACKAGE_NAME obs-studio)\n'
                'set(CPACK_PACKAGE_VERSION 32.1.0-beta2)\n'
                f'set(CPACK_GENERATOR {generator})\n'
                f'include("{GENERATOR_CONFIG.as_posix()}")\n'
                'message(STATUS "NAME=${CPACK_PACKAGE_NAME}")\n'
                'message(STATUS "VERSION=${CPACK_PACKAGE_VERSION}")\n'
                'message(STATUS "PRE=${CPACK_PRE_BUILD_SCRIPTS}")\n'
            )
            result = subprocess.run(['cmake', '-P', str(script)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        values = {}
        for line in (result.stdout + result.stderr).splitlines():
            key, _, value = line.removeprefix('-- ').partition('=')
            values[key] = value
        return values

    def test_msi_is_branded_beta_with_a_numeric_version_and_no_pdbs(self):
        values = self.evaluate('WIX')
        self.assertEqual(values['NAME'], 'OBS-Studio-Rust BETA')
        self.assertEqual(values['VERSION'], '32.1.0')
        self.assertTrue(values['PRE'].endswith('cpack-wix-strip-pdb.cmake'))

    def test_portable_zip_is_unchanged(self):
        values = self.evaluate('ZIP')
        self.assertEqual(values['NAME'], 'obs-studio')
        self.assertEqual(values['VERSION'], '32.1.0-beta2')
        self.assertEqual(values['PRE'], '')

    def test_pdb_strip_script_removes_only_pdbs(self):
        if not shutil.which('cmake'):
            self.skipTest('cmake is unavailable')
        with tempfile.TemporaryDirectory() as directory:
            stage = Path(directory)
            (stage / 'bin/64bit').mkdir(parents=True)
            (stage / 'bin/64bit/obs64.exe').write_text('exe')
            (stage / 'bin/64bit/obs64.pdb').write_text('pdb')
            script = ROOT / 'cmake/windows/cpack-wix-strip-pdb.cmake'
            subprocess.run(['cmake', f'-DCPACK_TEMPORARY_DIRECTORY={stage}', '-P', str(script)],
                           check=True)
            self.assertTrue((stage / 'bin/64bit/obs64.exe').exists())
            self.assertFalse((stage / 'bin/64bit/obs64.pdb').exists())

    def test_windows_cpack_config_wires_the_wix_settings(self):
        text = (ROOT / 'cmake/windows/cpackconfig.cmake').read_text()
        for needle in ('CPACK_PROJECT_CONFIG_FILE', 'CPACK_WIX_UPGRADE_GUID',
                       'CPACK_WIX_UI_REF WixUI_InstallDir', 'CPACK_PACKAGE_EXECUTABLES obs64',
                       'CPACK_WIX_ARCHITECTURE arm64', 'set(CPACK_GENERATOR ZIP)'):
            self.assertIn(needle, text)


class Signature(unittest.TestCase):
    def test_valid_always_passes(self):
        self.assertIsNone(windows_msi.check_signature('Valid', required=True))

    def test_unsigned_fails_only_when_required(self):
        self.assertIsNone(windows_msi.check_signature('NotSigned', required=False))
        self.assertIsNotNone(windows_msi.check_signature('NotSigned', required=True))

    def test_a_bad_signature_always_fails(self):
        for status in ('HashMismatch', 'NotTrusted', 'UnknownError'):
            self.assertIsNotNone(windows_msi.check_signature(status, required=False))


class ReleaseWiring(unittest.TestCase):
    def test_release_builds_round_trips_and_publishes_the_msi(self):
        text = RELEASE.read_text()
        self.assertIn('ci/windows_msi.py build', text)
        self.assertIn('ci/windows_msi.py sign', text)
        self.assertIn('ci/windows_msi.py roundtrip', text)
        self.assertIn('windows-$env:TARGET.msi', text)
        release_needs = text.split('\n  release:\n', 1)[1].split('needs:', 1)[1].splitlines()[0]
        self.assertIn('windows_msi', release_needs)


if __name__ == '__main__':
    unittest.main()
