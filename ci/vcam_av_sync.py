#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["numpy==2.3.4", "av==16.0.1", "websockets==15.0.1"]
# ///
"""A/V sync of the Linux Virtual Camera and its OBS Virtual Microphone.

Drives OBS over a private MCP socket or obs-websocket (no clicks, no computer use):

    uv run --script ci/vcam_av_sync.py measure [--seconds 12] [--url ws://127.0.0.1:4455]
    uv run --script ci/vcam_av_sync.py analyze <capture.mkv> <clip events json>
    uv run --script ci/vcam_av_sync.py clip <out.mkv>

`measure`:
1. Writes a probe clip in which a white flash and a 1 kHz beep start on the
   same frame, at irregular times, so one lag explains them unambiguously.
2. Adds the clip to the program scene as a temporary media source, stretched
   over the canvas, and starts the Virtual Camera if it is off.
3. Records the Virtual Camera device and the virtual microphone in a single
   ffmpeg process, both stamped with the wall clock.
4. Always removes the temporary source again.
5. Prints JSON with av_offset_ms (audio minus video; positive means the audio
   is late) and each path's latency from the moment the clip was restarted.

Everything a user would hear or see in an app reading the devices plays
during the capture: run it only when nobody is listening.

obs-rust/obs-studio#4087 (virtual microphone lag).
"""
from __future__ import annotations

import argparse
import asyncio
import json
import os
import stat
import uuid
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np

FPS = 30
RATE = 48000
BEEP_HZ = 1000.0
PULSE_MS = 100
# Irregular gaps, so the event train has a single best alignment.
EVENTS = [0.50, 1.30, 2.90, 3.60, 5.40, 6.10, 7.85, 8.40, 9.90, 10.55]
CLIP_SECONDS = 12.0
SOURCE_NAME = "vcam-av-sync-probe"


def write_clip(path: Path) -> list[float]:
    """The probe clip: black video and silence, with flash+beep events."""
    flash = "+".join(f"between(t,{t:.3f},{t + PULSE_MS / 1000:.3f})" for t in EVENTS)
    video = (
        f"color=c=black:s=640x360:r={FPS}:d={CLIP_SECONDS},"
        f"drawbox=x=0:y=0:w=iw:h=ih:color=white:t=fill:enable='{flash}'"
    )
    audio = (
        f"sine=f={BEEP_HZ}:r={RATE}:d={CLIP_SECONDS},"
        f"volume=0.5,volume=0:enable='not({flash})'"
    )
    subprocess.run(
        ["ffmpeg", "-loglevel", "error", "-y", "-f", "lavfi", "-i", video, "-f", "lavfi", "-i", audio,
         "-c:v", "libx264", "-preset", "ultrafast", "-g", str(FPS), "-pix_fmt", "yuv420p",
         "-c:a", "pcm_s16le", "-ac", "2", str(path)],
        check=True,
    )
    return EVENTS


def onsets(times: np.ndarray, level: np.ndarray, threshold: float, min_gap: float = 0.25) -> list[float]:
    """Rising edges of level through threshold, at least min_gap apart."""
    out: list[float] = []
    above = level > threshold
    for i in range(1, len(level)):
        if above[i] and not above[i - 1] and (not out or times[i] - out[-1] > min_gap):
            out.append(float(times[i]))
    return out


def read_capture(path: Path) -> tuple[list[float], list[float]]:
    """Flash and beep onsets (seconds, shared wall clock) in a capture."""
    import av

    flashes_t, luma = [], []
    beep_t: list[np.ndarray] = []
    beep_e: list[np.ndarray] = []
    with av.open(str(path)) as container:
        vstream = container.streams.video[0]
        for frame in container.decode(vstream):
            if frame.pts is None:
                continue
            flashes_t.append(float(frame.pts * vstream.time_base))
            luma.append(float(frame.to_ndarray(format="gray").mean()))
    with av.open(str(path)) as container:
        astream = container.streams.audio[0]
        for frame in container.decode(astream):
            if frame.pts is None:
                continue
            data = frame.to_ndarray().astype(np.float64)
            mono = data.reshape(-1) if data.shape[0] == 1 else data.mean(axis=0)
            if frame.format.is_packed and data.shape[0] == 1:
                mono = data.reshape(-1, frame.layout.nb_channels).mean(axis=1)
            start = float(frame.pts * astream.time_base)
            # 1 ms windows of energy near BEEP_HZ: correlate with a 1 kHz carrier.
            win = frame.sample_rate // 1000
            n = len(mono) // win
            if n == 0:
                continue
            t = np.arange(n * win) / frame.sample_rate
            carrier = np.exp(-2j * np.pi * BEEP_HZ * (t + start))
            band = np.abs((mono[: n * win] * carrier).reshape(n, win).sum(axis=1)) / win
            beep_t.append(start + np.arange(n) / 1000.0)
            beep_e.append(band)
    vt, vl = np.array(flashes_t), np.array(luma)
    at, ae = np.concatenate(beep_t), np.concatenate(beep_e)
    order = np.argsort(at)
    at, ae = at[order], ae[order]
    flashes = onsets(vt, vl, (vl.min() + vl.max()) / 2)
    beeps = onsets(at, ae, ae.max() / 4)
    return flashes, beeps


def align(observed: list[float], events: list[float], period: float) -> tuple[float, int]:
    """Lag (observed minus clip time) that matches the most events, and the count."""
    if not observed:
        return float("nan"), 0
    train = [e + k * period for k in range(-1, 4) for e in events]
    best = (float("nan"), 0, float("inf"))
    for o in observed:
        for e in train:
            lag = o - e
            hits = [min(abs(x - (t + lag)) for t in train) for x in observed]
            matched = sum(h < 0.05 for h in hits)
            err = float(np.median(hits))
            if matched > best[1] or (matched == best[1] and err < best[2]):
                best = (lag, matched, err)
    lag, matched, _ = best
    # Refine: mean residual of the matched events.
    res = [x - (min(train, key=lambda t: abs(x - (t + lag))) + lag) for x in observed]
    res = [r for r in res if abs(r) < 0.05]
    return lag + (float(np.median(res)) if res else 0.0), matched


async def obs_request(ws, kind: str, data: dict | None = None) -> dict:
    await ws.send(json.dumps({"op": 6, "d": {"requestType": kind, "requestId": kind, "requestData": data or {}}}))
    while True:
        msg = json.loads(await ws.recv())
        if msg["op"] == 7 and msg["d"]["requestId"] == kind:
            status = msg["d"]["requestStatus"]
            if not status["result"]:
                raise RuntimeError(f"{kind}: {status}")
            return msg["d"].get("responseData") or {}


class McpSession:
    """Bounded newline JSON-RPC exchange over an existing OBS socket."""

    def __init__(self, reader, writer):
        self.reader, self.writer = reader, writer
        self.next_id = 0

    async def rpc(self, method: str, params: dict) -> dict:
        self.next_id += 1
        request = {"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}
        self.writer.write(json.dumps(request).encode() + b"\n")
        await self.writer.drain()
        line = await asyncio.wait_for(self.reader.readline(), timeout=310)
        if not line:
            raise RuntimeError("MCP socket closed before response")
        reply = json.loads(line)
        if reply.get("jsonrpc") != "2.0" or reply.get("id") != self.next_id:
            raise RuntimeError("MCP response version/ID mismatch")
        if "error" in reply:
            raise RuntimeError(f"MCP protocol error: {reply['error']}")
        if "result" not in reply:
            raise RuntimeError("MCP response has no result")
        return reply["result"]

    async def call(self, name: str, arguments: dict | None = None) -> dict:
        result = await self.rpc("tools/call", {"name": name, "arguments": arguments or {}})
        if result.get("isError"):
            raise RuntimeError(f"{name}: {result.get('structuredContent', result)}")
        if "structuredContent" not in result:
            raise RuntimeError(f"{name}: missing structuredContent")
        return result["structuredContent"]


async def connect_mcp(socket_path: str | None):
    """Attach without spawning a GUI or a second OBS."""
    if socket_path is None:
        runtime = os.environ.get("XDG_RUNTIME_DIR")
        if not runtime:
            raise RuntimeError("XDG_RUNTIME_DIR is unset; use --mcp-socket")
        socket_path = str(Path(runtime) / "obs-studio-rust/control.sock")
    metadata = Path(socket_path).lstat()
    if not stat.S_ISSOCK(metadata.st_mode) or metadata.st_uid != os.geteuid() or metadata.st_mode & 0o077:
        raise RuntimeError("MCP endpoint must be a private socket owned by this user")
    reader, writer = await asyncio.open_unix_connection(socket_path, limit=32 * 1024 * 1024)
    session = McpSession(reader, writer)
    try:
        await session.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                      "clientInfo": {"name": "vcam-av-sync", "version": "1"}})
        writer.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}).encode() + b"\n")
        await writer.drain()
        return session
    except BaseException:
        writer.close()
        await writer.wait_closed()
        raise


async def stop_capture(recorder) -> None:
    """Stop and reap a capture even when a device ignores SIGINT."""
    if recorder.poll() is not None:
        return
    recorder.send_signal(signal.SIGINT)
    try:
        await asyncio.to_thread(recorder.wait, timeout=10)
    except subprocess.TimeoutExpired:
        recorder.kill()
        await asyncio.to_thread(recorder.wait, timeout=5)


async def measure_mcp(args: argparse.Namespace, clip: Path, capture: Path) -> dict:
    session = await connect_mcp(args.mcp_socket)
    recorder = None
    created = False
    # A unique name prevents modifying or removing a user's existing source.
    name = f"{SOURCE_NAME}-{uuid.uuid4().hex[:12]}"
    try:
        scene = (await session.call("scene_get_current"))["name"]
        if not scene:
            scene = f"vcam-av-sync-scene-{uuid.uuid4().hex[:12]}"
            await session.call("scene_create", {"name": scene})
        video = await session.call("screenshot", {"source": scene})
        await session.call("vcam_start", {"wait": True, "timeout_ms": 5000, "audio_enabled": True})
        mic = await session.call("vmic_status")
        if mic.get("active") is not True:
            raise RuntimeError(f"virtual microphone is not active: {mic}")
        await session.call("source_create", {"scene": scene, "kind": "ffmpeg_source", "name": name,
                           "settings": {"is_local_file": True, "local_file": str(clip), "looping": True,
                                        "restart_on_activate": True, "close_when_inactive": False}})
        created = True
        await session.call("scene_item_set_transform", {"scene": scene, "source": name,
                           "position": {"x": 0.0, "y": 0.0},
                           "scale": {"x": video["width"] / 640.0, "y": video["height"] / 360.0}})
        state = await session.call("wait_for", {"condition": {"type": "media_state", "name": name, "state": "playing"},
                                               "timeout_ms": 10000})
        if not state["satisfied"]:
            raise RuntimeError(f"probe did not start: {state}")
        await session.call("stats_reset")
        recorder = subprocess.Popen(
            ["ffmpeg", "-loglevel", "error", "-y", "-f", "v4l2", "-ts", "abs", "-i", args.device,
             "-f", "pulse", "-wallclock", "1", "-fragment_size", "1920", "-i", args.source,
             "-map", "0:v", "-map", "1:a", "-c:v", "ffv1", "-c:a", "pcm_f32le", "-copyts", str(capture)])
        # Capture must already be listening before the known media timeline begins.
        await asyncio.sleep(1.0)
        await session.call("media_restart", {"name": name})
        restarted = time.time()
        await asyncio.sleep(args.seconds)
        await stop_capture(recorder)
        if recorder.returncode not in (0, 255):
            raise RuntimeError(f"ffmpeg capture failed: {recorder.returncode}")
        snapshot = await session.call("stats_get")
    finally:
        original_error = sys.exception()
        try:
            if created:
                await session.call("source_remove", {"name": name})
        except Exception as cleanup_error:
            if original_error is None:
                raise
            original_error.add_note(f"probe cleanup also failed: {cleanup_error}")
        finally:
            try:
                if recorder is not None:
                    await stop_capture(recorder)
            finally:
                session.writer.close()
                await session.writer.wait_closed()
    result = analyze(capture, EVENTS, restarted)
    result.update({"capture": str(capture), "transport": "mcp", "stats": snapshot,
                   "measured_from": "independent ffmpeg v4l2/PulseAudio capture of flash/beep probe"})
    return result


async def measure(args: argparse.Namespace) -> dict:
    work = Path(tempfile.mkdtemp(prefix="vcam-av-sync-"))
    clip = work / "probe.mkv"
    write_clip(clip)
    capture = work / "capture.mkv"
    if args.transport == "mcp":
        return await measure_mcp(args, clip, capture)
    import websockets
    try:
        ws = await websockets.connect(args.url, subprotocols=["obswebsocket.json"], open_timeout=5)
    except (OSError, TimeoutError):
        if args.transport == "auto":
            return await measure_mcp(args, clip, capture)
        raise
    try:
        await ws.recv()
        await ws.send(json.dumps({"op": 1, "d": {"rpcVersion": 1}}))
        await ws.recv()
        video = await obs_request(ws, "GetVideoSettings")
        scene = (await obs_request(ws, "GetCurrentProgramScene"))["currentProgramSceneName"]
        if not (await obs_request(ws, "GetVirtualCamStatus"))["outputActive"]:
            await obs_request(ws, "StartVirtualCam")
            await asyncio.sleep(2)
        created = await obs_request(ws, "CreateInput", {
            "sceneName": scene, "inputName": SOURCE_NAME, "inputKind": "ffmpeg_source",
            "inputSettings": {"is_local_file": True, "local_file": str(clip), "looping": True,
                              "restart_on_activate": True, "close_when_inactive": False},
            "sceneItemEnabled": True})
        try:
            await obs_request(ws, "SetSceneItemTransform", {
                "sceneName": scene, "sceneItemId": created["sceneItemId"],
                "sceneItemTransform": {"positionX": 0, "positionY": 0, "boundsType": "OBS_BOUNDS_STRETCH",
                                       "boundsWidth": video["baseWidth"], "boundsHeight": video["baseHeight"]}})
            await asyncio.sleep(1.5)
            recorder = subprocess.Popen(
                ["ffmpeg", "-loglevel", "error", "-y",
                 "-f", "v4l2", "-ts", "abs", "-i", args.device,
                 "-f", "pulse", "-wallclock", "1", "-fragment_size", "1920", "-i", args.source,
                 # -copyts keeps wall-clock stamps, so -t cannot bound the
                 # run: stop it with SIGINT instead.
                 "-map", "0:v", "-map", "1:a", "-c:v", "ffv1", "-c:a", "pcm_f32le",
                 "-copyts", str(capture)])
            await asyncio.sleep(1.0)
            await obs_request(ws, "TriggerMediaInputAction", {
                "inputName": SOURCE_NAME, "mediaAction": "OBS_WEBSOCKET_MEDIA_INPUT_ACTION_RESTART"})
            restarted = time.time()
            await asyncio.sleep(args.seconds)
            recorder.send_signal(signal.SIGINT)
            await asyncio.to_thread(recorder.wait)
        finally:
            await obs_request(ws, "RemoveInput", {"inputName": SOURCE_NAME})
    finally:
        await ws.close()
    result = analyze(capture, EVENTS, restarted)
    result["transport"] = "websocket"
    result["capture"] = str(capture)
    return result


def analyze(capture: Path, events: list[float], restarted: float | None = None) -> dict:
    flashes, beeps = read_capture(capture)
    vlag, vmatched = align(flashes, events, CLIP_SECONDS)
    alag, amatched = align(beeps, events, CLIP_SECONDS)
    result = {
        "flashes": len(flashes), "beeps": len(beeps),
        "video_matched": vmatched, "audio_matched": amatched,
        # The clip loops, so each lag is known only up to whole periods.
        "av_offset_ms": round(((alag - vlag + CLIP_SECONDS / 2) % CLIP_SECONDS - CLIP_SECONDS / 2) * 1000, 1),
    }
    if restarted is not None:
        result["video_latency_ms"] = round(((vlag - restarted) % CLIP_SECONDS) * 1000, 1)
        result["audio_latency_ms"] = round(((alag - restarted) % CLIP_SECONDS) * 1000, 1)
    return result


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)
    m = sub.add_parser("measure")
    m.add_argument("--transport", choices=("auto", "websocket", "mcp"), default="auto")
    m.add_argument("--mcp-socket", default=None)
    m.add_argument("--url", default="ws://127.0.0.1:4455")
    m.add_argument("--device", default="/dev/video9")
    m.add_argument("--source", default="obs_virtual_microphone")
    m.add_argument("--seconds", type=float, default=12.0)
    m.add_argument("--max-offset-ms", type=float, default=None,
                   help="exit 1 when |av_offset_ms| is larger")
    a = sub.add_parser("analyze")
    a.add_argument("capture", type=Path)
    c = sub.add_parser("clip")
    c.add_argument("out", type=Path)
    args = parser.parse_args(argv)
    if args.cmd == "clip":
        write_clip(args.out)
        print(json.dumps({"clip": str(args.out), "events": EVENTS}))
        return 0
    if args.cmd == "analyze":
        print(json.dumps(analyze(args.capture, EVENTS)))
        return 0
    result = asyncio.run(measure(args))
    print(json.dumps(result))
    if args.max_offset_ms is not None and not abs(result["av_offset_ms"]) <= args.max_offset_ms:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
