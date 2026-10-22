"""Flatpak packaged runtime evidence must fail closed (#1760)."""
import importlib.util
import configparser
import json
import tempfile
import re
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class FlatpakRuntimeTest(unittest.TestCase):
    def module(self):
        spec = importlib.util.spec_from_file_location('flatpak_runtime', ROOT / 'ci/flatpak_runtime.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_browser_pixels_require_real_green_frames(self):
        module = self.module()
        module.check_browser_pixels(bytes([0, 255, 0]) * 150)
        for pixels in (b'', bytes([0, 255, 0]) * 119, bytes([0, 0, 0]) * 150,
                       bytes([255, 0, 0]) * 150, bytes([0, 255, 0]) * 149 + bytes([0, 0, 0])):
            with self.subTest(pixels=len(pixels)), self.assertRaises(ValueError):
                module.check_browser_pixels(pixels)

    def test_capture_log_requires_every_source_and_plugin(self):
        module = self.module()
        lines = ['[pipewire] Screencast session created', 'state: "streaming"', 'state: "streaming"',
                 '[camera-portal] Found device obs-test-camera', "Started recording from 'obs-test-mic'",
                 'Recording Start', 'Loaded Modules:', '    obs-browser.so', '    obs-websocket.so',
                 '    obs-ffmpeg.so', '----------------']
        modules = ['obs-browser.so', 'obs-websocket.so', 'obs-ffmpeg.so']
        module.check_capture_log('\n'.join(lines), modules)
        for index in (0, 1, 3, 4, 5, 7, 8, 9):
            with self.subTest(index=index), self.assertRaises(ValueError):
                module.check_capture_log('\n'.join(lines[:index] + lines[index + 1:]), modules)

    def test_aja_absence_requires_explicit_hardware_diagnostic(self):
        module = self.module()
        log = '\n'.join(['[pipewire] Screencast session created', 'state: "streaming"', 'state: "streaming"',
                         '[camera-portal] Found device obs-test-camera', "Started recording from 'obs-test-mic'",
                         'Recording Start', 'Loaded Modules:', '    obs-browser.so', '    obs-websocket.so',
                         '----------------'])
        modules = ['obs-browser.so', 'obs-websocket.so', 'aja.so', 'aja-output-ui.so']
        with self.assertRaises(ValueError):
            module.check_capture_log(log, modules)
        diagnostics = "\n".join(["No AJA devices found, skipping loading AJA plugin", "Failed to initialize module 'aja.so'",
                                  "No AJA devices found, skipping loading AJA UI plugin", "Failed to initialize module 'aja-output-ui.so'"])
        self.assertEqual(module.check_capture_log(log + '\n' + diagnostics, modules), ['aja.so', 'aja-output-ui.so'])
        with self.assertRaises(ValueError):
            module.check_capture_log(log + '\n' + diagnostics.replace("No AJA devices found", "Missing library"), modules)

    def test_export_requires_matching_debug_architecture_and_branch(self):
        module = self.module()
        app = 'com.obsproject.Studio/x86_64/master'
        debug = 'com.obsproject.Studio.Debug/x86_64/master'
        self.assertEqual(module.select_refs([app, debug]), (app, debug))
        for refs in ([app], [app, debug.replace('x86_64', 'aarch64')],
                     [app, debug.replace('master', 'stable')], [app, debug, app.replace('master', 'stable')]):
            with self.subTest(refs=refs), self.assertRaises(ValueError):
                module.select_refs(refs)

    def test_actual_exported_remote_refs_have_kind_prefixes(self):
        module = self.module()
        # Exact remote-ls output from hosted qualification 39754042722.
        refs = ['app/com.obsproject.Studio/x86_64/master',
                'runtime/com.obsproject.Studio.Debug/x86_64/master']
        self.assertEqual(module.select_refs(refs), ('com.obsproject.Studio/x86_64/master',
                                                   'com.obsproject.Studio.Debug/x86_64/master'))
        with self.assertRaises(ValueError):
            module.select_refs([refs[0], refs[1].replace('runtime/', 'app/')])

    def test_debug_inventory_includes_refs_hidden_by_flatpak_default(self):
        module = self.module()
        class ReachedInstall(Exception):
            pass
        def flatpak(args, **kwargs):
            if 'remote-ls' in args:
                # Real CLI semantics: locale/debug refs are hidden without --all.
                return 'com.obsproject.Studio/x86_64/master\n' + (
                    'com.obsproject.Studio.Debug/x86_64/master\n' if '--all' in args else '')
            if 'install' in args:
                raise ReachedInstall()
            return ''
        session = Mock()
        session.run.side_effect = flatpak
        with tempfile.TemporaryDirectory() as directory, patch.object(module, 'Session', return_value=session):
            args = SimpleNamespace(output=Path(directory) / 'evidence', repo=Path(directory))
            with self.assertRaises(ReachedInstall):
                module.qualification(args)

    def test_saved_profile_contains_capture_and_browser_sources(self):
        module = self.module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            module.write_profile(root, 4456, 'temporary-test-password')
            config = root / 'profile/obs-studio'
            global_config = configparser.ConfigParser()
            global_config.read(config / 'global.ini')
            self.assertFalse(global_config.getboolean('General', 'BrowserHWAccel'))
            scene = json.loads((config / 'basic/scenes/Test.json').read_text())
            kinds = {source['id'] for source in scene['sources']}
            self.assertEqual(kinds, {'scene', 'pipewire-screen-capture-source', 'pipewire-camera-source',
                                     'pulse_input_capture', 'browser_source'})
            server = json.loads((config / 'plugin_config/obs-websocket/config.json').read_text())
            self.assertTrue(server['auth_required'])
            self.assertTrue(server['server_enabled'])
            self.assertEqual(server['server_port'], 4456)
            self.assertTrue((root / 'browser.html').is_file())

    def test_profile_environment_is_set_inside_flatpak_sandbox(self):
        module = self.module()
        # Flatpak rewrites XDG_CONFIG_HOME after processing --env options.
        command = module.app_command(Path('/tmp/qualification evidence'), 'com.obsproject.Studio/x86_64/master')
        app_index = command.index('com.obsproject.Studio/x86_64/master')
        self.assertIn('--command=env', command[:app_index])
        self.assertIn('XDG_CONFIG_HOME=/tmp/qualification evidence/profile', command[app_index + 1:])
        self.assertEqual(command[app_index + 2], '/app/bin/obs')
        self.assertFalse(any(arg.startswith('--env=XDG_CONFIG_HOME=') for arg in command))

    def test_wait_for_version_retries_only_explicit_not_ready(self):
        module = self.module()
        def reply(code, result=False):
            return json.dumps(dict(op=7, d=dict(requestId='id', requestStatus=dict(code=code, result=result),
                                               responseData=dict(obsVersion='test'))))
        ws = Mock()
        ws.recv.side_effect = [reply(207), reply(100, True)]
        with patch.object(module.secrets, 'token_hex', return_value='id'), patch.object(module.time, 'sleep'):
            self.assertEqual(module.request(ws, 'GetVersion', ready_timeout=60), dict(obsVersion='test'))
        self.assertEqual(ws.send.call_count, 2)
        for name, timeout, code in [('StartRecord', 0, 207), ('GetVersion', 60, 500)]:
            ws = Mock()
            ws.recv.return_value = reply(code)
            with patch.object(module.secrets, 'token_hex', return_value='id'), self.assertRaises(RuntimeError):
                module.request(ws, name, ready_timeout=timeout)
            self.assertEqual(ws.send.call_count, 1)
        ws = Mock()
        ws.recv.return_value = reply(207)
        with patch.object(module.secrets, 'token_hex', return_value='id'), \
             patch.object(module.time, 'monotonic', side_effect=[0, 61]), self.assertRaises(RuntimeError):
            module.request(ws, 'GetVersion', ready_timeout=60)
        self.assertEqual(ws.send.call_count, 1)

    def test_record_start_waits_for_active_state_without_repeating_command(self):
        module = self.module()
        session = module.Session.__new__(module.Session)
        ws = Mock()
        with patch.object(module, 'request', side_effect=[{}, {'outputActive': False}, {'outputActive': True}]) as request, \
             patch.object(module.time, 'sleep'):
            module.start_recording(session, ws)
        self.assertEqual([call.args[1] for call in request.call_args_list],
                         ['StartRecord', 'GetRecordStatus', 'GetRecordStatus'])
        with patch.object(module, 'request', side_effect=[{}, {'outputActive': False}]) as request, \
             patch.object(module.time, 'sleep'), patch.object(module.time, 'monotonic', side_effect=[0, 0, 21]), \
             self.assertRaisesRegex(RuntimeError, 'recording to become active'):
            module.start_recording(session, ws)
        self.assertEqual([call.args[1] for call in request.call_args_list], ['StartRecord', 'GetRecordStatus'])

    def test_runtime_only_consumer_reuses_artifact_and_preserves_source_identity(self):
        text = (ROOT / '.github/workflows/ci.yml').read_text()
        self.assertIn('flatpak-runtime-reuse-run:', text)
        block = text.split('\n  flatpak-runtime-reuse:\n', 1)[1].split('\n  # Opt-in', 1)[0]
        self.assertIn('inputs.flatpak-runtime-reuse-run', block)
        self.assertIn('github-token: ${{ github.token }}', block)
        self.assertIn('FLATPAK_ARTIFACT_SOURCE', block)
        self.assertIn('flatpak-runtime-reuse]', text)
        script = (ROOT / 'ci/flatpak_runtime.py').read_text()
        self.assertIn("harness_source=os.environ.get('GITHUB_SHA')", script)
        self.assertIn("os.environ.get('FLATPAK_ARTIFACT_SOURCE'", script)

    def test_qualification_is_manual_and_preserves_existing_validators(self):
        for name in ('ci.yml', 'build-project.yaml'):
            text = (ROOT / '.github/workflows' / name).read_text()
            self.assertIn('flatpak-runtime-test:', text)
            block = re.search(r'(?m)^      flatpak-runtime-test:\n((?:        .*\n)+)', text)[1]
            self.assertIn('default: false', block)
        build = (ROOT / '.github/workflows/build-project.yaml').read_text()
        self.assertIn('uv run ci/flatpak_runtime.py', build)
        self.assertIn('name: Validate repository', build)
        self.assertIn('name: Validate build directory', build)
        self.assertIn('name: Validate AppStream', build)
        job = build.split('\n  flatpak-runtime-test:\n', 1)[1].split('\n  windows-build:', 1)[0]
        self.assertIn('!cancelled() && inputs.flatpak-runtime-test', job)
        self.assertIn("needs.flatpak-build.result == 'success'", job)
        self.assertIn("!inputs.flatpak-dependency-transfer-probe || needs.flatpak-compiler-consumer.result == 'success'", job)
        self.assertNotIn('continue-on-error', job)


    def test_dependency_transport_runtime_uses_consumer_export(self):
        build = (ROOT / '.github/workflows/build-project.yaml').read_text()
        producer = build.split('      - name: Archive Exported Flatpak for Runtime Qualification', 1)[1].split('  flatpak-compiler-consumer:', 1)[0]
        self.assertIn('inputs.flatpak-runtime-test && !inputs.flatpak-dependency-transfer-probe', producer)
        consumer = build.split('  flatpak-compiler-consumer:', 1)[1].split('  flatpak-runtime-test:', 1)[0]
        export = consumer.split('      - name: Archive Compiler Consumer for Runtime Qualification', 1)[1]
        self.assertIn('inputs.flatpak-runtime-test && inputs.flatpak-dependency-transfer-probe', export)
        self.assertIn('name: flatpak-runtime-repo', export)
        self.assertIn('tar -cf "$RUNNER_TEMP/flatpak-runtime-repo.tar" repo', export)
        runtime = build.split('  flatpak-runtime-test:', 1)[1].split('  windows-build:', 1)[0]
        self.assertIn('needs: [flatpak-build, flatpak-compiler-consumer]', runtime)
        self.assertIn('name: flatpak-runtime-repo', runtime)
