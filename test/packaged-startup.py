#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["websockets==15.0.1"]
# ///
"""Launch a packaged frontend with a saved screen/camera/microphone profile.

    uv run --no-project test/packaged-startup.py --app APP --config-dir DIR --platform windows --expect-arch arm64

Checks the running app over obs-websocket, including capture plugin registration,
saved inputs/scene/profile, and clean shutdown. Hosted runners have no camera or
microphone devices; this check validates package loading and saved-profile state.
The config directory must be absent and is removed after the owned process exits.
"""

from __future__ import annotations

import argparse
import asyncio
import base64
import configparser
import hashlib
import json
import os
import secrets
import shutil
import signal
import socket
import struct
import subprocess
import sys
import time
import uuid
from pathlib import Path

PROFILE = "PackagedSmoke"
SCENE = "Packaged scene"
KINDS = {
    "windows": ("monitor_capture", "dshow_input", "wasapi_input_capture"),
    "macos": ("screen_capture", "macos-avcapture", "coreaudio_input_capture"),
}
NAMES = ("Package screen", "Package camera", "Package microphone")


def require_config_directory(config: Path, platform: str) -> None:
    if platform == "windows":
        appdata = os.environ.get("APPDATA")
        if not appdata:
            raise RuntimeError(
                "APPDATA is required to locate the frontend configuration"
            )
        expected = Path(appdata) / "obs-studio-rust"
    else:
        expected = Path.home() / "Library/Application Support/obs-studio-rust"
    if config.resolve() != expected.resolve():
        raise RuntimeError(
            f"fixture must use the frontend configuration directory: {expected}"
        )


def write_profile(config: Path, platform: str, port: int, password: str) -> None:
    if config.exists():
        raise RuntimeError(f"refusing to replace existing configuration: {config}")
    config.mkdir(parents=True)
    try:
        _write_profile(config, platform, port, password)
    except BaseException:
        shutil.rmtree(config)
        raise


def _write_profile(config: Path, platform: str, port: int, password: str) -> None:
    (config / "global.ini").write_text(
        "[General]\nLastVersion=4294967295\nMacOSPermissionsDialogLastShown=2147483647\n",
        encoding="utf-8",
    )
    (config / "user.ini").write_text(
        "[General]\nFirstRun=true\nConfirmOnExit=false\n"
        f"[Basic]\nProfile={PROFILE}\nProfileDir={PROFILE}\n"
        f"SceneCollection={PROFILE}\nSceneCollectionFile={PROFILE}\n",
        encoding="utf-8",
    )
    profile = config / "basic/profiles" / PROFILE
    profile.mkdir(parents=True)
    (profile / "basic.ini").write_text(
        f"[General]\nName={PROFILE}\n[Video]\nBaseCX=1280\nBaseCY=720\n"
        "OutputCX=1280\nOutputCY=720\nFPSType=0\nFPSCommon=30\n"
        "[Audio]\nSampleRate=48000\nChannelSetup=Stereo\n",
        encoding="utf-8",
    )
    sources = [
        {
            "name": name,
            "id": kind,
            "uuid": str(uuid.uuid4()),
            "settings": {},
            "volume": 1.0,
        }
        for name, kind in zip(NAMES, KINDS[platform], strict=True)
    ]
    sources.append(
        {
            "name": SCENE,
            "id": "scene",
            "uuid": str(uuid.uuid4()),
            "settings": {
                "items": [
                    {
                        "name": source["name"],
                        "source_uuid": source["uuid"],
                        "visible": True,
                    }
                    for source in sources
                ]
            },
        }
    )
    scenes = config / "basic/scenes"
    scenes.mkdir(parents=True)
    (scenes / f"{PROFILE}.json").write_text(
        json.dumps(
            {
                "name": PROFILE,
                "current_scene": SCENE,
                "current_program_scene": SCENE,
                "scene_order": [{"name": SCENE}],
                "sources": sources,
            }
        ),
        encoding="utf-8",
    )
    websocket = config / "plugin_config/obs-websocket"
    websocket.mkdir(parents=True)
    (websocket / "config.json").write_text(
        json.dumps(
            {
                "first_load": False,
                "server_enabled": True,
                "server_port": port,
                "auth_required": True,
                "server_password": password,
                "alerts_enabled": False,
            }
        ),
        encoding="utf-8",
    )


class ConfigFixture:
    def __init__(  # noqa: C901
        self, config: Path, platform: str, mode: str, port: int, password: str
    ):
        self.config = config
        self.mode = mode
        self.owned = []
        self.originals = {}
        self.selected = {
            key: config.parent
            for key in (
                "Configuration",
                "Profiles",
                "SceneCollections",
                "PluginManagerSettings",
            )
        }
        self.staging = config.with_name(config.name + ".migration")
        self.lock = config.with_name(config.name + ".migration.lock")
        for path in (config, self.staging, self.lock):
            self.require_absent(path)
        upstream = config.parent / "obs-studio"
        if mode != "fresh":
            self.require_absent(upstream)
        try:
            if mode in ("fresh", "existing"):
                write_profile(config, platform, port, password)
                self.owned.append(config)
            else:
                # The owned frontend must create this directory during import.
                self.owned.append(config)
            if mode == "fresh":
                return
            write_profile(upstream, platform, port, password)
            self.owned.append(upstream)
            (upstream / "plugin_manager").mkdir()
            (upstream / "plugin_manager/migration-proof.txt").write_text(
                "preserved plugin settings"
            )
            if mode == "existing":
                # A distinct websocket configuration detects accidental import
                # over the Rust fixture, even when capture kinds are identical.
                websocket = upstream / "plugin_config/obs-websocket/config.json"
                settings = json.loads(websocket.read_text())
                settings["server_enabled"] = False
                settings["server_password"] = "upstream must not replace Rust"
                websocket.write_text(json.dumps(settings))
            roots = [upstream]
            if mode == "migrate-custom":
                locations = config.parent / ("packaged-locations-" + uuid.uuid4().hex)
                self.require_absent(locations)
                locations.mkdir()
                self.owned.append(locations)
                selected = {
                    key: locations / key
                    for key in (
                        "Configuration",
                        "Profiles",
                        "SceneCollections",
                        "PluginManagerSettings",
                    )
                }
                for root in selected.values():
                    (root / "obs-studio").mkdir(parents=True)
                    roots.append(root / "obs-studio")
                configuration = selected["Configuration"] / "obs-studio"
                shutil.move(upstream / "user.ini", configuration / "user.ini")
                self.selected = selected
                for key, relative in (
                    ("Profiles", "basic/profiles"),
                    ("SceneCollections", "basic/scenes"),
                    ("PluginManagerSettings", "plugin_manager"),
                ):
                    target = selected[key] / "obs-studio" / relative
                    target.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(upstream / relative, target)
                with (upstream / "global.ini").open("a", encoding="utf-8") as stream:
                    stream.write("[Locations]\n")
                    for key, root in selected.items():
                        stream.write(f"{key}={root.as_posix()}\n")
            self.originals = {root: self.snapshot(root) for root in roots}
        except BaseException:
            self.cleanup()
            raise

    @staticmethod
    def require_absent(path: Path) -> None:
        if path.exists() or path.is_symlink():
            raise RuntimeError(
                f"refusing to replace existing settings or migration state: {path}"
            )

    @staticmethod
    def snapshot(root: Path) -> dict:
        return {
            str(path.relative_to(root)): (
                "directory"
                if path.is_dir()
                else hashlib.sha256(path.read_bytes()).hexdigest()
            )
            for path in root.rglob("*")
        }

    def verify(self) -> None:  # noqa: C901
        for root, original in self.originals.items():
            if self.snapshot(root) != original:
                raise RuntimeError(
                    f"packaged frontend changed original settings: {root}"
                )
        if not self.config.is_dir():
            raise RuntimeError(
                "packaged frontend did not create its Rust configuration"
            )
        settings = configparser.ConfigParser(interpolation=None)
        settings.read(self.config / "global.ini", encoding="utf-8-sig")
        imported = settings.get("General", "RustConfigImportedFrom", fallback="")
        if self.mode.startswith("migrate-"):
            if (
                not imported
                or Path(imported.replace("\\\\", "\\")).resolve()
                != (self.config.parent / "obs-studio").resolve()
            ):
                raise RuntimeError(
                    "packaged frontend did not record first-run migration"
                )
            for key in (
                "Configuration",
                "Profiles",
                "SceneCollections",
                "PluginManagerSettings",
            ):
                value = settings.get("Locations", key, fallback="")
                if (
                    not value
                    or Path(value.replace("\\\\", "\\")).resolve()
                    != self.selected[key].resolve()
                ):
                    raise RuntimeError(f"migrated {key} lost the selected storage root")
            marker = (
                self.selected["PluginManagerSettings"]
                / "obs-studio-rust/plugin_manager/migration-proof.txt"
            )
            if marker.read_text() != "preserved plugin settings":
                raise RuntimeError("plugin-manager settings were not migrated")
        elif imported:
            raise RuntimeError("an existing Rust configuration was imported over")
        if self.staging.exists() or self.lock.exists():
            raise RuntimeError(
                "packaged frontend left migration staging or lock state behind"
            )
        print(
            f"Rust configuration isolation verified: {self.mode}; original settings unchanged"
        )

    def cleanup(self) -> None:
        for path in reversed(self.owned):
            if path.exists():
                shutil.rmtree(path)
        # These paths were absent before the owned frontend could create them.
        if self.staging.exists():
            shutil.rmtree(self.staging)
        if self.lock.exists():
            self.lock.unlink()


def require_windows_arch(app: Path, arch: str) -> None:
    with app.open("rb") as executable:
        header = executable.read(64)
        if len(header) != 64 or header[:2] != b"MZ":
            raise RuntimeError("packaged executable has no PE header")
        executable.seek(struct.unpack_from("<I", header, 0x3C)[0])
        pe = executable.read(6)
    if len(pe) != 6 or pe[:4] != b"PE\0\0":
        raise RuntimeError("packaged executable has an invalid PE header")
    machine = struct.unpack_from("<H", pe, 4)[0]
    expected = {"x64": 0x8664, "arm64": 0xAA64}[arch]
    if machine != expected:
        raise RuntimeError(
            f"packaged executable machine {machine:#x} is not {arch} ({expected:#x})"
        )


def require_windows_modules(log: str) -> None:
    marker = "Loaded Modules:"
    if marker not in log:
        raise RuntimeError("packaged frontend log has no Loaded Modules checkpoint")
    modules = set()
    for line in log.split(marker, 1)[1].splitlines():
        content = line.rsplit(": ", 1)[-1].strip()
        if content.startswith("-----"):
            break
        if content.lower().endswith(".dll") and " " not in content:
            modules.add(content.lower())
    print("Packaged Windows loaded modules:", ", ".join(sorted(modules)), flush=True)
    required = {
        "win-capture.dll",
        "win-dshow.dll",
        "win-wasapi.dll",
        "obs-websocket.dll",
    }
    missing = required - modules
    if missing:
        raise RuntimeError(
            f"packaged frontend loaded module list is missing: {sorted(missing)}"
        )


def validate_profile(
    platform: str, kinds: list[str], inputs: list[dict], scene: dict, profile: dict
) -> None:
    missing = set(KINDS[platform]) - set(kinds)
    if missing:
        raise RuntimeError(f"packaged capture plugins are missing: {sorted(missing)}")
    actual = {
        item["inputName"]: item.get("unversionedInputKind", item["inputKind"])
        for item in inputs
    }
    for name, kind in zip(NAMES, KINDS[platform], strict=True):
        if actual.get(name) != kind:
            raise RuntimeError(
                f"saved input {name!r} has kind {actual.get(name)!r}, expected {kind!r}"
            )
    if scene.get("currentProgramSceneName") != SCENE:
        raise RuntimeError(f"saved scene was not selected: {scene}")
    if profile.get("currentProfileName") != PROFILE:
        raise RuntimeError(f"saved profile was not selected: {profile}")


# gs_get_device_name() values: an API, not an adapter (obs-rust/obs-studio#929).
GRAPHICS_API_NAMES = {"OpenGL", "Direct3D 11", "Metal"}


def validate_gpu_status(status) -> None:
    """GetStats.gpuAcceleration from the GPU status light (#846, #941). Any
    state is fine on a hosted runner; the adapter must be a real one."""
    if not isinstance(status, dict):
        raise RuntimeError(f"GetStats.gpuAcceleration is missing: {status!r}")
    if status.get("state") not in ("green", "yellow", "red"):
        raise RuntimeError(f"gpuAcceleration has an unknown state: {status!r}")
    if not isinstance(status.get("detectedGpus"), list):
        raise RuntimeError(f"gpuAcceleration.detectedGpus is not a list: {status!r}")
    adapter = status.get("adapter")
    if not adapter or adapter in GRAPHICS_API_NAMES:
        raise RuntimeError(
            f"gpuAcceleration.adapter is a graphics API name, not an adapter: {adapter!r}"
        )


def require_gpu_log(log: str) -> None:
    """The frontend logs the GPU state at startup (#846)."""
    if "GPU acceleration: " not in log:
        raise RuntimeError("startup log has no 'GPU acceleration:' line")


async def wait_until_ready(client, proc: subprocess.Popen, deadline: float) -> dict:
    # Identification confirms the transport, not OBS_FRONTEND_EVENT_FINISHED_LOADING.
    # Only the protocol's explicit NotReady response is a startup condition.
    while True:
        if proc.poll() is not None:
            raise RuntimeError(
                f"packaged app exited before frontend readiness: {proc.returncode}"
            )
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise RuntimeError(
                "packaged frontend did not reach readiness within its startup deadline"
            )
        try:
            response = await asyncio.wait_for(
                client.request(
                    "GetInputKindList", {"unversioned": True}, timeout=min(15, remaining)
                ),
                timeout=remaining,
            )
        except TimeoutError as exc:
            raise RuntimeError(
                "packaged frontend did not answer GetInputKindList within its startup deadline"
            ) from exc
        result = response["d"]
        status = result["requestStatus"]
        if status["result"]:
            print("packaged frontend readiness confirmed by GetInputKindList")
            return result.get("responseData", {})
        if status["code"] != 207:
            raise RuntimeError(f"GetInputKindList failed: {status}")


def report_frontend_log(config: Path) -> None:
    """Print the newest frontend log before the fixture's cleanup removes it (#2161)."""
    logs = sorted((config / "logs").glob("*.txt"), key=lambda log: log.stat().st_mtime)
    if not logs:
        print(f"no frontend log under {config / 'logs'}")
        return
    print(f"::group::frontend log {logs[-1].name}")
    print(logs[-1].read_text(encoding="utf-8-sig", errors="replace"))
    print("::endgroup::")



DIAGNOSTIC_EXTENSIONS = (".ips", ".crash")


def diagnostic_report_directories() -> list[Path]:
    """Where macOS writes the frontend's crash reports, newest information last."""
    return [
        Path.home() / "Library/Logs/DiagnosticReports",
        Path("/Library/Logs/DiagnosticReports"),
    ]


def newest_diagnostic_reports(directory: Path, since: float) -> list[Path]:
    """Reports written after `since` (epoch seconds), newest first."""
    if not directory.is_dir():
        return []
    reports = []
    for path in directory.iterdir():
        try:
            stamp = path.stat().st_mtime
        except OSError:
            continue
        if path.is_file() and path.suffix in DIAGNOSTIC_EXTENSIONS and stamp >= since:
            reports.append((stamp, path))
    return [path for _, path in sorted(reports, reverse=True)]


def _frame_text(frame, images) -> str:
    if not isinstance(frame, dict):
        return "unknown frame"
    image = ""
    index = frame.get("imageIndex")
    if isinstance(index, int) and 0 <= index < len(images):
        entry = images[index]
        if isinstance(entry, dict) and entry.get("name"):
            image = str(entry["name"])
    symbol = str(frame.get("symbol") or "")
    text = f"{image} {symbol}".strip()
    location = frame.get("symbolLocation")
    offset = frame.get("imageOffset")
    if location is not None:
        text = f"{text} + {location}"
    elif offset is not None:
        text = f"{text} (image + {offset})".strip()
    return text or "unknown frame"


def summarize_diagnostic_report(text: str, frames: int = 10) -> list[str]:
    """The lines a lane reader needs from an Apple `.ips` crash report.

    `.ips` is two JSON documents: one header line, then the report body. A file
    that does not parse says so instead of raising - evidence must never turn
    one failure into two.
    """
    lines = text.splitlines()
    body: object = None
    if len(lines) > 1:
        try:
            body = json.loads("\n".join(lines[1:]))
        except json.JSONDecodeError:
            body = None
    if not isinstance(body, dict):
        return ["no frames parsed: the report is not the two-document .ips shape"]
    exception = body.get("exception")
    exception = exception if isinstance(exception, dict) else {}
    described = " ".join(
        str(exception[key]) for key in ("type", "signal", "subtype")
        if exception.get(key)
    )
    summary = [f"exception type: {described or 'unknown'}"]
    threads = body.get("threads")
    threads = threads if isinstance(threads, list) else []
    index = body.get("faultingThread")
    if not isinstance(index, int) or not 0 <= index < len(threads):
        summary.append("crashed thread: unknown")
        return summary
    thread = threads[index] if isinstance(threads[index], dict) else {}
    label = thread.get("name") or thread.get("queue") or ""
    summary.append(f"crashed thread: {index} {label}".rstrip())
    stacked = thread.get("frames")
    stacked = stacked if isinstance(stacked, list) else []
    images = body.get("usedImages")
    images = images if isinstance(images, list) else []
    for number, frame in enumerate(stacked[:frames]):
        summary.append(f"frame {number}: {_frame_text(frame, images)}")
    return summary


def report_crash_reports(directories, since: float, frames: int = 10) -> bool:
    """Print the newest crash report, or say plainly that none was written."""
    for directory in directories:
        reports = newest_diagnostic_reports(Path(directory), since)
        if not reports:
            continue
        report = reports[0]
        print(f"::group::shutdown evidence: {report.name}", flush=True)
        print(f"crash report: {report}", flush=True)
        try:
            text = report.read_text(encoding="utf-8", errors="replace")
        except OSError as error:
            print(f"crash report unreadable: {error}", flush=True)
        else:
            for line in summarize_diagnostic_report(text, frames):
                print(line, flush=True)
        print("::endgroup::", flush=True)
        if len(reports) > 1:
            print(
                f"{len(reports) - 1} older crash report(s) in {directory} not shown",
                flush=True,
            )
        return True
    print(
        "shutdown evidence: no crash report was written after the launch",
        flush=True,
    )
    return False


def describe_exit(returncode: int) -> str:
    """`-11` alone says nothing; name the signal when there is one."""
    if returncode < 0:
        try:
            return f"signal {-returncode} ({signal.Signals(-returncode).name})"
        except ValueError:
            return f"signal {-returncode}"
    return f"exit code {returncode}"


def _load(name: str, path: Path):
    """Import `path` as module `name` without editing sys.path."""
    import importlib.util

    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module

async def inspect_running_app(  # noqa: C901
    proc: subprocess.Popen, port: int, password: str, platform: str
) -> None:
    obsws = _load("obsws", Path(__file__).resolve().parent / "websocket-tier4" / "obsws.py")
    ObsWsClient, SUB_NONE = obsws.ObsWsClient, obsws.SUB_NONE

    deadline = time.monotonic() + 120
    client = None
    while time.monotonic() < deadline:
        if proc.poll() is not None:
            raise RuntimeError(f"packaged app exited during startup: {proc.returncode}")
        candidate = ObsWsClient(f"ws://127.0.0.1:{port}")
        try:
            hello = await asyncio.wait_for(candidate.connect(), timeout=5)
            if hello is None:
                raise RuntimeError("websocket handshake did not complete")
            await candidate.identify(
                password=password,
                auth_challenge=hello["d"]["authentication"],
                event_subscriptions=SUB_NONE,
            )
            if await candidate.wait_op(2, timeout=5) is None:
                raise RuntimeError("websocket authentication failed")
            client = candidate
            break
        except (OSError, TimeoutError):
            await candidate.close_conn()
            await asyncio.sleep(0.25)
        except BaseException:
            await candidate.close_conn()
            raise
    if client is None:
        raise RuntimeError(
            "packaged frontend did not complete startup within 120 seconds"
        )
    try:

        async def request(name, data=None):
            response = await client.request(name, data)
            result = response["d"]
            if not result["requestStatus"]["result"]:
                raise RuntimeError(f"{name} failed: {result['requestStatus']}")
            return result.get("responseData", {})

        kinds = await wait_until_ready(client, proc, deadline)
        inputs = await request("GetInputList")
        scene = await request("GetSceneList")
        profile = await request("GetProfileList")
        validate_profile(
            platform, kinds["inputKinds"], inputs["inputs"], scene, profile
        )
        stats = await request("GetStats")
        validate_gpu_status(stats.get("gpuAcceleration"))
        print(f"GPU status: {json.dumps(stats['gpuAcceleration'])}")
        items = await request("GetSceneItemList", {"sceneName": SCENE})
        if set(NAMES) - {item["sourceName"] for item in items["sceneItems"]}:
            raise RuntimeError("saved scene lost its capture source items")
        # Exercise the real screenshot caller affected by the prerequisite
        # websocket fix, including the packaged Qt image codecs.
        for format_, mime, signature in (
            ("png", "image/png", b"\x89PNG\r\n\x1a\n"),
            ("jpg", "image/jpeg", b"\xff\xd8\xff"),
        ):
            screenshot = await request(
                "GetSourceScreenshot",
                {
                    "sourceName": SCENE,
                    "imageFormat": format_,
                    "imageWidth": 64,
                    "imageHeight": 64,
                },
            )
            prefix, encoded = screenshot["imageData"].split(",", 1)
            if prefix != f"data:{mime};base64" or not base64.b64decode(
                encoded, validate=True
            ).startswith(signature):
                raise RuntimeError(
                    f"packaged {format_} screenshot has the wrong MIME type or image signature"
                )
    finally:
        await client.close_conn()


def close_frontend(  # noqa: C901
    proc: subprocess.Popen, platform: str, since: float | None = None
) -> None:
    since = time.time() - 600 if since is None else since
    if platform == "macos":
        proc.send_signal(signal.SIGINT)
    else:
        import ctypes
        from ctypes import wintypes

        user32 = ctypes.WinDLL("user32", use_last_error=True)
        callback_type = ctypes.WINFUNCTYPE(
            wintypes.BOOL, wintypes.HWND, wintypes.LPARAM
        )
        user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
        user32.GetWindowThreadProcessId.argtypes = [
            wintypes.HWND,
            ctypes.POINTER(wintypes.DWORD),
        ]
        user32.IsWindowVisible.argtypes = [wintypes.HWND]
        user32.PostMessageW.argtypes = [
            wintypes.HWND,
            wintypes.UINT,
            wintypes.WPARAM,
            wintypes.LPARAM,
        ]
        windows = []
        observed = []

        @callback_type
        def owned_window(hwnd, _):
            pid = wintypes.DWORD()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            if pid.value == proc.pid:
                visible = bool(user32.IsWindowVisible(hwnd))
                observed.append((hwnd, visible))
                if visible:
                    windows.append(hwnd)
            return True

        deadline = time.monotonic() + 30
        while True:
            windows.clear()
            observed.clear()
            if not user32.EnumWindows(owned_window, 0):
                raise ctypes.WinError(ctypes.get_last_error())
            if windows:
                break
            if proc.poll() is not None:
                raise RuntimeError(
                    f"packaged app exited before window discovery: {proc.returncode}"
                )
            if time.monotonic() >= deadline:
                raise RuntimeError(
                    "cannot find the owned frontend window for clean shutdown "
                    f"within 30 seconds (pid={proc.pid}, owned windows={observed})"
                )
            time.sleep(0.05)
        for hwnd in windows:
            if not user32.PostMessageW(
                hwnd, 0x0010, 0, 0
            ):  # WM_CLOSE, like the window's close button.
                raise ctypes.WinError(ctypes.get_last_error())
    try:
        result = proc.wait(timeout=30)
    except subprocess.TimeoutExpired:
        if platform == "macos" and proc.poll() is None:
            # Capture the owned live process before cleanup terminates it. Keep
            # the timeout fatal; a stack is evidence, not a successful shutdown.
            print(
                f"::group::shutdown evidence: sample of pid {proc.pid} after the 30 s timeout",
                flush=True,
            )
            try:
                sample = subprocess.run(
                    ["sample", str(proc.pid), "5", "-file", "/dev/stdout"],
                    capture_output=True,
                    text=True,
                    timeout=15,
                    check=False,
                )
                print(sample.stdout, flush=True)
                print(sample.stderr, flush=True)
            except (OSError, subprocess.TimeoutExpired) as error:
                print(f"shutdown sample unavailable: {error}", flush=True)
            print("::endgroup::", flush=True)
            report_crash_reports(diagnostic_report_directories(), since)
        raise
    if result != 0:
        # #2714: a bare `-11` named no frame; the report is on the run, so put
        # the part a reader needs into the step output before failing.
        print(f"shutdown evidence: frontend exited with {describe_exit(result)}", flush=True)
        if platform == "macos":
            report_crash_reports(diagnostic_report_directories(), since)
        raise RuntimeError(
            f"packaged frontend failed during shutdown: {describe_exit(result)}"
        )


def main() -> None:  # noqa: C901
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True)
    parser.add_argument("--config-dir", type=Path, required=True)
    parser.add_argument("--platform", choices=KINDS, required=True)
    parser.add_argument("--expect-arch", choices=("x64", "arm64"))
    parser.add_argument(
        "--config-mode",
        choices=("fresh", "migrate-default", "migrate-custom", "existing"),
        default="fresh",
    )
    args = parser.parse_args()
    app = args.app.resolve()
    if args.platform == "macos":
        app = app / "Contents/MacOS/OBS-Studio-Rust"
    if not app.is_file():
        raise RuntimeError(f"packaged executable is missing: {app}")
    if args.platform == "windows":
        if args.expect_arch is None:
            raise RuntimeError("Windows packages require --expect-arch")
        require_windows_arch(app, args.expect_arch)
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    password = secrets.token_hex(24)
    config = args.config_dir.resolve()
    require_config_directory(config, args.platform)
    # Ownership is established before launching: never delete an existing config.
    fixture = ConfigFixture(config, args.platform, args.config_mode, port, password)
    proc = None
    succeeded = False
    launched = time.time()
    try:
        proc = subprocess.Popen(
            [
                str(app),
                "--disable-shutdown-check",
                "--disable-updater",
                "--profile",
                PROFILE,
                "--collection",
                PROFILE,
                "--scene",
                SCENE,
            ],
            cwd=app.parent,
        )
        asyncio.run(inspect_running_app(proc, port, password, args.platform))
        close_frontend(proc, args.platform, launched)
        fixture.verify()
        logs = sorted((config / "logs").glob("*.txt"), key=lambda log: log.stat().st_mtime)
        current_logs = [
            log.read_text(encoding="utf-8-sig", errors="replace") for log in logs
        ]
        if not current_logs:
            raise RuntimeError("packaged frontend wrote no log")
        require_gpu_log(current_logs[-1])
        if args.platform == "windows":
            identity_logs = [
                log
                for log in current_logs
                if "Windows AppUserModelID verified: io.github.obs_rust.OBSStudioRust"
                in log
            ]
            if not identity_logs:
                raise RuntimeError(
                    "packaged frontend did not verify its Windows application identity"
                )
            require_windows_modules(identity_logs[-1])
        print(
            "packaged startup: capture plugins registered, saved profile loaded, clean shutdown"
        )
        succeeded = True
    finally:
        if proc is not None and proc.poll() is None:
            try:
                close_frontend(proc, args.platform, launched)
            except (RuntimeError, OSError, subprocess.TimeoutExpired):
                if proc.poll() is None:
                    proc.terminate()
                    try:
                        proc.wait(timeout=10)
                    except subprocess.TimeoutExpired:
                        proc.kill()
                        proc.wait(timeout=10)
        if not succeeded:
            report_frontend_log(config)
        fixture.cleanup()


if __name__ == "__main__":
    main()
