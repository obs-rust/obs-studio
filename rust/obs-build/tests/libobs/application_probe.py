#!/usr/bin/env -S uv run --script
"""Run a staged frontend with an owned scene/profile on an isolated X display."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import select
import subprocess
import time
import uuid


def profile(runtime: Path) -> Path:
    config = runtime / "user-config/obs-studio-rust"
    config.mkdir(parents=True)
    (config / "global.ini").write_text("[General]\nLastVersion=4294967295\n")
    (config / "user.ini").write_text(
        "[General]\nFirstRun=true\nConfirmOnExit=false\n"
        "[Basic]\nProfile=NativeSmoke\nProfileDir=NativeSmoke\n"
        "SceneCollection=NativeSmoke\nSceneCollectionFile=NativeSmoke\n"
    )
    selected = config / "basic/profiles/NativeSmoke"
    selected.mkdir(parents=True)
    (selected / "basic.ini").write_text(
        "[General]\nName=NativeSmoke\n[Video]\nBaseCX=640\nBaseCY=360\n"
        "OutputCX=640\nOutputCY=360\nFPSType=0\nFPSCommon=30\n"
        "[Audio]\nSampleRate=48000\nChannelSetup=Stereo\n"
    )
    if os.environ.get("OBS_BUILD_APP_RECORD"):
        recordings = runtime / "recordings"
        recordings.mkdir()
        encoder = "nvenc" if os.environ.get("OBS_BUILD_NVENC") else "x264"
        with (selected / "basic.ini").open("a") as output:
            output.write(
                "[Output]\nMode=Simple\n[SimpleOutput]\n"
                f"FilePath={recordings}\nRecFormat2=mkv\nRecQuality=Small\n"
                f"RecEncoder={encoder}\nRecAudioEncoder=aac\nRecTracks=1\n"
                "StreamAudioEncoder=aac\nABitrate=128\nVBitrate=1000\n"
            )
    color = {"name": "Native white", "id": "color_source_v3", "uuid": str(uuid.uuid4()),
             "settings": {"width": 640, "height": 360, "color": 0xFFFFFFFF}, "volume": 1.0}
    scene = {"name": "Native scene", "id": "scene", "uuid": str(uuid.uuid4()),
             "settings": {"items": [{"name": color["name"], "source_uuid": color["uuid"], "visible": True}]}}
    scenes = config / "basic/scenes"
    scenes.mkdir(parents=True)
    (scenes / "NativeSmoke.json").write_text(json.dumps({
        "name": "NativeSmoke", "current_scene": "Native scene", "current_program_scene": "Native scene",
        "scene_order": [{"name": "Native scene"}], "sources": [color, scene],
    }))
    return config


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--inspector", type=Path, required=True)
    args = parser.parse_args()
    runtime = args.runtime.resolve()
    config = profile(runtime)
    env = dict(os.environ)
    env.update(XDG_CONFIG_HOME=str(runtime / "user-config"), XDG_DATA_HOME=str(runtime / "user-data"),
               XDG_CACHE_HOME=str(runtime / "user-cache"), QT_QPA_PLATFORM="xcb",
               QT_QPA_PLATFORMTHEME="", QT_STYLE_OVERRIDE="Fusion")
    # Desktop-session paths may select plugins from a different Qt installation.
    # Use the linked Qt library's own default plugin paths for this isolated test.
    env.pop("QT_PLUGIN_PATH", None)
    env.pop("QT_QPA_PLATFORM_PLUGIN_PATH", None)
    env["LD_LIBRARY_PATH"] = str(runtime / "lib64") + os.pathsep + env.get("LD_LIBRARY_PATH", "")
    env.pop("OBS_DATA_PATH", None)
    env.pop("WAYLAND_DISPLAY", None)
    recording = bool(os.environ.get("OBS_BUILD_APP_RECORD"))
    if os.environ.get("OBS_BUILD_NVENC"):
        assert (runtime / "bin/obs-nvenc-test").is_file(), "NVENC capability helper was not staged"
    command = [str(runtime / "bin/obs-studio-rust"), "--multi", "--only-bundled-plugins",
               "--disable-shutdown-check", "--disable-updater", "--disable-missing-files-check", "--unfiltered_log"]
    if recording:
        command.append("--startrecording")
    app = subprocess.Popen(command, env=env, cwd=runtime, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    log = bytearray()
    drained = False
    try:
        deadline = time.monotonic() + 45
        ready = False
        while time.monotonic() < deadline:
            if app.poll() is not None:
                raise RuntimeError(f"frontend exited during startup: {app.returncode}")
            if select.select([app.stdout], [], [], 0.2)[0]:
                chunk = os.read(app.stdout.fileno(), 65536)
                log.extend(chunk)
                (runtime / "application.log").write_bytes(log)
            if b"==== Startup complete" in log and (not recording or b"==== Recording Start" in log):
                found = subprocess.run([str(args.inspector), "find", str(app.pid)], env=env, capture_output=True)
                if found.returncode == 0:
                    ready = True
                    break
        if not ready:
            raise RuntimeError("frontend did not finish startup and map a normal main window")
        maps = Path(f"/proc/{app.pid}/maps").read_text()
        (runtime / "application.maps").write_text(maps)
        for name in ["libobs.so.30", "libobs-frontend-api.so.30", "libobs-opengl.so.33"]:
            assert str(runtime / "lib64" / name) in maps, f"staged {name} was not loaded"
        for name in json.loads((runtime / "expected-modules.json").read_text()):
            assert str(runtime / "lib64/obs-modules/core" / (name + ".so")) in maps, f"module {name} was not loaded"
        if os.environ.get("OBS_BUILD_NVENC"):
            assert str(runtime / "lib64/obs-modules/core/obs-nvenc.so") in maps, "staged NVENC was not loaded"
        assert b"Native white" in log, "saved color source was not restored"
        if recording:
            # Keep the actual frontend output running long enough to decode a
            # meaningful recording. Window-close must finalize the mux helper.
            time.sleep(3)
        subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "x11grab", "-video_size", "1280x720",
                        "-i", env["DISPLAY"], "-frames:v", "1", str(runtime / "application.png")], env=env,
                       check=True, capture_output=True, timeout=15)
        subprocess.run([str(args.inspector), "close", str(app.pid)], env=env, check=True, capture_output=True)
        remainder, _ = app.communicate(timeout=30)
        log.extend(remainder)
        drained = True
        (runtime / "application.log").write_bytes(log)
        assert app.returncode == 0, f"frontend shutdown failed: {app.returncode}"
        assert b"==== Shutting down" in log, "frontend did not enter normal shutdown"
        saved = json.loads((config / "basic/scenes/NativeSmoke.json").read_text())
        assert any(source.get("name") == "Native white" for source in saved["sources"]), "saved scene was lost"
        if recording:
            if os.environ.get("OBS_BUILD_NVENC"):
                assert b"[obs-nvenc: 'simple_video_recording'] settings:" in log, "frontend did not use NVENC"
            recordings = list((runtime / "recordings").glob("*.mkv"))
            assert len(recordings) == 1, "frontend did not create the requested recording"
            verify_recording(recordings[0], env)
        print(f"OBS normal window, staged libraries/modules, saved scene, screenshot and clean shutdown: {runtime}")
    finally:
        if not drained:
            if app.poll() is None:
                app.terminate()
            try:
                remainder, _ = app.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                app.kill()
                remainder, _ = app.communicate(timeout=5)
            log.extend(remainder)
        (runtime / "application.log").write_bytes(log)


def verify_recording(path: Path, env: dict[str, str]) -> None:
    result = subprocess.run(
        ["ffprobe", "-v", "error", "-count_frames", "-show_streams", "-show_format", "-of", "json", str(path)],
        env=env, check=True, capture_output=True, timeout=15,
    )
    report = json.loads(result.stdout)
    path.with_suffix(".ffprobe.json").write_text(json.dumps(report, indent=2))
    video = next(stream for stream in report["streams"] if stream["codec_type"] == "video")
    audio = next(stream for stream in report["streams"] if stream["codec_type"] == "audio")
    assert video["codec_name"] == "h264" and (video["width"], video["height"]) == (640, 360)
    assert int(video["nb_read_frames"]) >= 60, "frontend recording has too few decoded frames"
    assert audio["codec_name"] == "aac" and int(audio["sample_rate"]) == 48000 and audio["channels"] == 2
    assert 2 <= float(report["format"]["duration"]) <= 15, "unexpected frontend recording duration"
    frame = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", str(path), "-vf", "scale=1:1", "-frames:v", "1",
         "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"],
        env=env, check=True, capture_output=True, timeout=15,
    ).stdout
    assert len(frame) == 3 and min(frame) >= 240, "frontend did not record the restored white source"
    print(f"Validated actual frontend H264/AAC recording and white frame: {path}")


if __name__ == "__main__":
    main()
