#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["numpy==2.3.4", "av==16.0.1", "websockets==15.0.1"]
# ///
"""A/V sync of the Linux Virtual Camera and its OBS Virtual Microphone.

Drives a running OBS over obs-websocket (no clicks, no computer use):

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


async def measure(args: argparse.Namespace) -> dict:
    import websockets

    work = Path(tempfile.mkdtemp(prefix="vcam-av-sync-"))
    clip = work / "probe.mkv"
    write_clip(clip)
    capture = work / "capture.mkv"
    async with websockets.connect(args.url, subprotocols=["obswebsocket.json"]) as ws:
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
    result = analyze(capture, EVENTS, restarted)
    result["capture"] = str(capture)
    return result


async def video_probe(args: argparse.Namespace) -> dict:
    """Render-to-device latency of the Virtual Camera alone: a full-canvas
    white color source is shown and hidden at known times, so no media
    decoding is in the path (#4119)."""
    import av
    import websockets

    work = Path(tempfile.mkdtemp(prefix="vcam-video-probe-"))
    capture = work / "video.mkv"
    toggles: list[tuple[float, bool]] = []
    async with websockets.connect(args.url, subprotocols=["obswebsocket.json"]) as ws:
        await ws.recv()
        await ws.send(json.dumps({"op": 1, "d": {"rpcVersion": 1}}))
        await ws.recv()
        video = await obs_request(ws, "GetVideoSettings")
        scene = (await obs_request(ws, "GetCurrentProgramScene"))["currentProgramSceneName"]
        if not (await obs_request(ws, "GetVirtualCamStatus"))["outputActive"]:
            await obs_request(ws, "StartVirtualCam")
            await asyncio.sleep(2)
        created = await obs_request(ws, "CreateInput", {
            "sceneName": scene, "inputName": SOURCE_NAME, "inputKind": "color_source_v3",
            "inputSettings": {"color": 0xFFFFFFFF, "width": video["baseWidth"], "height": video["baseHeight"]},
            "sceneItemEnabled": False})
        item = created["sceneItemId"]
        try:
            recorder = subprocess.Popen(
                ["ffmpeg", "-loglevel", "error", "-y", "-f", "v4l2", "-ts", "abs", "-i", args.device,
                 "-map", "0:v", "-c:v", "ffv1", "-copyts", str(capture)])
            await asyncio.sleep(1.5)
            for i in range(2 * args.toggles):
                on = i % 2 == 0
                sent = time.time()
                await obs_request(ws, "SetSceneItemEnabled",
                                  {"sceneName": scene, "sceneItemId": item, "sceneItemEnabled": on})
                toggles.append(((sent + time.time()) / 2, on))
                await asyncio.sleep(0.7)
            recorder.send_signal(signal.SIGINT)
            await asyncio.to_thread(recorder.wait)
        finally:
            await obs_request(ws, "RemoveInput", {"inputName": SOURCE_NAME})
    times, luma = [], []
    with av.open(str(capture)) as container:
        stream = container.streams.video[0]
        for frame in container.decode(stream):
            if frame.pts is not None:
                times.append(float(frame.pts * stream.time_base))
                luma.append(float(frame.to_ndarray(format="gray").mean()))
    mid = (min(luma) + max(luma)) / 2
    lat = []
    for t, on in toggles:
        edge = next((ft for ft, lv in zip(times, luma) if ft > t and (lv > mid) == on), None)
        if edge is not None:
            lat.append(round((edge - t) * 1000, 1))
    fps = video["fpsNumerator"] / video["fpsDenominator"]
    return {"fps": fps, "frame_ms": round(1000 / fps, 1), "video_latency_ms": lat,
            "median_ms": float(np.median(lat)) if lat else None, "capture": str(capture)}


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
    v = sub.add_parser("video-probe", help="render-to-device latency, no media source")
    v.add_argument("--url", default="ws://127.0.0.1:4455")
    v.add_argument("--device", default="/dev/video9")
    v.add_argument("--toggles", type=int, default=5)
    args = parser.parse_args(argv)
    if args.cmd == "video-probe":
        print(json.dumps(asyncio.run(video_probe(args))))
        return 0
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
