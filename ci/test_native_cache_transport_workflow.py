#!/usr/bin/env -S uv run --script
"""Trust and scope checks for the opt-in native cache transport route."""

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent.parent


class TransportWorkflowTest(unittest.TestCase):
    def test_publisher_is_manual_master_only_separate_and_serialized(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        build = (ROOT / '.github/workflows/build-project.yaml').read_text()
        job = build.split('  native-cache-transport:\n', 1)[1]
        self.assertIn("github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master' && inputs.native-cache-transport-probe", job)
        self.assertIn('needs: [macos-build, windows-build]', job)
        self.assertIn('contents: write', job)
        self.assertIn('cancel-in-progress: false', job)
        self.assertIn('fail-fast: false', job)
        # The seed comes from this run's app build as an artifact: the
        # Actions cache has no room for it (#1801).
        self.assertNotIn('uses: ./.github/actions/cache', job)
        self.assertIn('uses: actions/download-artifact@', job)
        self.assertIn('name: native-cache-seed-${{ matrix.platform }}-${{ steps.native-seed.outputs.suffix }}', job)
        self.assertIn('NATIVE_CACHE_JOB_TOKEN: ${{ github.token }}', job)
        self.assertNotIn('ci/native_cache_transport.py consume', job)
        self.assertNotIn('--expected ', job)
        self.assertIn("--identity '${{ runner.temp }}/native-cache-build-identity/native-cache-identity.json' --source", job)
        build_job = workflow.split('  build-project:\n', 1)[1].split('\n  #', 1)[0]
        self.assertIn('contents: write', build_job)
        self.assertIn('permissions:\n  actions: read\n  contents: read\n', build)
        self.assertEqual(build.count('contents: write'), 1)

    def test_roundtrip_has_a_separate_read_only_token_and_required_source(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('  native-cache-consumer:\n', workflow)
        job = workflow.split('  native-cache-consumer:\n', 1)[1].split('\n  ci-gate:', 1)[0]
        self.assertIn('needs: build-project', job)
        self.assertIn('always() && !cancelled()', job)
        self.assertIn("github.event_name == 'workflow_dispatch' && github.ref == 'refs/heads/master' && inputs.native-cache-transport-probe", job)
        self.assertIn('contents: read', job)
        self.assertIn('actions: read', job)
        self.assertNotIn('contents: write', job)
        self.assertIn('NATIVE_CACHE_JOB_TOKEN: ${{ github.token }}', job)
        self.assertIn('--require-hit --expected-source', job)
        self.assertIn('native-cache-build-identity/native-cache-identity.json', job)
        self.assertIn('fail-fast: false', job)
        self.assertIn('runs-on: ubuntu-26.04', job)
        gate = workflow.split('  ci-gate:\n', 1)[1]
        needs = next(line.strip() for line in gate.splitlines() if line.strip().startswith('needs: ['))
        self.assertIn('native-cache-consumer', needs.removeprefix('needs: [').removesuffix(']').split(', '))
        self.assertIn('build-project,', gate)

    def test_qualification_seed_is_profile_bound_and_uploaded_once(self):
        build = (ROOT / '.github/workflows/build-project.yaml').read_text()
        for platform, boundary in (('macos', '\n  ubuntu-build:'), ('windows', '\n  # zackees/ci.yml GATE-008')):
            job = build.split('  ' + platform + '-build:\n', 1)[1].split(boundary, 1)[0]
            self.assertIn('id: native-seed', job)
            self.assertIn('ci/native_cache_seed.py', job)
            self.assertIn('- name: Upload Qualified Native Transport Seed\n', job)
            upload = job.split('- name: Upload Qualified Native Transport Seed\n', 1)[1].split('\n      - ', 1)[0]
            self.assertIn("if: inputs.native-cache-reuse-source == '' && inputs.native-cache-transport-probe && github.event_name != 'pull_request' && github.ref == 'refs/heads/master'", upload)
            self.assertIn('uses: actions/upload-artifact@', upload)
            self.assertIn('name: native-cache-seed-' + platform + '-${{ steps.native-seed.outputs.suffix }}', upload)
            self.assertIn('include-hidden-files: true', upload)
            self.assertIn('retention-days: 1', upload)
            self.assertIn('if-no-files-found: error', upload)
            self.assertEqual(job.count('native-cache-seed-'), 1)
            self.assertLess(job.index('Record Native Transport Identity'), job.index('id: native-seed'))
            self.assertLess(job.index('Qualify Local Native Cache Archive'), job.index('Upload Qualified Native Transport Seed'))

    def test_identity_evidence_is_default_off_and_matches_native_axes(self):
        workflow = (ROOT / '.github/workflows/ci.yml').read_text()
        build = (ROOT / '.github/workflows/build-project.yaml').read_text()
        for text in (workflow, build):
            setting = text.split('      native-cache-transport-probe:\n', 1)[1].split('\n      ', 1)[0]
            # Remaining fields are read from the complete input block below.
            self.assertTrue(setting)
            self.assertIn('default: false', text.split('      native-cache-transport-probe:\n', 1)[1].split('\n      native-cache-probe:', 1)[0])
        for platform, boundary in (('macos', '\n  ubuntu-build:'), ('windows', '\n  # zackees/ci.yml GATE-008')):
            job = build.split('  ' + platform + '-build:\n', 1)[1].split(boundary, 1)[0]
            self.assertIn('inputs.native-cache-transport-probe', job)
            self.assertIn('ci/native_cache_identity.py', job)
            self.assertIn('native-cache-identity-' + platform, job)
            self.assertIn('retention-days: 1', job)


if __name__ == '__main__':
    unittest.main()
