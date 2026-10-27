#!/usr/bin/env -S uv run --script
"""Cheap toolchain identity fixtures, without native builds or network."""

import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location('native_cache_identity', Path(__file__).with_name('native_cache_identity.py'))
IDENTITY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IDENTITY)


class NativeCacheIdentityTest(unittest.TestCase):
    def test_macos_identity_changes_with_selected_sdk_or_compiler(self):
        versions = {'xcode': 'Xcode 26.5\nBuild version 17F10', 'compiler': 'Apple clang version 18.0.0',
                    'sdk': '26.5', 'sdk_build': '26F10'}
        with mock.patch.object(IDENTITY, 'tool_versions', return_value=versions):
            first = IDENTITY.capture('macos', 'arm64', 'RelWithDebInfo')
        for field in versions:
            with self.subTest(field=field), mock.patch.object(IDENTITY, 'tool_versions', return_value=dict(versions, **{field: 'changed'})):
                self.assertNotEqual(IDENTITY.capture('macos', 'arm64', 'RelWithDebInfo'), first)
        self.assertEqual(set(first), {'platform', 'architecture', 'configuration', 'compiler', 'sdk', 'cache_format'})

    def test_windows_inventory_and_compiler_content_invalidate_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            compiler = root / 'cl.exe'
            compiler.write_bytes(b'compiler one')
            versions = {'visual_studio': '18.1', 'default_tools': '14.50', 'available_tools': ['14.50'],
                        'sdk_versions': ['10.0.26100.0'], 'compiler_path': str(compiler)}
            with mock.patch.object(IDENTITY, 'tool_versions', return_value=versions):
                first = IDENTITY.capture('windows', 'x64', 'RelWithDebInfo')
                compiler.write_bytes(b'compiler two')
                self.assertNotEqual(IDENTITY.capture('windows', 'x64', 'RelWithDebInfo'), first)
            compiler.write_bytes(b'compiler one')
            for field, value in (('visual_studio', '18.2'), ('default_tools', '14.51'),
                                 ('available_tools', ['14.50', '14.51']), ('sdk_versions', ['10.0.26200.0'])):
                with self.subTest(field=field), mock.patch.object(IDENTITY, 'tool_versions', return_value=dict(versions, **{field: value})):
                    self.assertNotEqual(IDENTITY.capture('windows', 'x64', 'RelWithDebInfo'), first)

    def test_mismatch_preserves_captured_identity_and_still_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            current = {'platform': 'windows', 'architecture': 'x64', 'configuration': 'RelWithDebInfo',
                       'compiler': 'new-compiler', 'sdk': 'sdk', 'cache_format': 'ccache-4.14.1'}
            expected = root / 'expected.json'
            expected.write_text(json.dumps(dict(current, compiler='old-compiler')))
            output = root / 'current.json'
            arguments = ['identity', '--platform', 'windows', '--architecture', 'x64',
                         '--configuration', 'RelWithDebInfo', '--output', str(output), '--expected', str(expected)]
            with mock.patch.object(sys, 'argv', arguments), mock.patch.object(IDENTITY, 'capture', return_value=current):
                with self.assertRaisesRegex(ValueError, 'compiler'):
                    IDENTITY.main()
            self.assertEqual(json.loads(output.read_text()), current)
            self.assertEqual(json.loads(expected.read_text())['compiler'], 'old-compiler')

    def test_mismatch_diagnostics_record_image_and_only_differing_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            current = {'platform': 'windows', 'architecture': 'x64', 'configuration': 'Release',
                       'compiler': 'new', 'sdk': 'sdk', 'cache_format': 'ccache-4.14.1'}
            expected = root / 'expected.json'
            expected.write_text(json.dumps(dict(current, compiler='old')))
            diagnostics = root / 'diagnostics.json'
            arguments = ['identity', '--platform', 'windows', '--architecture', 'x64', '--configuration', 'Release',
                         '--output', str(root / 'current.json'), '--expected', str(expected),
                         '--diagnostics', str(diagnostics)]
            with mock.patch.object(sys, 'argv', arguments), mock.patch.object(IDENTITY, 'capture', return_value=current), mock.patch.dict(IDENTITY.os.environ, {'ImageVersion': '20261007.264.1', 'ImageOS': 'win25-vs2026'}, clear=True):
                with self.assertRaises(ValueError):
                    IDENTITY.main()
            report = json.loads(diagnostics.read_text())
            self.assertEqual(report['status'], 'mismatch')
            self.assertEqual(report['differing_fields'], ['compiler'])
            self.assertEqual(report['runner_image_version'], '20261007.264.1')
            self.assertEqual(report['runner_image_os'], 'win25-vs2026')

    def test_configuration_and_architecture_are_explicit(self):
        versions = {'xcode': '26.5', 'compiler': 'clang18', 'sdk': '26.5', 'sdk_build': '26F10'}
        with mock.patch.object(IDENTITY, 'tool_versions', return_value=versions):
            first = IDENTITY.capture('macos', 'arm64', 'Release')
            self.assertNotEqual(first, IDENTITY.capture('macos', 'x86_64', 'Release'))
            self.assertNotEqual(first, IDENTITY.capture('macos', 'arm64', 'RelWithDebInfo'))
        with self.assertRaises(ValueError):
            IDENTITY.capture('linux', 'x86_64', 'Release')


if __name__ == '__main__':
    unittest.main()
