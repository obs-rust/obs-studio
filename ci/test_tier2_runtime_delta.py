"""A prepared Tier 2 runtime survives small Ubuntu updates (#1592, #2072).

Usage:
    uv run python -m unittest ci/test_tier2_runtime_delta.py

The prepared image matched the closure exactly or was not used. One zlib
security update (1ubuntu3.1 -> 1ubuntu3.2, one of 224 packages) sent every
Tier 2 test job to the stock path, 75 s of apt instead of 20-30 s
(2026-10-24). A closure that differs in a few packages now loads the prepared
image and installs exactly those versions on top; the installed inventory is
still verified against the whole closure. A larger difference still falls back
to stock.
"""
import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import urllib.parse

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / 'rust/tools/linux-validate'


def load(name: str, file: str):
    spec = importlib.util.spec_from_file_location(name, TOOLS / file)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


selection = load('runtime_selection', 'runtime-selection.py')
runtime = load('ci_runtime', 'ci-runtime.py')

STOCK = (
    'Types: deb\nURIs: http://archive.ubuntu.com/ubuntu/\nSuites: resolute resolute-updates\n'
    'Components: main universe\n\n'
    'Types: deb\nURIs: http://security.ubuntu.com/ubuntu/\nSuites: resolute-security\n'
    'Components: main universe\n')
OLD = '1:1.3.dfsg+really1.3.1-1ubuntu3.1'
NEW = '1:1.3.dfsg+really1.3.1-1ubuntu3.2'
# One second before pipewire-pulse 1.6.2-1ubuntu1.2 left resolute-updates (#5578).
SNAPSHOT = '20261124T110707Z'


class DeltaTest(unittest.TestCase):
    def test_delta_lists_only_missing_or_changed_packages(self):
        requirements = f'a=1\nzlib1g={NEW}\nc=3\n'
        inventory = f'a:amd64\t1\nzlib1g:amd64\t{OLD}\nb:amd64\t2\n'
        self.assertEqual(selection.delta(requirements, inventory), [f'c:amd64=3', f'zlib1g:amd64={NEW}'])

    def test_matching_closure_has_no_delta(self):
        self.assertEqual(selection.delta('a=1\n', 'a:amd64\t1\n'), [])


class AptInstallTest(unittest.TestCase):
    packages = ['libc6:amd64=2.43-2ubuntu2.7', f'zlib1g:amd64={NEW}']
    prefix = ('docker', 'exec', 'container', 'timeout', '--kill-after=10s', '180s',
              'env', 'LC_ALL=C', 'apt-get', '-o', 'Acquire::Retries=3',
              '-o', 'Acquire::http::Timeout=15', '-o', 'Acquire::https::Timeout=15')

    def install(self, failures=(), host_list=None, archives=None):
        calls = []
        errors = iter(failures)
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        host = Path(tmp.name) / 'apt-mirrors.txt'
        if host_list is not None:
            host.write_text(host_list)

        def command(*argv, **kwargs):
            calls.append(argv)
            if 'install' in argv or 'update' in argv:
                error = next(errors, None)
                if error:
                    raise subprocess.CalledProcessError(error[0], argv, output=error[1])
            return ''

        with patch.object(runtime, 'command', side_effect=command), \
                patch.object(runtime, 'HOST_MIRRORS', host):
            try:
                runtime.install_packages('container', 'prepared', '', self.packages,
                                         *([archives] if archives else []))
            finally:
                self.calls = [c for c in calls if 'apt-get' in c]
                self.all_calls = [c for c in calls if 'apt-get' in c or 'dpkg' in c or self.mirror_of(c)]
                self.mirrors = [self.mirror_of(c) for c in calls if self.mirror_of(c)]

    @staticmethod
    def mirror_of(call):
        """The mirror a `use the mirror` call points the main source at."""
        if call[:3] == ('docker', 'exec', 'container') and 'sh' in call and call[-2:-1] == (runtime.SOURCES,):
            return call[-1]
        return None

    def test_delta_commands_use_native_acquire_bounds_and_timeout(self):
        self.install()
        self.assertEqual(self.calls, [(*self.prefix, 'update', '-qq'),
            (*self.prefix, 'install', '-y', '--no-install-recommends', *self.packages)])

    def test_unavailable_pin_refreshes_once_and_preserves_exact_versions(self):
        self.install([None, (100, "E: Version '2.43-2ubuntu2.7' for 'libc6:amd64' was not found")])
        self.assertEqual(self.calls, [(*self.prefix, 'update', '-qq'),
            (*self.prefix, 'install', '-y', '--no-install-recommends', *self.packages)] * 2)

    def test_unavailable_pin_after_refresh_fails_with_delta_packages(self):
        # A pin missing from the snapshot too still fails, with the apt output.
        missing = (100, "E: Version '2.43-2ubuntu2.7' for 'libc6:amd64' was not found")
        with patch.object(runtime, 'snapshot_before_supersession', return_value=SNAPSHOT), \
                self.assertRaises(RuntimeError) as error:
            self.install([None, missing, None, missing, None, missing])
        self.assertEqual(len(self.calls), 6)
        for package in self.packages:
            self.assertIn(package, str(error.exception))
        self.assertIn(missing[1], str(error.exception))

    def cached(self, *names):
        """An archive mount holding these .debs, as the restored apt cache does."""
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        for name in names:
            (Path(tmp.name) / name).write_bytes(b'')
        return Path(tmp.name)

    def test_a_pin_the_index_lost_installs_the_cached_deb_of_that_version(self):
        # Run 45405030842: the build container read a mirror that still had
        # pipewire-pulse 1.6.2-1ubuntu1.2; the test container's mirror had
        # only the phased 1.3, so the exact pin failed on every Tier 2 lane.
        # The build took that .deb from the same restored apt cache that is
        # mounted here, so install exactly it; never a newer version.
        missing = (100, f"E: Version '{NEW}' for 'zlib1g' was not found")
        archives = self.cached(f'zlib1g_{NEW.replace(":", "%3a")}_amd64.deb',
                               'zlib1g_1%3a1.3.dfsg+really1.3.1-1ubuntu3.1_amd64.deb')
        with patch.object(runtime, 'snapshot_before_supersession') as before:
            self.install([None, missing, None, missing], archives=archives)
        before.assert_not_called()
        install = (*self.prefix, 'install', '-y', '--no-install-recommends')
        self.assertEqual(self.calls, [
            (*self.prefix, 'update', '-qq'), (*install, *self.packages),
            (*self.prefix, 'update', '-qq'), (*install, *self.packages),
            (*install, self.packages[0],
             f'/var/cache/apt/archives/zlib1g_{NEW.replace(":", "%3a")}_amd64.deb')])

    def test_a_lost_pin_without_its_cached_deb_or_a_snapshot_still_fails(self):
        # Launchpad unreachable, or no supersession on record: apt's own error.
        missing = (100, f"E: Version '{NEW}' for 'zlib1g:amd64' was not found")
        archives = self.cached('zlib1g_1%3a1.3.dfsg+really1.3.1-1ubuntu3.1_amd64.deb')
        for lookup in (ValueError('no supersession'), OSError('unreachable')):
            with self.subTest(lookup=lookup), \
                    patch.object(runtime, 'snapshot_before_supersession', side_effect=lookup), \
                    self.assertRaises(RuntimeError) as error:
                self.install([None, missing, None, missing], archives=archives)
            self.assertEqual(len(self.calls), 4)
            self.assertIn(missing[1], str(error.exception))

    def test_a_lost_pin_without_its_cached_deb_installs_the_same_versions_from_the_snapshot(self):
        # #5578, master run 45400822340: Ubuntu superseded pipewire-pulse
        # 1.6.2-1ubuntu1.2 in resolute-updates at 11:07:08 UTC. A build that
        # fetched it from a mirror that had not synced yet leaves no cached
        # .deb when the archive cache is not restored. The snapshot from just
        # before the supersession has every pin, with signed indexes.
        missing = (100, "Package pipewire-pulse is not available, but is referred to by another package.\n"
                        "E: Version '1.6.2-1ubuntu1.2' for 'pipewire-pulse' was not found")
        self.packages = ['libc6:amd64=2.43-2ubuntu2.7', 'pipewire-pulse:amd64=1.6.2-1ubuntu1.2']
        archives = self.cached('pipewire-pulse_1.6.2-1ubuntu1.3_amd64.deb')
        with patch.object(runtime, 'snapshot_before_supersession', return_value=SNAPSHOT) as before:
            self.install([None, missing, None, missing], archives=archives)
        before.assert_called_once_with([('pipewire-pulse', '1.6.2-1ubuntu1.2', 'amd64')])
        install = ('install', '-y', '--no-install-recommends', *self.packages)
        self.assertEqual(self.calls, [(*self.prefix, 'update', '-qq'), (*self.prefix, *install),
                                      (*self.prefix, 'update', '-qq'), (*self.prefix, *install),
                                      (*self.prefix, '--snapshot', SNAPSHOT, 'update', '-qq'),
                                      (*self.prefix, '--snapshot', SNAPSHOT, *install)])

    def test_the_snapshot_is_one_second_before_the_earliest_supersession(self):
        # Launchpad lists one publication per architecture and pocket; the
        # last supersession on the pin's architecture is when it left the index.
        def entries(arch_times):
            return {'entries': [{'distro_arch_series_link':
                                 f'https://api.launchpad.net/1.0/ubuntu/resolute/{arch}',
                                 'date_superseded': stamp, 'date_removed': None}
                                for arch, stamp in arch_times]}
        replies = {
            'pipewire-pulse': entries([('amd64', '2026-09-20T02:58:21.906056+00:00'),
                                       ('amd64', '2026-11-24T11:07:08.225920+00:00'),
                                       ('arm64', '2026-11-23T01:00:00+00:00')]),
            'libfoo1': entries([('amd64', '2026-11-24T12:00:00.5+00:00')]),
        }
        urls = []

        def urlopen(url, timeout):
            urls.append(url)
            name = urllib.parse.parse_qs(urllib.parse.urlsplit(url).query)['binary_name'][0]
            return io.BytesIO(json.dumps(replies[name]).encode())

        with patch.object(runtime.urllib.request, 'urlopen', side_effect=urlopen):
            snapshot = runtime.snapshot_before_supersession(
                [('pipewire-pulse', '1.6.2-1ubuntu1.2', 'amd64'), ('libfoo1', '2', 'all')])
        self.assertEqual(snapshot, SNAPSHOT)
        query = urllib.parse.parse_qs(urllib.parse.urlsplit(urls[0]).query)
        self.assertEqual(query, {'ws.op': ['getPublishedBinaries'], 'binary_name': ['pipewire-pulse'],
                                 'version': ['1.6.2-1ubuntu1.2'], 'exact_match': ['true']})
        self.assertTrue(urls[0].startswith('https://api.launchpad.net/'), urls[0])

    def test_a_pin_that_was_never_superseded_has_no_snapshot(self):
        reply = {'entries': [{'distro_arch_series_link': 'https://api.launchpad.net/1.0/ubuntu/resolute/amd64',
                              'date_superseded': None, 'date_removed': None}]}
        with patch.object(runtime.urllib.request, 'urlopen',
                          return_value=io.BytesIO(json.dumps(reply).encode())), \
                self.assertRaises(ValueError):
            runtime.snapshot_before_supersession([('libc6', '2.43-2ubuntu2.7', 'amd64')])

    def test_a_cached_deb_does_not_stand_in_for_other_failures(self):
        archives = self.cached(f'zlib1g_{NEW.replace(":", "%3a")}_amd64.deb')
        with self.assertRaises(RuntimeError):
            self.install([None, (100, 'dependency conflict')], archives=archives)
        self.assertEqual(len(self.calls), 2)

    def test_other_failures_do_not_refresh(self):
        for failures, count in (([(100, 'archive unavailable')], 1),
                                ([None, (100, 'dependency conflict')], 2),
                                ([(124, 'update timed out')], 1)):
            with self.subTest(failures=failures), self.assertRaises(RuntimeError) as error:
                self.install(failures)
            self.assertEqual(len(self.calls), count)
            self.assertIn(self.packages[0], str(error.exception))

    recover = ('docker', 'exec', 'container', 'timeout', '--kill-after=10s', '180s',
               'env', 'LC_ALL=C', 'dpkg', '--configure', '-a')

    def test_a_timed_out_install_recovers_dpkg_and_retries_once(self):
        # #5504, run 45113911635 attempt 4: a slow mirror let the 180 s bound
        # stop the install (exit 124). The .debs downloaded so far stay in the
        # archive mount, so one more attempt finishes; a timeout in the unpack
        # leaves dpkg interrupted, so recover it first.
        install = (*self.prefix, 'install', '-y', '--no-install-recommends', *self.packages)
        update = (*self.prefix, 'update', '-qq')
        for code in (124, 137):
            with self.subTest(code=code):
                self.install([None, (code, 'timed out')])
                # #5510: the retry fetches the index from the next mirror.
                apt = [c for c in self.all_calls if not self.mirror_of(c)]
                self.assertEqual(apt, [update, install, self.recover, update, install])
                self.assertEqual(self.mirrors, ['http://archive.ubuntu.com/ubuntu/',
                                                'http://mirrors.edge.kernel.org/ubuntu/'])

    def test_a_second_timeout_fails_with_the_delta_packages(self):
        with self.assertRaises(RuntimeError) as error:
            self.install([None, (124, 'timed out'), None, (124, 'timed out again')])
        self.assertEqual(len(self.calls), 4)
        self.assertEqual(self.all_calls.count(self.recover), 1)
        self.assertIn('exit 124', str(error.exception))
        self.assertIn(self.packages[0], str(error.exception))

    def test_the_main_source_starts_at_the_runners_mirror(self):
        # #5510: the host's own list, as the build container's Install Git
        # reads it; the mirror is set before the first index fetch.
        aws = 'http://us-east-2.ec2.archive.ubuntu.com/ubuntu/\tpriority:1\nhttps://archive.ubuntu.com/ubuntu/\tpriority:2\n'
        self.install(host_list=aws)
        self.assertEqual(self.mirrors, ['http://us-east-2.ec2.archive.ubuntu.com/ubuntu/'])
        self.assertTrue(self.mirror_of(self.all_calls[0]), self.all_calls)
        self.install([None, (124, 'timed out')], host_list=aws)
        self.assertEqual(self.mirrors, ['http://us-east-2.ec2.archive.ubuntu.com/ubuntu/',
                                        'http://archive.ubuntu.com/ubuntu/'])

    def test_the_mirror_switch_keeps_an_unknown_sources_layout(self):
        # The switch is one shell call that edits ubuntu.sources only when it
        # has the security stanza, and never fails the job.
        with patch.object(runtime, 'command', return_value='') as command:
            runtime.use_mirror('container', 'http://x.example/ubuntu/')
        argv = command.call_args.args
        self.assertEqual(argv[:4], ('docker', 'exec', 'container', 'sh'))
        self.assertEqual(argv[-2:], (runtime.SOURCES, 'http://x.example/ubuntu/'))
        with tempfile.TemporaryDirectory() as tmp:
            for layout, changed in ((STOCK, True), (STOCK.replace('security.ubuntu.com', 'ports.example'), False)):
                path = Path(tmp) / 'ubuntu.sources'
                path.write_text(layout)
                result = subprocess.run(['sh', *argv[4:-2], str(path), 'http://x.example/ubuntu/'],
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr)
                text = path.read_text()
                self.assertEqual('URIs: http://x.example/ubuntu/' in text, changed, text)
                self.assertEqual(text.count('URIs: http://x.example/ubuntu/'), int(changed))

    def test_other_install_failures_do_not_recover_or_retry(self):
        with self.assertRaises(RuntimeError):
            self.install([None, (100, 'dependency conflict')])
        self.assertNotIn(self.recover, self.all_calls)
        self.assertEqual(len(self.calls), 2)

    def test_refresh_failure_stops_before_second_install(self):
        with self.assertRaises(RuntimeError):
            self.install([None, (100, "E: Version '2' for 'libc6' was not found"),
                          (124, 'refresh timed out')])
        self.assertEqual(len(self.calls), 3)

    def test_empty_delta_runs_no_commands(self):
        with patch.object(runtime, 'command') as command:
            runtime.install_packages('container', 'prepared', 'a=1\n', [])
        command.assert_not_called()

    def test_apt_diagnostics_capture_stderr(self):
        with patch.object(runtime.subprocess, 'check_output', return_value='apt error\n') as output:
            self.assertEqual(runtime.command('apt-get', 'update', capture_stderr=True), 'apt error')
        output.assert_called_once_with(('apt-get', 'update'), text=True, stderr=subprocess.STDOUT)


class ExecuteTest(unittest.TestCase):
    def descriptor(self, inventory: str) -> dict:
        return {'base': runtime.BASE, 'platform': 'linux/amd64', 'inventory': inventory,
                'image_id': 'sha256:' + 'a' * 64, 'compressed_bytes': 4,
                'archive_sha256': hashlib.sha256(b'good').hexdigest(),
                'url': 'https://github.com/obs-rust/obs-studio-libs/releases/download/runtime-1/image.tar.zst'}

    def run_execute(self, requirements: str, image_inventory: str, installed: str):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / 'descriptor.json').write_text(json.dumps(self.descriptor(image_inventory)))
            (root / 'packages.txt').write_text(requirements)
            args = argparse.Namespace(packages=root / 'packages.txt', descriptor=root / 'descriptor.json',
                                      state=root / 'state', cost=root / 'cost', source=root, build=root,
                                      uv=root / 'uv', archives=root, mode='ON', part='suite')
            calls = []

            def command(*argv, **_kwargs):
                calls.append(argv)
                if argv[:2] == ('docker', 'create'):
                    return 'b' * 64
                if 'dpkg-query' in argv:
                    return installed
                return ''

            with patch.object(runtime, 'download_prepared', return_value='sha256:' + 'a' * 64), \
                    patch.object(runtime, 'command', side_effect=command), \
                    patch.object(runtime.subprocess, 'run'):
                runtime.execute(args)
            report = json.loads((root / 'state/runtime-measurement.json').read_text())
            return calls, report

    def test_small_drift_installs_the_delta_on_the_prepared_image(self):
        calls, report = self.run_execute(f'a=1\nzlib1g={NEW}\n', f'a\t1\nzlib1g\t{OLD}\n',
                                         f'a\t1\nzlib1g\t{NEW}\n')
        self.assertEqual(report['variant'], 'prepared')
        self.assertEqual(report['delta'], [f'zlib1g:amd64={NEW}'])
        installs = [c for c in calls if 'install' in c]
        self.assertEqual(len(installs), 1)
        self.assertIn(f'zlib1g:amd64={NEW}', installs[0])
        self.assertNotIn(('docker', 'pull', runtime.BASE), calls)

    def test_delta_that_does_not_install_fails(self):
        with self.assertRaises(ValueError):
            self.run_execute(f'a=1\nzlib1g={NEW}\n', f'a\t1\nzlib1g\t{OLD}\n', f'a\t1\nzlib1g\t{OLD}\n')

    def test_large_drift_falls_back_to_stock(self):
        count = runtime.MAX_DELTA + 1
        requirements = ''.join(f'p{i}=2\n' for i in range(count))
        image = ''.join(f'p{i}\t1\n' for i in range(count))
        installed = ''.join(f'p{i}\t2\n' for i in range(count))
        calls, report = self.run_execute(requirements, image, installed)
        self.assertEqual(report['variant'], 'stock')
        self.assertIn(('docker', 'pull', runtime.BASE), calls)


if __name__ == '__main__':
    unittest.main()
