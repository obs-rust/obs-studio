#!/usr/bin/env -S uv run --script
"""verify-recording gives the recording the AppImage test made a pass/fail (#962).

The rule lives in build-aux/appimage/verify_recording.py since the port off the
116-line shell script, and appimage.yaml and ci/flatpak_runtime.py call it. This
drives the port's `verify()` over a stub `ffmpeg`/`ffprobe` (a callable the tool
outputs come from), pinning the verdict the shell reached for a good recording
and for each failure the shell tested: a frozen picture, a silent microphone,
audio that is not the 440 Hz tone, silence, the wrong canvas, too few frames, no
audio stream, and a file that does not decode. Every check fails closed.

    uv run python -m unittest ci/test_verify_recording.py
"""

from __future__ import annotations

import importlib.util
import io
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
APPIMAGE = ROOT / "build-aux" / "appimage"
MODULE = APPIMAGE / "verify_recording.py"
SHELL_SCRIPT = APPIMAGE / "verify-recording.sh"

# The tool output a passing recording produces. `verify()` runs its tools
# through a callable that returns `(returncode, combined stdout+stderr)`, so a
# case is one entry changing one entry here.
GOOD = {
    "decode": (0, ""),
    "duration": (0, "12.033000\n"),
    "size": (0, "320x240\n"),
    "frames": (0, "361\n"),
    "audio": (0, "aac,48000,2\n"),
    "freeze": (0, ""),
    "rms": (0, "  RMS level dB: -11.738818\n"),
    "tone": (0, "  RMS level dB: -11.740692\n"),
    "silence": (0, ""),
}

_PROBE_KEY = {
    "format=duration": "duration",
    "stream=width,height": "size",
    "stream=nb_read_frames": "frames",
    "stream=codec_name,sample_rate,channels": "audio",
}


def load_module():
    """build-aux/appimage/verify_recording.py by path: `uv run python -m unittest
    ci/test_verify_recording.py` does not put build-aux/ on sys.path, and no file
    in this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("verify_recording", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def tool(**overrides):
    """A stand-in for the module's `run_tool`, dispatching on the argv the shell
    passed: which ffprobe field set it asked for, or which ffmpeg filter."""
    outputs = {**GOOD, **overrides}

    def run(argv: list[str]) -> tuple[int, str]:
        if argv[0] == "ffprobe":
            field = argv[argv.index("-show_entries") + 1]
            return outputs[_PROBE_KEY[field]]
        video = argv[argv.index("-vf") + 1] if "-vf" in argv else ""
        audio = argv[argv.index("-af") + 1] if "-af" in argv else ""
        if "freezedetect" in video:
            return outputs["freeze"]
        if "silencedetect" in audio:
            return outputs["silence"]
        if "bandpass" in audio:
            return outputs["tone"]
        if "astats" in audio:
            return outputs["rms"]
        return outputs["decode"]

    return run


class VerdictTest(unittest.TestCase):
    """The pass/fail the shell reached for a good recording and each failure."""

    def setUp(self):
        self.module = load_module()

    def check(self, expected, **overrides):
        """`verify()` verdict plus its printed output (the `::error::` lines)."""
        captured = io.StringIO()
        with redirect_stdout(captured):
            verdict = self.module.verify("recording.mkv", tool=tool(**overrides))
        self.assertEqual(verdict, expected, captured.getvalue())
        return captured.getvalue()

    def test_a_good_recording_passes(self):
        self.assertNotIn("::error::", self.check(True))

    def test_a_frozen_screen_fails(self):
        out = self.check(False, freeze=(0, "  freeze_start: 1.2\n"))
        self.assertIn("the screen picture froze (1 times)", out)

    def test_a_silent_microphone_fails(self):
        out = self.check(False, rms=(0, "  RMS level dB: -60\n"))
        self.assertIn("the microphone is silent or nearly so (RMS -60 dB)", out)

    def test_audio_that_is_not_the_tone_fails(self):
        out = self.check(False, tone=(0, "  RMS level dB: -40\n"))
        self.assertIn("the audio is not the 440 Hz test tone", out)

    def test_silence_in_the_microphone_fails(self):
        out = self.check(False, silence=(0, "  silence_start: 1.0\n"))
        self.assertIn("the microphone dropped out (1 silences)", out)

    def test_the_wrong_canvas_fails(self):
        out = self.check(False, size=(0, "640x480\n"))
        self.assertIn("the video is 640x480, not the 320x240 canvas", out)

    def test_too_few_frames_fails(self):
        out = self.check(False, frames=(0, "100\n"))
        self.assertIn("the video has 100 frames, fewer than 5 s at 30 fps", out)

    def test_missing_frames_fails(self):
        out = self.check(False, frames=(0, "\n"))
        self.assertIn("the video has no frames", out)

    def test_no_audio_stream_fails(self):
        out = self.check(False, audio=(0, "\n"))
        self.assertIn("the recording has no audio stream", out)

    def test_a_file_that_does_not_decode_fails(self):
        out = self.check(False, decode=(1, "Invalid data found\n"))
        self.assertIn("the recording does not decode cleanly", out)

    def test_a_bad_canvas_stops_before_the_pictures(self):
        # The shell returned right after the format checks, so no region runs.
        out = self.check(False, size=(0, "\n"))
        self.assertNotIn("region:", out)


class WiringTest(unittest.TestCase):
    """The shell script is gone; the callers run the Python module."""

    def test_the_shell_script_is_ported(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "the shell script is the ported one")

    def test_no_gen_005_exception_is_left(self):
        ci_toml = (ROOT / "ci.toml").read_text(encoding="utf-8")
        self.assertNotIn(
            "build-aux/appimage/verify-recording.sh",
            ci_toml,
            "the GEN-005 exception for verify-recording.sh is still in ci.toml",
        )

    def test_flatpak_runtime_runs_the_python(self):
        source = (ROOT / "ci" / "flatpak_runtime.py").read_text(encoding="utf-8")
        # assertTrue with a short message: assertIn would echo the whole file.
        self.assertTrue(
            "build-aux/appimage/verify_recording.py" in source and "verify-recording.sh" not in source,
            "flatpak_runtime.py does not run verify_recording.py through the interpreter",
        )

    def test_the_appimage_workflow_runs_the_python(self):
        workflow = (ROOT / ".github" / "workflows" / "appimage.yaml").read_text(encoding="utf-8")
        # assertTrue with a short message: assertIn would echo the whole workflow.
        self.assertTrue(
            "uv run --no-project build-aux/appimage/verify_recording.py" in workflow
            and "verify-recording.sh" not in workflow,
            "appimage.yaml does not run the verification through uv",
        )


if __name__ == "__main__":
    unittest.main()
