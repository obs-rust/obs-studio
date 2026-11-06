#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["numpy==2.3.4", "sounddevice==0.5.3"]
# ///
"""Virtual camera audio on real Windows and macOS runners (#3183, #3184).

`profile` writes a saved profile whose scene plays a 440 Hz tone and whose
Virtual Camera sends track 1 to the virtual microphone. `record` captures the
virtual device and fails unless the tone comes out of it. `devices` lists
what PortAudio sees.
"""
from __future__ import annotations

import argparse
import json
import math
import struct
import sys
import wave
from pathlib import Path

RATE = 48000
TONE_HZ = 440.0


def write_tone(path: Path, seconds: float = 2.0) -> None:
    frames = int(RATE * seconds)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(RATE)
        data = bytearray()
        for i in range(frames):
            s = int(12000 * math.sin(2 * math.pi * TONE_HZ * i / RATE))
            data += struct.pack("<hh", s, s)
        w.writeframes(bytes(data))


def write_profile(root: Path, tone: Path, device: str) -> None:
    profile = root / "basic" / "profiles" / "Test"
    scenes = root / "basic" / "scenes"
    profile.mkdir(parents=True, exist_ok=True)
    scenes.mkdir(parents=True, exist_ok=True)
    (root / "global.ini").write_text("[General]\nLastVersion=4294967295\n")
    (root / "user.ini").write_text(
        "[General]\nFirstRun=true\nConfirmOnExit=false\n\n"
        "[Basic]\nProfile=Test\nProfileDir=Test\nSceneCollection=Test\nSceneCollectionFile=Test\n"
    )
    (profile / "basic.ini").write_text(
        "[General]\nName=Test\n\n[Output]\nMode=Simple\n\n"
        "[Video]\nBaseCX=320\nBaseCY=240\nOutputCX=320\nOutputCY=240\nFPSType=0\nFPSCommon=30\n\n"
        "[Audio]\nSampleRate=48000\nChannelSetup=Stereo\n"
    )
    vcam = {"type2": 3, "audio": True, "audio_track": 1}
    if device:
        vcam["audio_device"] = device
    collection = {
        "name": "Test",
        "current_scene": "Scene",
        "current_program_scene": "Scene",
        "scene_order": [{"name": "Scene"}],
        "sources": [
            {
                "id": "scene", "versioned_id": "scene", "name": "Scene",
                "settings": {"id_counter": 1, "items": [
                    {"name": "Tone", "id": 1, "visible": True, "pos": {"x": 0, "y": 0},
                     "scale": {"x": 1, "y": 1}, "align": 5},
                ]},
            },
            {
                "id": "ffmpeg_source", "versioned_id": "ffmpeg_source", "name": "Tone",
                "mixers": 255,
                "settings": {"is_local_file": True, "local_file": str(tone), "looping": True,
                             "restart_on_activate": False, "close_when_inactive": False},
            },
        ],
        "virtual-camera": vcam,
    }
    (scenes / "Test.json").write_text(json.dumps(collection, indent=2))


def find_input(name: str):
    import sounddevice as sd

    for i, dev in enumerate(sd.query_devices()):
        if dev["max_input_channels"] > 0 and name.lower() in dev["name"].lower():
            return i, dev
    return None, None


def record(name: str, seconds: float, out: Path) -> int:
    import numpy as np
    import sounddevice as sd

    index, dev = find_input(name)
    if index is None:
        print(f"FAIL: no input device matching {name!r}", file=sys.stderr)
        print(sd.query_devices(), file=sys.stderr)
        return 1
    channels = min(2, dev["max_input_channels"])
    print(f"recording {seconds}s from #{index} {dev['name']!r} ({channels} ch)")
    audio = sd.rec(int(seconds * RATE), samplerate=RATE, channels=channels, dtype="float32", device=index)
    sd.wait()
    with wave.open(str(out), "wb") as w:
        w.setnchannels(channels)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes((np.clip(audio, -1, 1) * 32767).astype("<i2").tobytes())
    # Skip the first half second: the device may still be filling.
    mono = audio[RATE // 2:].mean(axis=1)
    rms = float(np.sqrt(np.mean(mono ** 2)))
    spectrum = np.abs(np.fft.rfft(mono * np.hanning(len(mono))))
    peak = float(np.fft.rfftfreq(len(mono), 1 / RATE)[int(spectrum.argmax())])
    print(f"rms={rms:.4f} peak={peak:.1f} Hz")
    if rms < 0.01:
        print("FAIL: the virtual microphone is silent", file=sys.stderr)
        return 1
    if abs(peak - TONE_HZ) > 5:
        print(f"FAIL: expected the {TONE_HZ:.0f} Hz tone, got {peak:.1f} Hz", file=sys.stderr)
        return 1
    print("PASS: the tone came out of the virtual microphone")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("profile")
    p.add_argument("--config-root", type=Path, required=True)
    p.add_argument("--tone", type=Path, required=True)
    p.add_argument("--device", default="")
    r = sub.add_parser("record")
    r.add_argument("--device", required=True)
    r.add_argument("--seconds", type=float, default=3.0)
    r.add_argument("--out", type=Path, required=True)
    sub.add_parser("devices")
    args = parser.parse_args()

    if args.cmd == "profile":
        args.tone.parent.mkdir(parents=True, exist_ok=True)
        write_tone(args.tone)
        write_profile(args.config_root, args.tone.resolve(), args.device)
        return 0
    if args.cmd == "record":
        return record(args.device, args.seconds, args.out)
    import sounddevice as sd

    print(sd.query_devices())
    return 0


if __name__ == "__main__":
    sys.exit(main())
