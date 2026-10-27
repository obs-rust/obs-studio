#!/usr/bin/env -S uv run --script
"""Check the recording test-appimage.sh made with a saved profile:

    uv run --no-project build-aux/appimage/verify_recording.py <recording.mkv>
    uv run --no-project build-aux/appimage/verify_recording.py --self-test

Runs outside the test container (it needs ffmpeg, which the container must not
have). The scene is a PipeWire screen capture filling the 320x240 canvas with a
camera in the bottom-right 80x60 corner, both fed moving test patterns, plus a
microphone fed a 440 Hz tone. So: the file decodes cleanly at 320x240 with
enough frames, both picture regions keep changing, and the audio is the tone
throughout. Every check fails closed: a tool error is a failure, never a pass.

Ported off the 116-line shell script in #962; the AppImage workflow and
ci/flatpak_runtime.py call this file.
"""

from __future__ import annotations

import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path

# One tool run: `(returncode, combined stdout+stderr)`, like the shell's `2>&1`.
Tool = Callable[[list[str]], "tuple[int, str]"]

CANVAS = "320x240"
MIN_FRAMES = 150
MIC_RMS_FLOOR_DB = -40.0
TONE_MATCH_DB = 3.0
TONE_HZ = 440
# The moving-picture regions the shell's `for region` loop checked: crop=WxH+X+Y.
REGIONS = (("screen", "240:180:0:0"), ("camera", "80:60:240:180"))
# The band the tone measurement isolates around 440 Hz.
TONE_FILTER = f"bandpass=f={TONE_HZ}:width_type=h:w=100,"

_RMS_RE = re.compile(r"RMS level dB: *(-*[0-9.inf]*)")
_NUMBER_RE = re.compile(r"^-?[0-9.]+$")


def run_tool(argv: list[str]) -> tuple[int, str]:
    """Run a tool with stderr merged into stdout, like the shell's `2>&1`."""
    result = subprocess.run(argv, capture_output=True, text=True)
    return result.returncode, result.stdout + result.stderr


def count(pattern: str, text: str) -> int:
    """Lines of `text` containing `pattern`, as `grep -c` counted them."""
    return sum(1 for line in text.splitlines() if pattern in line)


def _fail(message: str) -> bool:
    """Print an `::error::` line (the shell's `error()`) and report failure."""
    print(f"::error::{message}")
    return True


def _probe(args: list[str], rec: str, tool: Tool) -> str:
    """One `ffprobe -v error` field, stripped (the shell's `$(ffprobe ...)`)."""
    return tool(["ffprobe", "-v", "error", *args, rec])[1].strip()


def _check_container(rec: str, tool: Tool) -> bool:
    """Decode and format checks; the shell returned right after these fail."""
    code, out = tool(["ffmpeg", "-nostdin", "-hide_banner", "-v", "error", "-i", rec, "-f", "null", "-"])
    if code != 0 or out:
        for line in out.splitlines()[:5]:
            print(line)
        return _fail("the recording does not decode cleanly")

    duration = _probe(["-show_entries", "format=duration", "-of", "csv=p=0"], rec, tool)
    size = _probe(["-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0:s=x"], rec, tool)
    frames = _probe(["-select_streams", "v:0", "-count_frames", "-show_entries", "stream=nb_read_frames", "-of", "csv=p=0"], rec, tool)
    audio = _probe(["-select_streams", "a:0", "-show_entries", "stream=codec_name,sample_rate,channels", "-of", "csv=p=0"], rec, tool)
    print(f"duration: {duration} s, video: {size}, {frames} frames, audio: {audio or 'none'}")

    failed = False
    if size != CANVAS:
        failed = _fail(f"the video is {size or 'missing'}, not the {CANVAS} canvas")
    if not (frames.isdigit() and int(frames) >= MIN_FRAMES):
        failed = _fail(f"the video has {frames or 'no'} frames, fewer than 5 s at 30 fps") or failed
    if not audio:
        failed = _fail("the recording has no audio stream") or failed
    return failed


def _check_motion(rec: str, tool: Tool) -> bool:
    """Each picture region keeps changing; a freeze fails it."""
    failed = False
    for name, region in REGIONS:
        code, out = tool(["ffmpeg", "-nostdin", "-hide_banner", "-ss", "1", "-i", rec, "-an",
                          "-vf", f"crop={region},freezedetect=n=0.001:d=1", "-f", "null", "-"])
        if code != 0:
            _fail(f"ffmpeg failed on the {name} region")
            failed = True
            continue
        freezes = count("freeze_start", out)
        times = " ".join(re.findall(r"freeze_start: [0-9.]*", out))
        print(f"{name} region: {freezes} freezes {times}")
        if freezes != 0:
            _fail(f"the {name} picture froze ({freezes} times)")
            failed = True
    return failed


def _measure_rms(prefix: str, rec: str, tool: Tool) -> str | None:
    """The last `RMS level dB` astats measured, or None when ffmpeg failed."""
    code, out = tool(["ffmpeg", "-nostdin", "-hide_banner", "-ss", "1", "-i", rec, "-vn",
                      "-af", f"{prefix}astats=measure_perchannel=none", "-f", "null", "-"])
    if code != 0:
        return None
    matches = _RMS_RE.findall(out)
    return matches[-1] if matches else ""


def _check_audio(rec: str, tool: Tool) -> bool:
    """The microphone is loud, at 440 Hz throughout, and never drops out."""
    failed = False
    overall = _measure_rms("", rec, tool)
    if overall is None:
        _fail("ffmpeg failed to measure the audio")
        overall = ""
        failed = True
    tone = _measure_rms(TONE_FILTER, rec, tool)
    if tone is None:
        _fail("ffmpeg failed to measure the audio at 440 Hz")
        tone = ""
        failed = True
    print(f"audio RMS: {overall or '?'} dB, at 440 Hz: {tone or '?'} dB")
    # The shell's two awk checks: the level is a number above the floor, and the
    # tone is within TONE_MATCH_DB of the whole signal (a non-numeric level
    # counts as 0, as awk's own coercion did).
    level = float(overall) if _NUMBER_RE.match(overall) else 0.0
    if not (_NUMBER_RE.match(overall) and level > MIC_RMS_FLOOR_DB):
        failed = _fail(f"the microphone is silent or nearly so (RMS {overall or '?'} dB)") or failed
    if not (_NUMBER_RE.match(tone) and level - float(tone) < TONE_MATCH_DB):
        failed = _fail(
            f"the audio is not the {TONE_HZ} Hz test tone "
            f"(RMS {overall or '?'} dB, {tone or '?'} dB at {TONE_HZ} Hz)"
        ) or failed

    code, out = tool(["ffmpeg", "-nostdin", "-hide_banner", "-ss", "1", "-i", rec, "-vn",
                      "-af", "silencedetect=noise=-50dB:d=0.3", "-f", "null", "-"])
    if code != 0:
        _fail("ffmpeg failed to look for silence")
        failed = True
    silences = count("silence_start", out)
    print(f"audio: {silences} silences")
    if silences != 0:
        failed = _fail(f"the microphone dropped out ({silences} silences)") or failed
    return failed


def verify(rec: str, tool: Tool = run_tool) -> bool:
    """True when the recording passes every check; each failure prints ::error::."""
    if _check_container(rec, tool):
        return False
    failed = _check_motion(rec, tool)
    failed = _check_audio(rec, tool) or failed
    return not failed


def _make(path: Path, video: str, audio: str, tool: Tool = run_tool) -> None:
    """Encode a synthetic recording, the shell self-test's `make`."""
    code, out = tool(["ffmpeg", "-nostdin", "-v", "error", "-f", "lavfi", "-i", video,
                      "-f", "lavfi", "-i", "testsrc2=s=80x60:r=30", "-f", "lavfi", "-i", audio,
                      "-filter_complex", "[0][1]overlay=240:180[v]", "-map", "[v]", "-map", "2",
                      "-t", "8", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", str(path)])
    if code != 0:
        raise RuntimeError(f"ffmpeg could not build {path.name}: {out.strip()}")


def self_test(tool: Tool = run_tool) -> int:
    """A good recording passes; a frozen, silent or garbage file fails."""
    tone = f"sine=f={TONE_HZ}:r=48000"
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        _make(directory / "good.mkv", "testsrc2=s=320x240:r=30", tone, tool)
        _make(directory / "frozen.mkv", "testsrc2=s=320x240:r=30,trim=end_frame=1,tpad=stop=-1:stop_mode=clone", tone, tool)
        _make(directory / "silent.mkv", "testsrc2=s=320x240:r=30", "anullsrc=r=48000:cl=stereo", tool)
        (directory / "garbage.mkv").write_bytes(os.urandom(100000))
        ok = True
        for name, expect_pass in (("good", True), ("frozen", False), ("silent", False), ("garbage", False)):
            print(f"== self-test: {name} (expect {'pass' if expect_pass else 'fail'})")
            if verify(str(directory / f"{name}.mkv"), tool) != expect_pass:
                _fail(f"verify_recording.py self-test: the {name} case came out wrong")
                ok = False
    if not ok:
        return 1
    print("verify-recording: self-test passed (good passes; frozen, silent and garbage fail)")
    return 0


def main(argv: list[str]) -> int:
    if not shutil.which("ffmpeg") or not shutil.which("ffprobe"):
        _fail("ffmpeg and ffprobe are required")
        return 1
    if argv and argv[0] == "--self-test":
        return self_test()
    if not argv:
        print("usage: verify_recording.py <recording.mkv> | --self-test", file=sys.stderr)
        return 2
    if not verify(argv[0]):
        print("verify-recording: FAILED")
        return 1
    print("verify-recording: screen and camera move, the microphone carries the 440 Hz tone throughout")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
