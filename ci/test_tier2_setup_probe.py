"""Prepared-environment measurements require immutable, complete evidence (#1592)."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('setup_probe', ROOT / 'ci/tier2_setup_probe.py')
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class ProbeTest(unittest.TestCase):
    def test_corrupt_archive_is_rejected_before_loading_an_image(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'image.tar.zst').write_bytes(b'corrupt')
            (root / 'bundle.json').write_text(json.dumps({
                'archive_sha256': hashlib.sha256(b'original').hexdigest(),
                'image_id': 'sha256:' + 'a' * 64,
                'inventory': 'example\\t1\\n',
            }))
            with patch.object(probe, 'command') as command:
                with self.assertRaisesRegex(ValueError, 'checksum'):
                    probe.verify_bundle(root)
                command.assert_not_called()

    def test_measure_rejects_corruption_before_any_docker_command(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'image.tar.zst').write_bytes(b'corrupt')
            (root / 'bundle.json').write_text(json.dumps({
                'archive_sha256': hashlib.sha256(b'original').hexdigest(),
            }))
            with patch.object(probe, 'command') as command, patch.object(probe.subprocess, 'run') as run:
                with self.assertRaisesRegex(ValueError, 'checksum'):
                    probe.measure(root, 'prepared', root)
                command.assert_not_called()
                run.assert_not_called()

    def test_image_identity_is_not_an_arbitrary_docker_argument(self):
        with self.assertRaises(ValueError):
            probe.image_id('--privileged')
        self.assertEqual(probe.image_id('sha256:' + 'a' * 64), 'sha256:' + 'a' * 64)

    def test_package_inventory_comparison_is_order_independent_but_exact(self):
        self.assertEqual(probe.inventory('a:amd64\t1\nb\t2\n'), probe.inventory('b\t2\na:amd64\t1\n'))
        self.assertNotEqual(probe.inventory('a\t1\n'), probe.inventory('a\t2\n'))
        for invalid in ('a\t1\na\t2\n', 'not an inventory', ''):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                probe.inventory(invalid)

    def test_only_exact_package_versions_can_enter_the_experiment(self):
        self.assertEqual(probe.package_specs('libc6:amd64=2.42-0ubuntu3\n'), ['libc6:amd64=2.42-0ubuntu3'])
        for text in ('--allow-unauthenticated\n', 'libc6\n', 'a=1; touch x\n', ''):
            with self.subTest(text=text), self.assertRaises(ValueError):
                probe.package_specs(text)


class ProbeWorkflowTest(unittest.TestCase):
    def test_manual_probe_is_master_only_and_does_not_replace_normal_build(self):
        text = (ROOT / '.github/workflows/ci.yml').read_text()
        producer = text.split('  tier2-setup-producer:', 1)[1].split('  tier2-setup-consumer:', 1)[0]
        self.assertIn("github.event_name == 'workflow_dispatch'", producer)
        self.assertIn("github.ref == 'refs/heads/master'", producer)
        self.assertIn('inputs.tier2-setup-probe', producer)
        self.assertIn('needs: build-project', producer)
        self.assertIn('always() && !cancelled()', producer)
        normal = text.split('  build-project:', 1)[1].split('  flatpak-runtime-reuse:', 1)[0]
        # The setup flag may be referenced for conflicting-input validation,
        # but must not select/replace the normal build or become its own input.
        normal_condition = normal.split('    if: ', 1)[1].splitlines()[0]
        self.assertNotIn('tier2-setup-probe', normal_condition)
        self.assertNotIn('\n      tier2-setup-probe:', normal)
        self.assertNotIn('actions/cache/save@', producer)

    def test_stock_does_not_pay_for_prepared_image_transfer(self):
        text = (ROOT / '.github/workflows/ci.yml').read_text()
        consumers = text.split('  tier2-setup-consumer:', 1)[1].split('  validate-data:', 1)[0]
        image_step = consumers.split('- name: Download Prepared Image', 1)[1].split('- name: Measure', 1)[0]
        self.assertIn("matrix.variant == 'prepared'", image_step)
        self.assertIn('variant: [stock, prepared]', consumers)
        self.assertIn('trial: [1, 2, 3]', consumers)
        self.assertIn('fail-fast: false', consumers)


if __name__ == '__main__':
    unittest.main()
