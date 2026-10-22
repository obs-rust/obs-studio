#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["websockets==15.0.1"]
# ///
"""Run the exported Flatpak with saved capture/browser sources (#1760)."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import secrets
import select
import shlex
import signal
import socket
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APP = 'com.obsproject.Studio'


def check_browser_pixels(pixels: bytes) -> None:
    """The browser occupies an otherwise unused 32x32 recording region."""
    if len(pixels) % 3 or len(pixels) < 120 * 3:
        raise ValueError('browser produced fewer than 120 decoded frames')
    if any(r > 40 or g < 200 or b > 40 for r, g, b in zip(pixels[0::3], pixels[1::3], pixels[2::3])):
        raise ValueError('browser HTML did not render its green test rectangle throughout the recording')


def check_capture_log(log: str, modules: list[str]) -> list[str]:
    required = ['[pipewire] Screencast session created', '[camera-portal] Found device obs-test-camera',
                "Started recording from 'obs-test-mic'", 'Recording Start']
    missing = [message for message in required if message not in log]
    unavailable_hardware = []
    if log.count('state: "streaming"') < 2:
        missing.append('both PipeWire sources streaming')
    loaded = re.search(r'Loaded Modules:(.*?)-{10}', log, re.S)
    if not loaded:
        missing.append('loaded-module inventory')
    else:
        for module in modules:
            aja_diagnostic = {'aja.so': 'AJA plugin', 'aja-output-ui.so': 'AJA UI plugin'}.get(module)
            if (aja_diagnostic and f'No AJA devices found, skipping loading {aja_diagnostic}' in log
                    and f"Failed to initialize module '{module}'" in log):
                unavailable_hardware.append(module)
                continue
            if module not in ('decklink.so', 'obs-nvenc.so') and not re.search(r'\b' + re.escape(module) + r'\s*$', loaded[1], re.M):
                missing.append(module)
    if missing:
        raise ValueError('missing packaged runtime evidence: ' + ', '.join(missing))
    return unavailable_hardware


def select_refs(refs: list[str]) -> tuple[str, str]:
    normalized = []
    for ref in refs:
        ref = ref.strip()
        if ref.startswith(f'app/{APP}/'):
            ref = ref.removeprefix('app/')
        elif ref.startswith(f'runtime/{APP}.Debug/'):
            ref = ref.removeprefix('runtime/')
        normalized.append(ref)
    refs = normalized
    apps = [ref for ref in refs if re.fullmatch(re.escape(APP) + r'/[^/]+/[^/]+', ref)]
    if len(apps) != 1:
        raise ValueError(f'expected one exported application ref, found {apps}')
    app_ref = apps[0]
    debug_ref = app_ref.replace(f'{APP}/', f'{APP}.Debug/', 1)
    if debug_ref not in refs:
        raise ValueError('exported repository has no matching Debug extension')
    return app_ref, debug_ref


def write_profile(output: Path, port: int, password: str) -> None:
    config = output / 'profile/obs-studio'
    profile = config / 'basic/profiles/Test'
    profile.mkdir(parents=True)
    scenes = config / 'basic/scenes'
    scenes.mkdir()
    (output / 'recordings').mkdir()
    (output / 'browser.html').write_text('<!doctype html><html style="background:#00ff00;width:100%;height:100%"></html>')
    # This runner exposes llvmpipe and no DRM render node. Exercise the user's
    # software browser setting explicitly rather than requesting shared GPU textures.
    (config / 'global.ini').write_text('[General]\nLastVersion=4294967295\nBrowserHWAccel=false\n')
    (config / 'user.ini').write_text('[General]\nFirstRun=true\nConfirmOnExit=false\n\n[Basic]\nProfile=Test\nProfileDir=Test\nSceneCollection=Test\nSceneCollectionFile=Test\n')
    (profile / 'basic.ini').write_text(f'''[General]
Name=Test
[Output]
Mode=Simple
[SimpleOutput]
FilePath={output / 'recordings'}
RecFormat2=mkv
RecQuality=Small
RecEncoder=x264
[Video]
BaseCX=320
BaseCY=240
OutputCX=320
OutputCY=240
FPSType=0
FPSCommon=30
[Audio]
SampleRate=48000
ChannelSetup=Stereo
''')
    items = []
    for index, (name, x, y, width, height) in enumerate([
            ('Screen', 0, 0, 320, 240), ('Camera', 240, 180, 80, 60),
            ('Mic', 0, 0, 0, 0), ('Browser', 272, 32, 32, 32)], 1):
        item = dict(name=name, id=index, visible=True, pos=dict(x=x, y=y), scale=dict(x=1, y=1), align=5)
        if width:
            item.update(bounds_type=2, bounds=dict(x=width, y=height), bounds_align=0)
        items.append(item)
    sources = [dict(id='scene', versioned_id='scene', name='Scene', settings=dict(id_counter=4, items=items))]
    for name, kind, settings in [
        ('Screen', 'pipewire-screen-capture-source', dict(ShowCursor=False)),
        ('Camera', 'pipewire-camera-source', dict(device_id='obs-test-camera',
            format=json.dumps(dict(encoded=False, video_format=4, width=160, height=120)),
            framerate=json.dumps(dict(framerate=dict(numerator=30, denominator=1))))),
        ('Mic', 'pulse_input_capture', dict(device_id='obs-test-mic')),
        ('Browser', 'browser_source', dict(is_local_file=True, local_file=str(output / 'browser.html'),
                                         width=32, height=32, fps=30))]:
        sources.append(dict(id=kind, versioned_id=kind, name=name, settings=settings))
    (scenes / 'Test.json').write_text(json.dumps(dict(name='Test', current_scene='Scene', current_program_scene='Scene',
                                                   scene_order=[dict(name='Scene')], sources=sources), indent=2))
    websocket = config / 'plugin_config/obs-websocket'
    websocket.mkdir(parents=True)
    (websocket / 'config.json').write_text(json.dumps(dict(first_load=False, server_enabled=True, server_port=port,
        alerts_enabled=False, auth_required=True, server_password=password)))


def app_command(output: Path, app_ref: str) -> list[str]:
    # Flatpak replaces protected XDG variables after processing --env. Set the
    # fixture's config root with the packaged runtime's env, inside the sandbox.
    return ['flatpak', '--user', 'run', f'--filesystem={output}',
            '--env=LIBGL_ALWAYS_SOFTWARE=1', '--env=QT_QPA_PLATFORM=xcb',
            '--command=env', app_ref, f'XDG_CONFIG_HOME={output / "profile"}',
            '/app/bin/obs', '--disable-shutdown-check', '--verbose', '--profile', 'Test', '--collection', 'Test']


class Session:
    def __init__(self, output: Path):
        self.output = output
        self.env = {k: v for k, v in os.environ.items() if k not in (
            'DISPLAY', 'WAYLAND_DISPLAY', 'DBUS_SESSION_BUS_ADDRESS', 'PIPEWIRE_REMOTE', 'PIPEWIRE_RUNTIME_DIR',
            'PIPEWIRE_CONFIG_DIR', 'PIPEWIRE_MODULE_DIR', 'SPA_PLUGIN_DIR', 'PULSE_SERVER', 'XDG_RUNTIME_DIR')}
        self.env.update(XDG_RUNTIME_DIR=str(output / 'run'), XDG_CONFIG_HOME=str(output / 'config'),
                        XDG_DATA_HOME=str(output / 'data'), XDG_CACHE_HOME=str(output / 'cache'),
                        XDG_STATE_HOME=str(output / 'state'))
        for name in ('run', 'config', 'data', 'cache', 'state'):
            (output / name).mkdir(mode=0o700)
        self.processes = []
        self.logs = []
        self.command_index = 0

    def run(self, args: list[str], timeout: int = 180) -> str:
        self.command_index += 1
        result = subprocess.run(args, env=self.env, capture_output=True, text=True, timeout=timeout)
        (self.output / f'command-{self.command_index:02}.log').write_text(
            json.dumps(args) + '\n' + result.stdout + result.stderr)
        result.check_returncode()
        return result.stdout

    def start(self, name: str, args: list[str], pipe: bool = False, extra: dict | None = None):
        log = (self.output / f'{name}.log').open('w')
        self.logs.append(log)
        process = subprocess.Popen(args, env={**self.env, **(extra or {})}, stdout=subprocess.PIPE if pipe else log,
                                   stderr=log, start_new_session=True)
        self.processes.append(process)
        return process

    def line(self, process) -> str:
        if not select.select([process.stdout], [], [], 15)[0]:
            raise RuntimeError('desktop service did not publish its address within 15 seconds')
        line = process.stdout.readline().decode().strip()
        if not line:
            raise RuntimeError('desktop service exited without an address')
        return line

    def wait_for(self, predicate, description: str, seconds: int = 20):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            result = predicate()
            if result:
                return result
            time.sleep(0.2)
        raise RuntimeError('timed out waiting for ' + description)

    def close(self):
        for process in reversed(self.processes):
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
        for process in reversed(self.processes):
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
        for log in self.logs:
            log.close()


def connect_obs(port: int, password: str):
    from websockets.sync.client import connect
    deadline = time.monotonic() + 60
    while True:
        try:
            ws = connect(f'ws://127.0.0.1:{port}', open_timeout=1, proxy=None)
            break
        except (OSError, TimeoutError):
            if time.monotonic() >= deadline:
                raise
            time.sleep(0.2)
    hello = json.loads(ws.recv(timeout=10))
    if hello['op'] != 0:
        raise RuntimeError('obs-websocket did not send a v5 Hello')
    challenge = hello['d']['authentication']
    secret = base64.b64encode(hashlib.sha256((password + challenge['salt']).encode()).digest()).decode()
    auth = base64.b64encode(hashlib.sha256((secret + challenge['challenge']).encode()).digest()).decode()
    ws.send(json.dumps(dict(op=1, d=dict(rpcVersion=1, authentication=auth, eventSubscriptions=0))))
    if json.loads(ws.recv(timeout=10))['op'] != 2:
        raise RuntimeError('obs-websocket v5 identification failed')
    return ws


def request(ws, name: str, ready_timeout: int = 0) -> dict:
    if ready_timeout and name != 'GetVersion':
        raise ValueError('only the read-only GetVersion readiness check may retry')
    deadline = time.monotonic() + ready_timeout
    while True:
        identity = secrets.token_hex(8)
        ws.send(json.dumps(dict(op=6, d=dict(requestType=name, requestId=identity))))
        message = json.loads(ws.recv(timeout=30))
        if message['op'] != 7 or message['d']['requestId'] != identity:
            raise RuntimeError(f'obs-websocket {name} invalid response: {message}')
        status = message['d']['requestStatus']
        if status['result']:
            return message['d'].get('responseData', {})
        # The unchanged plugin accepts clients before scene loading completes.
        # Retry only its explicit NotReady status, bounded by a deadline.
        if ready_timeout and status['code'] == 207 and time.monotonic() < deadline:
            time.sleep(0.2)
            continue
        raise RuntimeError(f'obs-websocket {name} failed: {message}')


def start_recording(session: Session, ws) -> None:
    request(ws, 'StartRecord')
    session.wait_for(lambda: request(ws, 'GetRecordStatus')['outputActive'],
                     'packaged recording to become active', seconds=20)


def qualification(args) -> dict:
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    session = Session(output)
    start = time.monotonic()
    result = dict(status='failed', source=os.environ.get('FLATPAK_ARTIFACT_SOURCE', os.environ.get('GITHUB_SHA')),
                  harness_source=os.environ.get('GITHUB_SHA'),
                  artifact_run=os.environ.get('FLATPAK_ARTIFACT_RUN', os.environ.get('GITHUB_RUN_ID')), app=APP,
                  coverage='synthetic screen/camera/microphone producers; software browser rendering; no physical devices or hardware GPU',
                  limits='One runtime qualification, not routine full-job performance evidence')
    try:
        flatpak = ['flatpak', '--user']
        session.run([*flatpak, 'remote-add', '--if-not-exists', 'flathub', 'https://flathub.org/repo/flathub.flatpakrepo'])
        session.run([*flatpak, 'remote-add', '--no-gpg-verify', 'qualification', str(args.repo.resolve())])
        refs = session.run([*flatpak, 'remote-ls', '--all', 'qualification', '--columns=ref']).splitlines()
        app_ref, debug_ref = select_refs(refs)
        session.run([*flatpak, 'install', '-y', '--noninteractive', 'qualification', app_ref, debug_ref], timeout=900)
        installed = Path(session.run([*flatpak, 'info', '--show-location', app_ref]).strip())
        debug = Path(session.run([*flatpak, 'info', '--show-location', debug_ref]).strip())
        result.update(app_ref=app_ref, debug_ref=debug_ref,
                      app_commit=session.run([*flatpak, 'info', '--show-commit', app_ref]).strip(),
                      debug_commit=session.run([*flatpak, 'info', '--show-commit', debug_ref]).strip())
        cef = list((installed / 'files').rglob('libcef.so'))
        cef_debug = list((debug / 'files').rglob('libcef.so.debug'))
        if len(cef) != 1 or len(cef_debug) != 1:
            raise RuntimeError('installed application/debug extension must contain exactly one CEF binary/debug pair')
        ids = [re.search(r'Build ID: ([0-9a-f]+)', session.run(['readelf', '-n', str(path)]))[1]
               for path in (cef[0], cef_debug[0])]
        if ids[0] != ids[1]:
            raise RuntimeError('CEF binary and installed Debug extension build IDs differ')
        # Explicit directory covers Flatpak's /app/lib/debug layout from a host debugger.
        gdb = session.run(['gdb', '-nx', '-batch', '-iex', 'set debuginfod enabled off',
                           '-ex', f'set debug-file-directory {debug / "files"}', '-ex', f'file {cef[0]}',
                           '-ex', f'add-symbol-file {cef_debug[0]}', '-ex', 'info line cef_version_info'])
        if not re.search(r'Line \d+ of .*libcef_dll2.cc', gdb):
            raise RuntimeError('GDB could not resolve CEF source lines from the installed Debug extension')
        result['cef_build_id'] = ids[0]
        flags = shlex.split(session.run(['pkg-config', '--cflags', '--libs', 'gio-unix-2.0', 'libpipewire-0.3', 'libpulse', 'libcurl']))
        kit = output / 'testkit'
        session.run(['gcc', '-O2', '-Wall', '-Wextra', '-Werror', '-o', str(kit),
                     str(ROOT / 'build-aux/appimage/testkit.c'), *flags, '-lm'])
        display = session.start('xvfb', ['Xvfb', '-displayfd', '1', '-screen', '0', '1280x800x24', '-nolisten', 'tcp'], pipe=True)
        session.env['DISPLAY'] = ':' + session.line(display)
        bus = session.start('dbus', ['dbus-daemon', '--session', '--nofork', '--print-address=1'], pipe=True)
        session.env['DBUS_SESSION_BUS_ADDRESS'] = session.line(bus)
        session.start('pipewire', ['pipewire'])
        session.wait_for(lambda: (output / 'run/pipewire-0').is_socket(), 'PipeWire socket')
        session.start('wireplumber', ['wireplumber'])
        session.start('pulse', ['pipewire-pulse'])
        session.wait_for(lambda: (output / 'run/pulse/native').is_socket(), 'PulseAudio socket')
        for name, pattern, fmt, width, height in [('screen', 'smpte', 'BGRx', 320, 240), ('camera', 'ball', 'YUY2', 160, 120)]:
            session.start('producer-' + name, ['gst-launch-1.0', '-q', 'videotestsrc', f'pattern={pattern}',
                *(['horizontal-speed=4'] if name == 'screen' else ['background-color=0xff204080']), 'is-live=true', '!',
                f'video/x-raw,format={fmt},width={width},height={height},framerate=30/1', '!', 'pipewiresink', 'mode=provide',
                f'client-name=obs-test-{name}', 'stream-properties=props,media.class=Video/Source,' +
                ('media.role=Camera,' if name == 'camera' else '') + f'node.name=obs-test-{name},node.description=obs-test-{name}'])
        session.run([str(kit), 'write-tone', str(output / 'tone.wav'), '180'])
        session.start('producer-mic', ['pw-play', str(output / 'tone.wav')], extra={
            'PIPEWIRE_PROPS': '{ media.class = "Audio/Source" node.name = "obs-test-mic" node.description = "obs-test-mic" }'})
        def nodes():
            objects = json.loads(session.run(['pw-dump']))
            return {obj.get('info', {}).get('props', {}).get('node.name'): obj['id'] for obj in objects}
        found = session.wait_for(lambda: (data if all(n in data for n in ('obs-test-screen', 'obs-test-camera', 'obs-test-mic')) else None)
                                 if (data := nodes()) else None, 'all three producers')
        session.start('portal', [str(kit), 'fake-portal'], extra={
            'OBS_TEST_SCREEN_NODE': str(found['obs-test-screen']), 'OBS_TEST_SCREEN_SIZE': '320x240'})
        session.wait_for(lambda: 'serving' in (output / 'portal.log').read_text(), 'fake portal')
        with socket.socket() as port_socket:
            port_socket.bind(('127.0.0.1', 0))
            port = port_socket.getsockname()[1]
        password = secrets.token_urlsafe(24)
        write_profile(output, port, password)
        app = session.start('obs', app_command(output, app_ref))
        ws = connect_obs(port, password)
        try:
            result['websocket_version'] = request(ws, 'GetVersion', ready_timeout=60)
            inputs = request(ws, 'GetInputList')['inputs']
            expected = dict(Screen='pipewire-screen-capture-source', Camera='pipewire-camera-source',
                            Mic='pulse_input_capture', Browser='browser_source')
            if {item['inputName']: item['inputKind'] for item in inputs} != expected:
                raise RuntimeError(f'saved sources did not load: {inputs}')
            start_recording(session, ws)
            time.sleep(12)
            recording = Path(request(ws, 'StopRecord')['outputPath'])
        finally:
            ws.close()
        # Signal only the process executing this installation's binary.
        executable = installed / 'files/bin/obs'
        processes = []
        for process in Path('/proc').glob('[0-9]*'):
            try:
                if (process / 'exe').samefile(executable):
                    processes.append(int(process.name))
            except (FileNotFoundError, PermissionError, ProcessLookupError):
                continue
        if len(processes) != 1:
            raise RuntimeError(f'could not identify exactly one packaged OBS process: {processes}')
        os.kill(processes[0], signal.SIGINT)
        if app.wait(timeout=30) != 0:
            raise RuntimeError('packaged OBS did not exit cleanly')
        logs = list((output / 'profile/obs-studio/logs').glob('*.txt'))
        if len(logs) != 1:
            raise RuntimeError('saved profile must produce exactly one OBS log')
        modules = []
        for path in (installed / 'files').rglob('*.so'):
            if 'obs-modules' in path.parts or 'obs-plugins' in path.parts:
                symbols = session.run(['nm', '-D', '--defined-only', str(path)])
                if re.search(r'\bobs_module_load$', symbols, re.M):
                    modules.append(path.name)
        modules = sorted(set(modules))
        if not {'obs-browser.so', 'obs-websocket.so'}.issubset(modules):
            raise RuntimeError('package module inventory omits browser or unchanged WebSocket plugin')
        result['unavailable_hardware_modules'] = check_capture_log(logs[0].read_text(), modules)
        if 'OpenPipeWireRemote' not in (output / 'portal.log').read_text():
            raise RuntimeError('packaged app never requested its portal PipeWire connection')
        session.run(['bash', str(ROOT / 'build-aux/appimage/verify-recording.sh'), str(recording)])
        pixels = subprocess.check_output(['ffmpeg', '-nostdin', '-v', 'error', '-ss', '2', '-i', str(recording),
            '-an', '-vf', 'crop=32:32:272:32,scale=1:1:flags=area,format=rgb24', '-f', 'rawvideo', '-'], timeout=60)
        check_browser_pixels(pixels)
        result.update(status='success', recording=str(recording), browser_frames=len(pixels) // 3, modules=modules)
        return result
    finally:
        try:
            session.close()
        finally:
            result['seconds'] = time.monotonic() - start
            (output / 'result.json').write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True, help='exported Flatpak OSTree repository')
    parser.add_argument('--output', type=Path, required=True, help='new directory for installation, session and evidence')
    print(json.dumps(qualification(parser.parse_args()), indent=2))
