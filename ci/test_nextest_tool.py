"""Pinned test-tool archives must be verified before installing executable bytes."""
import hashlib
import io
from pathlib import Path
import tarfile
import tempfile
import unittest

import importlib.util

spec = importlib.util.spec_from_file_location('nextest_tool_test_target', Path(__file__).with_name('nextest_tool.py'))
nextest_tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(nextest_tool)


class NextestToolTest(unittest.TestCase):
    def archive(self, root, kind=tarfile.REGTYPE, name='cargo-nextest'):
        path = root / 'tool.tar.gz'
        with tarfile.open(path, 'w:gz') as archive:
            member = tarfile.TarInfo(name)
            member.type = kind
            member.linkname = '/outside'
            data = b'verified executable bytes'
            member.size = len(data) if kind == tarfile.REGTYPE else 0
            archive.addfile(member, io.BytesIO(data) if member.size else None)
        return path, hashlib.sha256(path.read_bytes()).hexdigest()

    def test_validated_regular_binary_installs_without_extracting_other_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive, digest = self.archive(root)
            output = root / 'bin' / 'cargo-nextest'
            nextest_tool.unpack_checked(archive, output, digest)
            self.assertEqual(output.read_bytes(), b'verified executable bytes')
            self.assertTrue(output.stat().st_mode & 0o100)

    def test_archive_checksum_mismatch_cannot_replace_an_existing_binary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive, _ = self.archive(root)
            output = root / 'cargo-nextest'
            output.write_bytes(b'old')
            with self.assertRaisesRegex(ValueError, 'checksum'):
                nextest_tool.unpack_checked(archive, output, '0' * 64)
            self.assertEqual(output.read_bytes(), b'old')

    def test_link_entries_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive, digest = self.archive(root, tarfile.SYMTYPE)
            with self.assertRaises(ValueError):
                nextest_tool.unpack_checked(archive, root / 'cargo-nextest', digest)

    def test_unsupported_hosts_do_not_download_or_change_the_cargo_runner(self):
        from unittest import mock
        with mock.patch.object(nextest_tool.host, 'is_linux', return_value=False):
            self.assertIsNone(nextest_tool.ensure_nextest())


class NextestDownloadResilienceTest(unittest.TestCase):
    """A CDN blip must not redden the gate; a wrong byte or a 404 still must."""

    def setUp(self):
        from unittest import mock
        self.mock = mock
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        path, digest = NextestToolTest.archive(None, root)
        self.payload = path.read_bytes()
        self.digest = digest
        patches = [
            mock.patch.object(nextest_tool.host, 'is_linux', return_value=True),
            mock.patch.object(nextest_tool.platform, 'machine', return_value='x86_64'),
            mock.patch.dict(nextest_tool.ARCHIVES, {'x86_64': ('x86_64-unknown-linux-musl', digest)}),
            mock.patch.dict(nextest_tool.os.environ, {'XDG_CACHE_HOME': str(root / 'cache'),
                                                     'CARGO_HOME': str(root / 'cargo'), 'PATH': ''}),
            mock.patch.object(nextest_tool.time, 'sleep', create=True),
        ]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)
        # These cases cover the anonymous download a local run makes; a CI
        # job's token must not switch them to the API path (#4209).
        for name in ('GITHUB_TOKEN', 'GH_TOKEN'):
            nextest_tool.os.environ.pop(name, None)
        self.addCleanup(self.tmp.cleanup)

    def transport(self, *outcomes):
        import urllib.error
        calls = []

        def fake(url, timeout):
            calls.append(url)
            outcome = outcomes[min(len(calls), len(outcomes)) - 1]
            if isinstance(outcome, int):
                raise urllib.error.HTTPError(url, outcome, 'status', {}, None)
            if isinstance(outcome, BaseException):
                raise outcome
            return io.BytesIO(outcome)
        return self.mock.patch.object(nextest_tool, 'urlopen', side_effect=fake), calls

    def test_transient_timeouts_and_5xx_are_retried_until_verified(self):
        patch, calls = self.transport(TimeoutError('read timed out'), 504, ConnectionResetError(), self.payload)
        with patch:
            output = nextest_tool.ensure_nextest()
        self.assertEqual(len(calls), 4)
        self.assertEqual(output.read_bytes(), b'verified executable bytes')

    def test_not_found_fails_immediately_without_retry(self):
        patch, calls = self.transport(404, self.payload)
        with patch, self.assertRaisesRegex(RuntimeError, 'HTTP 404'):
            nextest_tool.ensure_nextest()
        self.assertEqual(len(calls), 1)

    def test_digest_mismatch_is_fatal_and_not_retried(self):
        patch, calls = self.transport(b'tampered bytes', self.payload)
        with patch, self.assertRaisesRegex(ValueError, 'checksum'):
            nextest_tool.ensure_nextest()
        self.assertEqual(len(calls), 1)
        cache = Path(self.tmp.name) / 'cache' / 'obs-rust' / 'nextest' / nextest_tool.VERSION
        self.assertEqual(list(cache.rglob('nextest.tar.gz')), [])

    def test_exhausted_budget_names_url_attempts_and_recovery(self):
        patch, calls = self.transport(TimeoutError('read timed out'))
        with patch, self.assertRaises(RuntimeError) as raised:
            nextest_tool.ensure_nextest()
        message = str(raised.exception)
        self.assertEqual(len(calls), nextest_tool.ATTEMPTS)
        self.assertIn('https://github.com/nextest-rs/nextest/releases/download/', message)
        self.assertIn(f'{nextest_tool.ATTEMPTS} attempts', message)
        self.assertIn('read timed out', message)
        self.assertIn('cargo install cargo-nextest', message)

    def test_installed_pinned_binary_skips_the_network(self):
        binary = Path(self.tmp.name) / 'cargo' / 'bin' / 'cargo-nextest'
        binary.parent.mkdir(parents=True)
        binary.write_text(f'#!/bin/sh\necho "cargo-nextest-nextest {nextest_tool.VERSION}"\n')
        binary.chmod(0o755)
        patch, calls = self.transport(TimeoutError('network must not be used'))
        with patch:
            self.assertEqual(nextest_tool.ensure_nextest(), binary)
        self.assertEqual(calls, [])

    def test_installed_other_version_is_ignored(self):
        binary = Path(self.tmp.name) / 'cargo' / 'bin' / 'cargo-nextest'
        binary.parent.mkdir(parents=True)
        binary.write_text('#!/bin/sh\necho "cargo-nextest-nextest 0.9.1"\n')
        binary.chmod(0o755)
        patch, calls = self.transport(self.payload)
        with patch:
            self.assertNotEqual(nextest_tool.ensure_nextest(), binary)
        self.assertEqual(len(calls), 1)



class NextestAuthenticatedDownloadTest(unittest.TestCase):
    """With a workflow token, nextest comes through the authenticated API (#4209).

    GitHub can answer anonymous release downloads with 504 for hours while
    authenticated requests work, so a CI job must not depend on the
    anonymous URL when it has a token.
    """

    TOKEN = 'ghs_test-token-value'
    NAME = f'cargo-nextest-{nextest_tool.VERSION}-x86_64-unknown-linux-musl.tar.gz'
    API = 'https://api.github.com/repos/nextest-rs/nextest/releases'
    ASSET = API + '/assets/4209'

    def setUp(self):
        NextestDownloadResilienceTest.setUp(self)
        nextest_tool.os.environ['GITHUB_TOKEN'] = self.TOKEN
        # Anonymous release downloads are down, as in the 2026-11-09 incident.
        self.anonymous = []

        def blocked(url, timeout):
            import urllib.error
            self.anonymous.append(url)
            raise urllib.error.HTTPError(url, 504, 'Gateway Timeout', {}, None)
        patch = self.mock.patch.object(nextest_tool, 'urlopen', side_effect=blocked)
        patch.start()
        self.addCleanup(patch.stop)

    def api(self, *asset_outcomes):
        import json
        import urllib.error
        requests = []
        served = []

        def fake(request, timeout):
            requests.append(request)
            if request.full_url == f'{self.API}/tags/cargo-nextest-{nextest_tool.VERSION}':
                body = {'assets': [{'name': 'other.tar.gz', 'url': self.API + '/assets/1'},
                                   {'name': self.NAME, 'url': self.ASSET}]}
                return io.BytesIO(json.dumps(body).encode())
            if request.full_url == self.ASSET:
                served.append(request)
                outcome = asset_outcomes[min(len(served), len(asset_outcomes)) - 1]
                if isinstance(outcome, int):
                    raise urllib.error.HTTPError(request.full_url, outcome, 'status', {}, None)
                return io.BytesIO(outcome)
            raise AssertionError(f'unexpected request {request.full_url}')
        # create=True: the RED run on master shows the behaviour (the anonymous 504),
        # not a missing attribute.
        return self.mock.patch.object(nextest_tool, 'authenticated_open', side_effect=fake, create=True), requests

    def test_token_fetches_the_asset_through_the_api_not_the_anonymous_url(self):
        patch, requests = self.api(self.payload)
        with patch:
            output = nextest_tool.ensure_nextest()
        self.assertEqual(output.read_bytes(), b'verified executable bytes')
        self.assertEqual(self.anonymous, [])
        self.assertEqual([r.full_url for r in requests],
                         [f'{self.API}/tags/cargo-nextest-{nextest_tool.VERSION}', self.ASSET])
        for request in requests:
            self.assertEqual(request.get_header('Authorization'), f'Bearer {self.TOKEN}')
            self.assertNotIn(self.TOKEN, request.full_url)
        self.assertEqual(requests[1].get_header('Accept'), 'application/octet-stream')

    def test_gh_token_is_used_when_github_token_is_absent(self):
        del nextest_tool.os.environ['GITHUB_TOKEN']
        nextest_tool.os.environ['GH_TOKEN'] = 'gh-cli-token'
        patch, requests = self.api(self.payload)
        with patch:
            nextest_tool.ensure_nextest()
        self.assertEqual(self.anonymous, [])
        self.assertEqual(requests[-1].get_header('Authorization'), 'Bearer gh-cli-token')

    def test_api_5xx_is_retried_and_the_digest_still_checked(self):
        patch, requests = self.api(504, b'tampered bytes')
        with patch, self.assertRaisesRegex(ValueError, 'checksum'):
            nextest_tool.ensure_nextest()
        self.assertEqual(self.anonymous, [])
        self.assertEqual(len(requests), 4)  # tag + asset (504), then tag + asset

    def test_failure_message_never_contains_the_token(self):
        patch, _ = self.api(503)
        with patch, self.assertRaises(RuntimeError) as raised:
            nextest_tool.ensure_nextest()
        self.assertNotIn(self.TOKEN, str(raised.exception))
        self.assertIn('HTTP 503', str(raised.exception))

    def test_without_a_token_the_api_is_not_used(self):
        del nextest_tool.os.environ['GITHUB_TOKEN']
        patch, requests = self.api(self.payload)
        with patch, self.assertRaisesRegex(RuntimeError, 'HTTP 504'):
            nextest_tool.ensure_nextest()
        self.assertEqual(requests, [])
        self.assertEqual(len(self.anonymous), nextest_tool.ATTEMPTS)

    def test_redirect_to_another_host_drops_the_authorization_header(self):
        from urllib.request import Request
        handler = nextest_tool.SameHostAuthRedirect()
        request = Request(self.ASSET, headers={'Authorization': f'Bearer {self.TOKEN}',
                                               'Accept': 'application/octet-stream'})
        signed = 'https://release-assets.githubusercontent.com/github-production-release-asset/1?sig=x'
        moved = handler.redirect_request(request, None, 302, 'Found', {}, signed)
        self.assertIsNone(moved.get_header('Authorization'))
        same = handler.redirect_request(request, None, 302, 'Found', {}, self.API + '/assets/4210')
        self.assertEqual(same.get_header('Authorization'), f'Bearer {self.TOKEN}')


if __name__ == '__main__':
    unittest.main()
