"""Dell packs favour CPU time; hosted archives retain their existing policy."""
import importlib
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent


class DellCompressionTest(unittest.TestCase):
    def test_tier2_workflow_passes_runner_and_handoff_selection(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        build = workflow.split('      - name: Build Tier 2 (libobs and libobs-c-reference)', 1)[1].split('        run:', 1)[0]
        for setting in ('OBS_TIER2_PERSISTENT: ${{ steps.pcache.outputs.zccache }}',
                        'OBS_TIER2_HANDOFF: ${{ steps.pcache.outputs.handoff }}',
                        'MQ_RUNNER_LABELS: ${{ inputs.mq-runner-labels }}'):
            self.assertIn(setting, build)
        self.assertIn('ci/tier2_pack.py', (ROOT / 'rust/tools/linux-validate/validate.sh').read_text())

    def test_both_tier2_readers_autodetect_old_and_new_packs(self):
        workflow = (ROOT / '.github/workflows/build-project.yaml').read_text()
        unpack = workflow.split('      - name: Unpack Tier 2 Tree', 1)[1].split('\n      - ', 1)[0]
        self.assertIn('tar -xf', unpack)
        prepared = (ROOT / 'ci/ci_gate_steps.py').read_text().split('def validate_prepared_suites()', 1)[1]
        self.assertNotIn("'--zstd'", prepared)

    def test_actions_cache_upload_is_level_one_and_multithreaded_only_on_dell(self):
        action = (ROOT / '.github/actions/cache/action.yml').read_text()
        save = action.split('    - name: Save cache', 1)[1]
        self.assertIn("ZSTD_CLEVEL: ${{ runner.environment == 'self-hosted' && '1' || env.ZSTD_CLEVEL }}", save)
        self.assertIn("ZSTD_NBTHREADS: ${{ runner.environment == 'self-hosted' && '0' || env.ZSTD_NBTHREADS }}", save)

    def test_setup_bundle_is_still_hosted_level_three_two_threads(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        producer = workflow.split('  tier2-setup-producer:', 1)[1].split('  tier2-setup-consumer:', 1)[0]
        self.assertIn('runs-on: ubuntu-26.04', producer)
        probe = (ROOT / 'ci/tier2_setup_probe.py').read_text()
        self.assertIn("'zstd', '-T2', '-3'", probe)
        self.assertIn('compression-level: 0', producer)

    def test_dell_routed_artifact_steps_skip_zip_compression_hosted_keeps_six(self):
        # No YAML dependency: job headers and pinned upload callsites are enough.
        for filename in ('build-project.yaml', 'ci.yml', 'check-format.yaml'):
            text = (ROOT / '.github/workflows' / filename).read_text()
            for job in re.split(r'(?m)^  [a-z0-9-]+:\n', text)[1:]:
                runner = re.search(r'(?m)^    runs-on: (.*)', job)
                if not runner or 'mq-runner' not in runner[1]:
                    continue
                for step in re.split(r'(?m)^      - ', job)[1:]:
                    if 'uses: actions/upload-artifact@' not in step:
                        continue
                    with self.subTest(workflow=filename, step=step.splitlines()[0]):
                        self.assertRegex(step, r"compression-level: (?:0|\$\{\{ runner.environment == 'self-hosted' && '0' \|\| '6' \}\})\n")

    def test_pack_commands_and_roundtrip(self):
        pack = importlib.import_module('tier2_pack')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            build = root / 'build'
            (build / 'tree/cargo').mkdir(parents=True)
            (build / 'tree/test').write_text('real payload\n' * 20)
            (build / 'tree/excluded.o').write_text('object')
            (build / 'tree/cargo/excluded').write_text('cargo')
            (build / 'tree/link').symlink_to('test')
            marker = root / 'marker'
            marker.write_text('dell-fsx3mn2\n')
            for persistent, handoff, labels, options in (
                ('', '', '', ['--zstd']),
                ('cache', '', '["self-hosted","dell-fsx3mn2"]', ['-I', 'zstd -1 -T0']),
                ('cache', 'handoff', '["self-hosted","other"]', ['-I', 'zstd -1 -T0']),
                ('cache', 'handoff', 'invalid', ['-I', 'zstd -1 -T0']),
                ('cache', 'handoff', '["self-hosted","dell-fsx3mn2"]', []),
            ):
                with self.subTest(persistent=persistent, handoff=handoff, labels=labels):
                    env = {'OBS_TIER2_PERSISTENT': persistent, 'OBS_TIER2_HANDOFF': handoff, 'MQ_RUNNER_LABELS': labels}
                    output = root / f'pack-{len(list(root.glob("pack-*")))}'
                    with patch.dict(os.environ, env, clear=True), patch.object(pack.subprocess, 'run', wraps=subprocess.run) as run:
                        pack.pack(build, output, marker)
                    self.assertEqual(run.call_args.args[0][1:1 + len(options)], options)
                    archive = output / 'tier2-tree.tar.zst'
                    compressed = archive.read_bytes()[:4] == bytes.fromhex('28b52ffd')
                    self.assertEqual(compressed, bool(options))
                    dest = output / 'extract'
                    dest.mkdir()
                    subprocess.run(['tar', '-xf', str(archive), '-C', str(dest)], check=True)
                    self.assertEqual((dest / 'tree/test').read_bytes(), (build / 'tree/test').read_bytes())
                    self.assertTrue((dest / 'tree/link').is_symlink())
                    self.assertFalse((dest / 'tree/excluded.o').exists())
                    self.assertFalse((dest / 'tree/cargo').exists())


if __name__ == '__main__':
    unittest.main()
