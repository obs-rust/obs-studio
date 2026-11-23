#!/usr/bin/env -S uv run --script
"""Prebaking must preserve hosted fallback, package identity and writer isolation."""
import email.message
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

from test_ci_pre import jobs, needs, upstream
from test_workflow_duplicate_keys import duplicate_keys, truncated_scalars

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / 'ci/tier2_image.py'
BUILD = ROOT / '.github/workflows/build-project.yaml'
PUSH = ROOT / '.github/workflows/push.yaml'


def load():
    spec = importlib.util.spec_from_file_location('tier2_image', HELPER)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class ImageTest(unittest.TestCase):
    def setUp(self):
        self.image = load()

    def test_identity_is_deterministic_and_covers_every_input(self):
        image = self.image
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for path in image.INPUTS:
                (root / path).parent.mkdir(parents=True, exist_ok=True)
                (root / path).write_bytes((ROOT / path).read_bytes())
            original = image.identity(root)
            self.assertEqual(original, image.identity(root))
            self.assertRegex(original, r'^ghcr.io/obs-rust/tier2-linux:[0-9a-f]{64}$')
            for path in image.INPUTS:
                file = root / path
                before = file.read_bytes()
                file.write_bytes(before + b'\n# changed\n')
                self.assertNotEqual(original, image.identity(root), path)
                file.write_bytes(before)
            dockerfile = root / image.DOCKERFILE
            dockerfile.write_text(dockerfile.read_text().replace('f144425f', 'a144425f'))
            self.assertNotEqual(original, image.identity(root))

    def test_dell_selects_digest_and_hosted_never_probes(self):
        image = self.image
        digest = 'sha256:' + 'a' * 64
        with patch.object(image, 'manifest_digest', return_value=digest) as probe:
            self.assertEqual(image.select_image('', ROOT), image.base_image(ROOT))
            probe.assert_not_called()
            self.assertEqual(image.select_image('["self-hosted"]', ROOT), image.REPOSITORY + '@' + digest)
            probe.assert_called_once_with(image.identity(ROOT))

    def test_missing_or_error_or_invalid_digest_falls_back(self):
        for result in (None, '', 'latest', 'sha256:bad'):
            with self.subTest(result=result), patch.object(self.image, 'manifest_digest', return_value=result):
                self.assertEqual(self.image.select_image('["self-hosted"]', ROOT), self.image.base_image(ROOT))
        for error in (urllib.error.URLError('offline'), TimeoutError(), ValueError('bad response'), OSError('offline'), RuntimeError('protocol failure')):
            with self.subTest(error=error), patch.object(self.image, 'manifest_digest', side_effect=error):
                self.assertEqual(self.image.select_image('["self-hosted"]', ROOT), self.image.base_image(ROOT))

    def test_manifest_probe_uses_anonymous_token_and_verifies_digest(self):
        body = b'{"schemaVersion":2}'
        digest = 'sha256:' + hashlib.sha256(body).hexdigest()
        token, manifest = io.BytesIO(b'{"token":"anonymous-pull"}'), io.BytesIO(body)
        manifest.headers = {'Docker-Content-Digest': digest}
        with patch.object(self.image.urllib.request, 'urlopen', side_effect=[token, manifest]) as open_url:
            self.assertEqual(self.image.manifest_digest(self.image.identity(ROOT)), digest)
            request = open_url.call_args.args[0]
            self.assertEqual(request.get_header('Authorization'), 'Bearer anonymous-pull')
            self.assertIn('/manifests/', request.full_url)
            self.assertLessEqual(open_url.call_args.kwargs['timeout'], 10)
        manifest = io.BytesIO(body)
        manifest.headers = {'Docker-Content-Digest': 'sha256:' + 'b' * 64}
        with patch.object(self.image.urllib.request, 'urlopen', side_effect=[io.BytesIO(b'{"token":"a"}'), manifest]):
            with self.assertRaises(ValueError):
                self.image.manifest_digest(self.image.identity(ROOT))

    def test_builder_rejects_untrusted_events_before_probe_or_docker(self):
        for event, ref, repo in [('pull_request', 'refs/heads/master', 'obs-rust/obs-studio'),
                                 ('merge_group', 'refs/heads/master', 'obs-rust/obs-studio'),
                                 ('push', 'refs/tags/v1', 'obs-rust/obs-studio'),
                                 ('push', 'refs/heads/master', 'fork/obs-studio')]:
            env = {'GITHUB_EVENT_NAME': event, 'GITHUB_REF': ref, 'GITHUB_REPOSITORY': repo}
            with self.subTest(env=env), patch.dict(os.environ, env), \
                    patch.object(self.image, 'manifest_digest') as probe, \
                    patch.object(self.image.subprocess, 'run') as run:
                with self.assertRaises(ValueError):
                    self.image.publish(ROOT)
                probe.assert_not_called()
                run.assert_not_called()

    def test_existing_tag_is_never_rebuilt(self):
        env = {'GITHUB_EVENT_NAME': 'push', 'GITHUB_REF': 'refs/heads/master',
               'GITHUB_REPOSITORY': 'obs-rust/obs-studio'}
        with patch.dict(os.environ, env), patch.object(self.image, 'manifest_digest', return_value='sha256:' + 'a' * 64), \
                patch.object(self.image.subprocess, 'run') as run:
            self.image.publish(ROOT)
            run.assert_not_called()

    def test_missing_tag_builds_and_pushes_the_shared_identity(self):
        image = self.image
        env = {'GITHUB_EVENT_NAME': 'push', 'GITHUB_REF': 'refs/heads/master',
               'GITHUB_REPOSITORY': 'obs-rust/obs-studio', 'GITHUB_ACTOR': 'builder',
               'GITHUB_TOKEN': 'test-only'}
        with patch.dict(os.environ, env), patch.object(image, 'manifest_digest', return_value=None), \
                patch.object(image.subprocess, 'run') as run:
            image.publish(ROOT)
            calls = run.call_args_list
            self.assertIn('--password-stdin', calls[0].args[0])
            self.assertEqual(calls[0].kwargs['input'], 'test-only')
            self.assertEqual(calls[1].args[0], ['docker', 'build', '--platform', 'linux/amd64',
                '--tag', image.identity(ROOT), '--file', str(ROOT / image.DOCKERFILE), str(ROOT / image.CONTEXT)])
            self.assertEqual(calls[2].args[0], ['docker', 'push', image.identity(ROOT)])

    def test_probe_error_does_not_overwrite_an_unknown_tag(self):
        env = {'GITHUB_EVENT_NAME': 'push', 'GITHUB_REF': 'refs/heads/master',
               'GITHUB_REPOSITORY': 'obs-rust/obs-studio'}
        with patch.dict(os.environ, env), patch.object(self.image, 'manifest_digest', side_effect=TimeoutError()), \
                patch.object(self.image.subprocess, 'run') as run:
            with self.assertRaises(TimeoutError):
                self.image.publish(ROOT)
            run.assert_not_called()

    def test_verify_fails_loudly_when_the_package_is_private(self):
        # select_image() swallows every probe error, so a private package
        # degrades to stock Ubuntu in silence. Publishing is the one place that
        # can tell, and it must not leave the flip to memory.
        # verify() closes the response before exiting, so the self-test gate's
        # ResourceWarning check (#2235) stays clean.
        def private(code):
            def raise_(*_args, **_kwargs):
                raise urllib.error.HTTPError('https://ghcr.io', code, 'no',
                                             email.message.Message(), io.BytesIO(b''))
            return raise_

        for code in (401, 403, 404):
            with self.subTest(code=code), \
                    patch.object(self.image, 'manifest_digest', side_effect=private(code)), \
                    self.assertRaises(SystemExit) as error:
                self.image.verify(ROOT)
            message = str(error.exception)
            self.assertIn(f'HTTP {code}', message)
            self.assertIn('packages/container/tier2-linux/settings', message)
            self.assertIn('public', message)

    def test_verify_fails_when_the_tag_is_missing(self):
        with patch.object(self.image, 'manifest_digest', return_value=None), \
                self.assertRaises(SystemExit) as error:
            self.image.verify(ROOT)
        self.assertIn('missing', str(error.exception))

    def test_verify_passes_on_an_anonymous_read(self):
        digest = 'sha256:' + 'a' * 64
        with patch.object(self.image, 'manifest_digest', return_value=digest):
            self.image.verify(ROOT)


class WorkflowTest(unittest.TestCase):
    def test_builder_is_master_push_only_and_off_gate_path(self):
        all_jobs = jobs(PUSH.read_text())
        builder = all_jobs['tier2-image']
        self.assertIn("github.event_name == 'push'", builder)
        self.assertIn("github.ref == 'refs/heads/master'", builder)
        self.assertIn("github.repository == 'obs-rust/obs-studio'", builder)
        self.assertIn('packages: write', builder)
        self.assertIn('uv run --no-project ci/tier2_image.py publish', builder)
        # A private package is invisible to the selector; the publish job must
        # say so rather than let every Tier 2 job rebuild from stock Ubuntu.
        self.assertIn('uv run --no-project ci/tier2_image.py verify', builder)
        self.assertLess(builder.index('tier2_image.py publish'),
                        builder.index('tier2_image.py verify'))
        self.assertEqual(needs(builder), [])
        for name in all_jobs:
            if name != 'tier2-image':
                self.assertNotIn('tier2-image', upstream(name, all_jobs))
        for action in re.findall(r'uses: ([^\s]+)', builder):
            self.assertRegex(action, r'@[0-9a-f]{40}$')

    def test_selector_wires_build_and_both_runtime_modes(self):
        all_jobs = jobs(BUILD.read_text())
        selector = all_jobs['check-event']
        self.assertIn('tier2_image: ${{ steps.tier2-image.outputs.image }}', selector)
        self.assertIn('MQ_RUNNER_LABELS: ${{ inputs.mq-heavy-labels }}', selector)
        self.assertIn('uv run --no-project ci/tier2_image.py select', selector)
        build, tests = all_jobs['rust-linux-tier2-build'], all_jobs['rust-linux-tier2']
        self.assertIn('image: ${{ needs.check-event.outputs.tier2_image }}', build)
        self.assertIn('TIER2_PREBAKED: ${{ needs.check-event.outputs.tier2_prebaked }}', build)
        self.assertIn('--prebaked-image', tests)
        self.assertIn("needs.check-event.outputs.tier2_prebaked == 'true'", tests)
        for block in (build, tests):
            restore = block.split('id: apt-cache', 1)[1].split('with:', 1)[0]
            self.assertIn("needs.check-event.outputs.tier2_prebaked != 'true'", restore)

    def test_dockerfile_bootstrap_matches_workflow_and_has_no_user_change(self):
        image = load()
        dockerfile = (ROOT / image.DOCKERFILE).read_text()
        self.assertIn('FROM ' + image.base_image(ROOT), dockerfile)
        self.assertNotRegex(dockerfile, r'(?m)^(USER|ENV|ENTRYPOINT|CMD|WORKDIR) ')
        self.assertIn('--no-install-recommends', dockerfile)
        self.assertIn('/var/lib/apt/lists/*', dockerfile)
        bootstrap = (ROOT / image.BOOTSTRAP).read_text().split()
        build = jobs(BUILD.read_text())['rust-linux-tier2-build']
        installs = re.findall(r'apt-get install[^\n]*--fix-missing ([^\n]+)', build)
        self.assertEqual(len(installs), 2)
        for packages in installs:
            self.assertEqual(packages.split(' &&', 1)[0].split(), bootstrap)
        self.assertIn('apt-packages.txt', dockerfile)

    def test_workflows_parse_and_all_needs_and_output_references_resolve(self):
        for file in (BUILD, PUSH):
            text = file.read_text()
            self.assertEqual(duplicate_keys(text), [])
            self.assertEqual(truncated_scalars(text), [])
            all_jobs = jobs(text)
            for name, block in all_jobs.items():
                for dependency in needs(block):
                    self.assertIn(dependency, all_jobs, name)
                for dependency, output in re.findall(r'needs\.([\w-]+)\.outputs\.([\w-]+)', block):
                    self.assertIn(dependency, needs(block), name)
                    self.assertRegex(all_jobs[dependency], rf'(?m)^      {re.escape(output)}:')


if __name__ == '__main__':
    unittest.main()
