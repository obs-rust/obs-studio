#!/usr/bin/env -S uv run --script
"""Exercise relocation and fail-closed execution of the Tier 2 bundle."""
from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from ci import macos_tier2_bundle as bundle


class BundleTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.build = self.root / 'producer'
        self.build.mkdir()
        self.source = self.root / 'source'
        self.source.mkdir()
        self.output = self.root / 'bundle'
        self.binary = self.build / 'test_probe'
        self.binary.write_bytes(b'fixture')
        self.registry = self.build / 'CTestTestfile.cmake'
        self.registry.write_text(
            f'add_test(probe "{self.binary}")\n'
            'set_tests_properties(probe PROPERTIES TIMEOUT 17 RUN_SERIAL TRUE '
            'SKIP_RETURN_CODE 77 ENVIRONMENT "OBS_TEST=yes")\n'
        )

    def stage(self):
        return bundle.stage(self.build, self.source, self.output, [self.binary], lambda _: None)

    def test_relocation_preserves_ctest_properties(self):
        self.stage()
        moved = self.root / 'relocated'
        self.output.rename(moved)
        bundle.prepare(moved, self.source)
        registry = (moved / 'build/CTestTestfile.cmake').read_text()
        self.assertIn(str(moved / 'build/test_probe'), registry)
        self.assertNotIn(str(self.build), registry)
        for assertion in ('TIMEOUT 17', 'RUN_SERIAL TRUE', 'SKIP_RETURN_CODE 77', 'OBS_TEST=yes'):
            self.assertIn(assertion, registry)

    def test_missing_binary_or_registry_is_rejected(self):
        with self.assertRaises(FileNotFoundError):
            bundle.stage(self.build, self.source, self.output, [self.build / 'absent'], lambda _: None)
        self.registry.rename(self.build / 'saved')
        with self.assertRaises(ValueError):
            self.stage()

    def test_tampering_is_rejected_before_any_process_runs(self):
        self.stage()
        (self.output / 'build/test_probe').write_bytes(b'changed')
        with patch.object(subprocess, 'run') as run:
            with self.assertRaises(ValueError):
                bundle.run(self.output, self.source, 'shared', 1)
            run.assert_not_called()

    def test_source_root_and_escaping_symlink_are_rejected(self):
        self.stage()
        with self.assertRaises(ValueError):
            bundle.prepare(self.output, self.root / 'different-source')
        outside = self.root / 'outside'
        outside.write_bytes(b'outside')
        link = self.build / 'escape'
        link.symlink_to(outside)
        with self.assertRaises(ValueError):
            bundle.stage(self.build, self.source, self.root / 'bad', [link], lambda _: None)

    def test_nonzero_test_exit_is_propagated(self):
        self.stage()
        with patch.object(subprocess, 'run', return_value=subprocess.CompletedProcess([], 8)) as run:
            self.assertEqual(bundle.run(self.output, self.source, 'shared', 3), 8)
            args, kwargs = run.call_args
            self.assertIn('--output-on-failure', args[0])
            self.assertIn('3', args[0])
            self.assertEqual(kwargs['env']['DYLD_FRAMEWORK_PATH'], str(self.output / 'build/cores/shared'))

    def test_failed_signature_verification_prevents_bundle_creation(self):
        def reject(_):
            raise ValueError('invalid signature')
        with self.assertRaisesRegex(ValueError, 'invalid signature'):
            bundle.stage(self.build, self.source, self.output, [self.binary], reject)
        self.assertFalse((self.output / 'manifest.json').exists())

    def test_manifest_records_incomplete_native_coverage(self):
        self.stage()
        manifest = json.loads((self.output / 'manifest.json').read_text())
        self.assertFalse(manifest['native_contract_prebuilt'])
        self.assertEqual(manifest['target'], 'aarch64-apple-darwin')


if __name__ == '__main__':
    unittest.main()
