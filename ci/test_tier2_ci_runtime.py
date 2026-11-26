"""Runtime delivery must preserve integrity and complete-suite execution paths."""
import hashlib
import importlib.util
import argparse
import json
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import ANY, call, patch
import urllib.error

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location('ci_runtime', ROOT / 'rust/tools/linux-validate/ci-runtime.py')
runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime)


class RuntimeTest(unittest.TestCase):
    def test_cpu_protocol_marker_is_bound_to_the_current_socket(self):
        with tempfile.TemporaryDirectory() as directory, runtime.socket.socket(runtime.socket.AF_UNIX) as listener:
            socket_path = Path(directory) / "lease.sock"
            listener.bind(str(socket_path))
            marker = socket_path.with_name("cpu-plan-v1")
            self.assertFalse(runtime.supports_cpu_plan(socket_path))
            marker.write_text(str(socket_path.stat().st_ino + 1))
            self.assertFalse(runtime.supports_cpu_plan(socket_path))
            marker.write_text(str(socket_path.stat().st_ino))
            self.assertTrue(runtime.supports_cpu_plan(socket_path))

    def test_cpu_reservation_partitions_the_complete_suite(self):
        with patch.object(runtime.os, "sched_getaffinity", return_value=set(range(16))):
            docker, environment = runtime.cpu_plan("all", "0-15", "8-15")
            self.assertEqual(docker, ["--cpuset-cpus", "0-15", "--cpus", "16"])
            self.assertIn("OBS_TIER2_SERIAL_CPUS=8-15", environment)
            self.assertIn("OBS_CTEST_JOBS=12", environment)
            self.assertTrue(any(value.endswith("/isolation-schedule.json") for value in environment))
            for part, isolated in (("suite", "8-15"), ("all", "8-14"), ("all", "14-17")):
                with self.subTest(part=part, isolated=isolated), self.assertRaises(ValueError):
                    runtime.cpu_plan(part, "0-15", isolated)

    def test_cpu_plan_defaults_to_complete_ctest(self):
        docker, environment = runtime.cpu_plan("all", "", "")
        self.assertEqual(docker, ["--cpus", "4"])
        self.assertIn("OBS_TIER2_SERIAL_CPUS=", environment)
        self.assertFalse(any(value.startswith("OBS_TIER2_ISOLATED_SCHEDULE=") for value in environment))

    def test_shared_artifact_lends_cpu_plan_only_to_complete_suites(self):
        grant = {"parent": "obs_tier2_a.slice", "reserved": "0-15", "isolated": "8-15"}
        with patch.object(runtime.os, "sched_getaffinity", return_value=set(range(16, 32))):
            docker, environment = runtime.cpu_plan("all", "0-15", "8-15", grant["parent"])
            self.assertEqual(docker[:2], ["--cgroup-parent", grant["parent"]])
            self.assertIn("OBS_TIER2_SERIAL_CPUS=8-15", environment)
        args = argparse.Namespace(part="all")
        runtime.use_cpu_grant(args, {**grant, "parent": "system.slice"})
        self.assertFalse(hasattr(args, "cpu_parent"))
        runtime.use_cpu_grant(args, grant)
        self.assertEqual((args.cpu_parent, args.reserved_cpus, args.isolated_cpus),
                         (grant["parent"], grant["reserved"], grant["isolated"]))
        for part in ("suite", "serial-1-of-2"):
            args = argparse.Namespace(part=part)
            runtime.use_cpu_grant(args, grant)
            self.assertFalse(hasattr(args, "cpu_parent"))

    def test_shared_mount_error_falls_back_before_docker(self):
        with patch.dict(os.environ, {"OBS_TIER2_ARTIFACT_SOCKET": "/nonexistent/lease.sock"}), \
                patch.object(runtime, "execute") as execute:
            self.assertEqual(runtime.execute_shared(argparse.Namespace()), 75)
            execute.assert_not_called()

    def test_shared_mount_remains_leased_through_runtime(self):
        path = "/run/obs-tier2-artifacts/sessions/tier2-job-example/merged"
        response = json.dumps({"path": path, "packages": "libc6=1\n"}).encode() + b"\n"
        with patch.object(runtime.socket, 'socket') as connect, patch.object(runtime, 'execute') as execute:
            lease = connect.return_value.__enter__.return_value
            lease.makefile.return_value = io.BytesIO(response)

            def run(args):
                self.assertEqual(args.build, Path(path))
                self.assertEqual(args.packages.read_text(), "libc6=1\n")
                connect.return_value.__exit__.assert_not_called()

            execute.side_effect = run
            self.assertEqual(runtime.execute_shared(argparse.Namespace()), 0)
            lease.assert_has_calls([call.settimeout(5), call.connect(ANY), call.settimeout(600)])
            execute.assert_called_once()
            connect.return_value.__exit__.assert_called_once()

    def descriptor(self):
        return {'base': runtime.BASE, 'platform': 'linux/amd64', 'inventory': 'a\t1\n',
                'image_id': 'sha256:' + 'a' * 64, 'compressed_bytes': 4,
                'archive_sha256': hashlib.sha256(b'good').hexdigest(),
                'url': 'https://github.com/obs-rust/obs-studio-libs/releases/download/runtime-1/image.tar.zst'}

    def test_changed_closure_falls_back_without_download_or_docker(self):
        with patch.object(runtime.urllib.request, 'urlopen') as download, patch.object(runtime, 'command') as command:
            # A few changed packages install on top (ci/test_tier2_runtime_delta.py).
            changed = ''.join(f'p{i}=1\n' for i in range(runtime.MAX_DELTA + 1))
            self.assertIsNone(runtime.prepared_image(self.descriptor(), changed, Path('/unused')))
            download.assert_not_called()
            command.assert_not_called()

    def test_unavailable_asset_falls_back_without_docker_load(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(runtime.urllib.request, 'urlopen',
                side_effect=urllib.error.URLError('offline')), patch.object(runtime, 'command',
                side_effect=subprocess.CalledProcessError(1, 'inspect')) as command:
            self.assertIsNone(runtime.prepared_image(self.descriptor(), 'a=1\n', Path(tmp)))
            command.assert_called_once_with('docker', 'image', 'inspect', '--format', '{{.Id}}',
                                            self.descriptor()['image_id'])

    def test_corrupt_archive_fails_before_docker_load(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root / 'runtime.tar.zst'
            archive.write_bytes(b'evil')
            with self.assertRaisesRegex(ValueError, 'checksum'):
                runtime.verify_archive(archive, self.descriptor())

    def test_mounts_preserve_original_paths_and_writable_timing_history(self):
        paths = [Path('/host') / name for name in ('source', 'build', 'cost', 'uv', 'apt')]
        mounts = runtime.mounts(*paths)
        self.assertIn('type=bind,src=/host/source,dst=/__w/obs-studio/obs-studio,readonly', mounts)
        self.assertIn('type=bind,src=/host/build,dst=/__w/_temp/tier2', mounts)
        self.assertIn('type=bind,src=/host/cost,dst=/__w/_temp/tier2-cost', mounts)

    def test_step_summary_reaches_hosted_and_dell_runtime_variants(self):
        for variant in ('stock', 'prepared', 'prebaked'):
            for mode in ('OFF', 'ON'):
                with self.subTest(variant=variant, mode=mode), tempfile.TemporaryDirectory() as tmp:
                    root = Path(tmp)
                    (root / 'descriptor.json').write_text(json.dumps(self.descriptor()))
                    (root / 'packages.txt').write_text('a=1\n')
                    summary = root / 'summary.md'
                    args = argparse.Namespace(packages=root / 'packages.txt', descriptor=root / 'descriptor.json',
                        state=root / 'state', cost=root / 'cost', source=root, build=root,
                        uv=root / 'uv', archives=root, mode=mode, part='all',
                        prebaked_image=('ghcr.io/obs-rust/tier2-linux@sha256:' + 'c' * 64
                                        if variant == 'prebaked' else ''))

                    def command(*argv):
                        if argv[:2] == ('docker', 'create'):
                            return 'b' * 64
                        return 'a\t1' if 'dpkg-query' in argv else ''

                    with patch.dict(os.environ, {'GITHUB_STEP_SUMMARY': str(summary)}), \
                            patch.object(runtime, 'prepared_image', return_value=(
                                self.descriptor()['image_id'] if variant == 'prepared' else None)), \
                            patch.object(runtime, 'command', side_effect=command) as commands, \
                            patch.object(runtime.subprocess, 'run') as run:
                        runtime.execute(args)
                        create = next(call.args for call in commands.call_args_list
                                      if call.args[:2] == ('docker', 'create'))
                        self.assertIn(f'type=bind,src={summary},dst=/tier2-step-summary', create)
                        self.assertIn('GITHUB_STEP_SUMMARY=/tier2-step-summary', create)
                        self.assertTrue(summary.is_file())
                        self.assertIn(f'OBS_TIER2_MODES={mode}', run.call_args.args[0])

    def test_complete_suite_failure_propagates_and_container_stops(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            descriptor = self.descriptor()
            (root / 'descriptor.json').write_text(json.dumps(descriptor))
            (root / 'packages.txt').write_text('a=1\n')
            args = argparse.Namespace(packages=root / 'packages.txt', descriptor=root / 'descriptor.json',
                                      state=root / 'state', cost=root / 'cost', source=root,
                                      build=root, uv=root / 'uv', archives=root, mode='ON', part='suite')
            ident = 'b' * 64

            def command(*argv):
                if argv[:2] == ('docker', 'create'):
                    return ident
                if 'dpkg-query' in argv:
                    return 'a\t1'
                return ''

            with patch.object(runtime, 'prepared_image', return_value=descriptor['image_id']), \
                    patch.object(runtime, 'command', side_effect=command) as commands, \
                    patch.object(runtime.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'test')) as run:
                with self.assertRaises(subprocess.CalledProcessError):
                    runtime.execute(args)
                invocation = run.call_args.args[0]
                self.assertIn('OBS_TIER2_MODES=ON', invocation)
                self.assertIn('OBS_TIER2_STAGE=test', invocation)
                self.assertIn('OBS_CTEST_JOBS=8', invocation)
                self.assertIn('OBS_CTEST_PART=suite', invocation)
                self.assertIn('OBS_TIER2_COST_DIR=' + runtime.COST, invocation)
                self.assertEqual(commands.call_args.args, ('docker', 'stop', '--timeout', '1', ident))
                report = json.loads((root / 'state/runtime-measurement.json').read_text())
                self.assertEqual(report['variant'], 'prepared')


class RuntimeReuseTest(unittest.TestCase):
    descriptor = RuntimeTest.descriptor

    def setUp(self):
        env = patch.dict(os.environ, {}, clear=True)
        env.start()
        self.addCleanup(env.stop)
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        marker = patch.object(runtime, 'ASSET_CACHE_MARKER', Path(temporary.name) / 'absent')
        marker.start()
        self.addCleanup(marker.stop)

    def enable_cache(self, cache):
        marker = cache.parent / '.persistent-cache'
        marker.touch()
        for name, value in (('ASSET_CACHE_DIR', cache), ('ASSET_CACHE_MARKER', marker)):
            setting = patch.object(runtime, name, value)
            setting.start()
            self.addCleanup(setting.stop)

    def deliver(self, root, first_inspect='sha256:' + 'b' * 64, network=None):
        descriptor = self.descriptor()
        calls = []

        def command(*args):
            calls.append(args)
            if len(calls) == 1:
                if isinstance(first_inspect, Exception):
                    raise first_inspect
                return first_inspect
            return descriptor['image_id']

        with patch.object(runtime, 'command', side_effect=command), \
                patch.object(runtime.urllib.request, 'urlopen',
                             side_effect=network or (lambda *args, **kwargs: io.BytesIO(b'good'))) as download:
            self.assertEqual(runtime.download_prepared(descriptor, root), descriptor['image_id'])
        return calls, download

    def test_matching_image_skips_network_verification_and_load(self):
        with tempfile.TemporaryDirectory() as tmp, patch.object(runtime, 'verify_archive') as verify:
            calls, download = self.deliver(Path(tmp), self.descriptor()['image_id'])
            self.assertEqual(calls, [('docker', 'image', 'inspect', '--format', '{{.Id}}',
                                     self.descriptor()['image_id'])])
            download.assert_not_called()
            verify.assert_not_called()
            self.assertEqual(list(Path(tmp).iterdir()), [])

    def test_mismatching_or_missing_image_downloads_and_checks_loaded_identity(self):
        for first in ('sha256:' + 'b' * 64, subprocess.CalledProcessError(1, 'inspect')):
            with self.subTest(first=first), tempfile.TemporaryDirectory() as tmp:
                calls, download = self.deliver(Path(tmp), first)
                download.assert_called_once_with(self.descriptor()['url'], timeout=60)
                self.assertEqual(calls[1][0], 'zstd')
                self.assertEqual(calls[2][:3], ('docker', 'image', 'load'))
                self.assertEqual(calls[-1], calls[0])

    def test_verified_cache_avoids_network_and_corrupt_cache_downloads_without_replacement(self):
        for contents in (b'good', b'evil'):
            with self.subTest(contents=contents), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                cache = root / 'cache'
                cache.mkdir()
                self.enable_cache(cache)
                cached = cache / self.descriptor()['archive_sha256']
                cached.write_bytes(contents)
                calls, download = self.deliver(root)
                self.assertEqual(download.call_count, int(contents != b'good'))
                self.assertEqual((root / 'runtime.tar.zst').read_bytes(), b'good')
                self.assertEqual(cached.read_bytes(), contents)
                self.assertEqual(list(cache.iterdir()), [cached])
                self.assertEqual(calls[2][:3], ('docker', 'image', 'load'))

    def test_cold_cache_publishes_atomically(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache = root / 'new-cache'
            cache.mkdir()
            self.enable_cache(cache)
            replace = os.replace

            def publish(source, destination):
                self.assertEqual(Path(source).read_bytes(), b'good')
                self.assertFalse(Path(destination).exists())
                self.assertEqual(Path(source).parent, cache)
                self.assertTrue(Path(source).name.startswith('.' + self.descriptor()['archive_sha256']))
                self.assertEqual(Path(source).suffix, '.part')
                self.assertEqual(Path(source).stat().st_mode & 0o777, 0o644)
                replace(source, destination)

            with patch.object(os, 'replace', side_effect=publish) as atomic:
                _, download = self.deliver(root)
            download.assert_called_once()
            atomic.assert_called_once()
            self.assertEqual(len(list(cache.iterdir())), 1)

    def test_cache_io_errors_fall_back_to_download(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache = root / 'not-a-directory'
            cache.write_bytes(b'file')
            self.enable_cache(cache)
            _, download = self.deliver(root)
            download.assert_called_once()
            self.assertEqual(cache.read_bytes(), b'file')

    def test_cache_read_and_publish_errors_do_not_block_download(self):
        for operation in ('copyfile', 'chmod', 'replace'):
            with self.subTest(operation=operation), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                cache = root / 'cache'
                cache.mkdir()
                cached = cache / self.descriptor()['archive_sha256']
                if operation == 'copyfile':
                    cached.write_bytes(b'good')
                self.enable_cache(cache)
                target = runtime.shutil if operation == 'copyfile' else runtime.os
                with patch.object(target, operation, side_effect=OSError('cache unavailable')):
                    _, download = self.deliver(root)
                self.assertEqual(download.call_count, 1 if operation == 'copyfile' else 2)
                self.assertEqual((root / 'runtime.tar.zst').read_bytes(), b'good')
                self.assertEqual(list(cache.iterdir()), [cached] if operation == 'copyfile' else [])

    def test_download_corruption_and_loaded_identity_mismatch_still_fail(self):
        for corrupt in (True, False):
            with self.subTest(corrupt=corrupt), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                cache = root / 'cache'
                cache.mkdir()
                self.enable_cache(cache)
                with patch.object(runtime, 'command', return_value='wrong') as command, \
                        patch.object(runtime.urllib.request, 'urlopen',
                                     return_value=io.BytesIO(b'evil' if corrupt else b'good')):
                    with self.assertRaisesRegex(ValueError, 'checksum' if corrupt else 'identity'):
                        runtime.download_prepared(self.descriptor(), root)
                if corrupt:
                    self.assertEqual(list(cache.iterdir()), [])
                    self.assertEqual(command.call_count, 1)
                else:
                    self.assertEqual(command.call_count, 4)

    def test_dell_cache_requires_marker_and_existing_directory(self):
        for marker_exists, directory_exists in ((False, True), (True, False), (True, True)):
            with self.subTest(marker=marker_exists, directory=directory_exists), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                marker = root / '.persistent-cache'
                cache = root / 'assets'
                if marker_exists:
                    marker.touch()
                if directory_exists:
                    cache.mkdir()
                    (cache / self.descriptor()['archive_sha256']).write_bytes(b'good')
                with patch.object(runtime, 'ASSET_CACHE_DIR', cache, create=True), \
                        patch.object(runtime, 'ASSET_CACHE_MARKER', marker, create=True), \
                        patch.object(runtime.os, 'utime') as touch:
                    _, download = self.deliver(root)
                hit = marker_exists and directory_exists
                self.assertEqual(download.call_count, int(not hit))
                self.assertEqual(touch.call_count, int(hit))

    def test_dell_cache_leaves_existing_corruption_for_host_gc(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            marker = root / '.persistent-cache'
            marker.touch()
            cache = root / 'assets'
            cache.mkdir()
            cached = cache / self.descriptor()['archive_sha256']
            cached.write_bytes(b'evil')
            with patch.object(runtime, 'ASSET_CACHE_DIR', cache, create=True), \
                    patch.object(runtime, 'ASSET_CACHE_MARKER', marker, create=True):
                _, download = self.deliver(root)
            download.assert_called_once()
            self.assertEqual(cached.read_bytes(), b'evil')
            self.assertEqual((root / 'runtime.tar.zst').read_bytes(), b'good')
            self.assertEqual(list(cache.iterdir()), [cached])

    def test_cache_gc_refresh_permission_error_falls_back_to_job_download(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache = root / 'cache'
            cache.mkdir()
            (cache / self.descriptor()['archive_sha256']).write_bytes(b'good')
            self.enable_cache(cache)
            with patch.object(runtime.os, 'utime', side_effect=PermissionError('another uid')):
                _, download = self.deliver(root)
            download.assert_called_once()

    def test_environment_variables_cannot_bypass_persistent_marker(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache = root / 'cache'
            cache.mkdir()
            (cache / self.descriptor()['archive_sha256']).write_bytes(b'good')
            os.environ['OBS_ASSET_CACHE_DIR'] = str(cache)
            os.environ['OBS_RUNNER_CACHE'] = str(cache)
            _, download = self.deliver(root)
            download.assert_called_once()

    def test_network_download_starts_in_shared_part_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache = root / 'assets'
            cache.mkdir()
            self.enable_cache(cache)

            def network(*args, **kwargs):
                parts = list(cache.iterdir())
                self.assertEqual(len(parts), 1)
                self.assertTrue(parts[0].name.startswith('.' + self.descriptor()['archive_sha256'] + '.'))
                self.assertEqual(parts[0].suffix, '.part')
                self.assertFalse((root / 'runtime.tar.zst').exists())
                return io.BytesIO(b'good')

            _, download = self.deliver(root, network=network)
            download.assert_called_once()

    def test_unwritable_shared_cache_downloads_once_in_job_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cache = root / 'assets'
            cache.mkdir()
            self.enable_cache(cache)
            with patch.object(runtime.tempfile, 'NamedTemporaryFile', side_effect=PermissionError('cache read-only')):
                _, download = self.deliver(root)
            download.assert_called_once()
            self.assertEqual((root / 'runtime.tar.zst').read_bytes(), b'good')
            self.assertEqual(list(cache.iterdir()), [])

    def test_dell_cache_miss_publishes_world_readable_verified_bytes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            marker = root / '.persistent-cache'
            marker.touch()
            cache = root / 'assets'
            cache.mkdir()
            with patch.object(runtime, 'ASSET_CACHE_DIR', cache, create=True), \
                    patch.object(runtime, 'ASSET_CACHE_MARKER', marker, create=True):
                _, download = self.deliver(root)
            download.assert_called_once()
            cached = cache / self.descriptor()['archive_sha256']
            self.assertEqual(cached.read_bytes(), b'good')
            self.assertEqual(cached.stat().st_mode & 0o777, 0o644)
            self.assertEqual(list(cache.iterdir()), [cached])


class PrebakedRuntimeTest(unittest.TestCase):
    def test_prebaked_runtime_keeps_exact_install_and_inventory_check_without_update(self):
        for matching in (True, False):
            with self.subTest(matching=matching), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                (root / 'packages').write_text('a=1\n')
                args = argparse.Namespace(packages=root / 'packages', descriptor=root / 'unused',
                    state=root / 'state', cost=root / 'cost', source=root, build=root,
                    uv=root / 'uv', archives=root, mode='OFF', part='all',
                    prebaked_image='ghcr.io/obs-rust/tier2-linux@sha256:' + 'c' * 64)

                def command(*argv):
                    if argv[:2] == ('docker', 'create'):
                        return 'd' * 64
                    if 'dpkg-query' in argv:
                        return 'a\t' + ('1' if matching else '2')
                    return ''

                with patch.object(runtime, 'prepared_image') as prepared, \
                        patch.object(runtime, 'command', side_effect=command) as commands, \
                        patch.object(runtime.subprocess, 'run') as run:
                    if matching:
                        runtime.execute(args)
                        self.assertEqual(json.loads((args.state / 'runtime-measurement.json').read_text())['variant'], 'prebaked')
                        run.assert_called_once()
                    else:
                        with self.assertRaisesRegex(ValueError, 'package closure'):
                            runtime.execute(args)
                        run.assert_not_called()
                    prepared.assert_not_called()
                    calls = [call.args for call in commands.call_args_list]
                    self.assertFalse(any('update' in call for call in calls))
                    self.assertTrue(any('install' in call and 'a=1' in call for call in calls))
                    self.assertEqual(calls[-1], ('docker', 'stop', '--timeout', '1', 'd' * 64))


if __name__ == '__main__':
    unittest.main()
