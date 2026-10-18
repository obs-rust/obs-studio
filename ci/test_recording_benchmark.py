#!/usr/bin/env -S uv run --script
"""Tests for the full-app recording evidence runner (#1305).

    uv run python -m unittest ci/test_recording_benchmark.py
"""

from __future__ import annotations

import copy
import importlib.util
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Loaded by path: ci/ tests import only the standard library and ci/
# modules (test_ci_tests_stdlib_only.py). recording.py needs websockets
# only for a live run, not at import.
spec = importlib.util.spec_from_file_location("recording", ROOT / "rust/tools/benchmarks/recording.py")
rb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(rb)


def stats(t: float, rendered: int, render_skip: int = 0, output: int | None = None,
          output_skip: int = 0, ms: float = 3.0, cpu: float = 5.0) -> dict:
    return {
        "t": t,
        "cpuUsage": cpu,
        "activeFps": 25.0,
        "averageFrameRenderTime": ms,
        "renderTotalFrames": rendered,
        "renderSkippedFrames": render_skip,
        "outputTotalFrames": rendered if output is None else output,
        "outputSkippedFrames": output_skip,
    }


def recorder(total: int, skipped: int = 0) -> dict:
    return {"outputActive": True, "outputTotalFrames": total, "outputSkippedFrames": skipped}


def samples(n: int = 61, ms: float = 3.0, cpu: float = 5.0, render_skip_every: int = 0) -> list[dict]:
    out = []
    skipped = 0
    for i in range(n):
        if render_skip_every and i and i % render_skip_every == 0:
            skipped += 1
        out.append(stats(1000.0 + i, 25_000 + 25 * i, render_skip=skipped, ms=ms, cpu=cpu))
    return out


IDENTITY = {
    "renderer": {"api": "OpenGL", "adapter": "NVIDIA GeForce RTX 3060", "version": "3.3.0 NVIDIA 595.104.02"},
    "graphics": {"path": "/opt/obs/lib/libobs-opengl.so.30", "sha256": "g" * 64},
}


def run(backend: str, ms: float = 3.0, cpu: float = 5.0, render_skip_every: int = 0,
        video: dict | None = None, encoder: str = "nvenc") -> dict:
    s = samples(ms=ms, cpu=cpu, render_skip_every=render_skip_every)
    identity = copy.deepcopy(IDENTITY)
    identity["libobs"] = {"path": f"/opt/obs/{backend}/libobs.so.30", "sha256": backend * 32, "backend": backend}
    return {
        "identity": identity,
        "conditions": {
            "video": video or {"baseWidth": 3840, "baseHeight": 2160, "outputWidth": 3840,
                               "outputHeight": 2160, "fpsNumerator": 25, "fpsDenominator": 1},
            "profile": {"Output/Mode": "Simple", "SimpleOutput/RecEncoder": encoder},
            "scene": "Scene",
            "source": "9f573a9b3",
        },
        "metrics": rb.run_metrics(s, recorder(1500), recorder(3000)),
    }


class RunMetricsTest(unittest.TestCase):
    def test_counts_rates_and_medians(self):
        s = samples(render_skip_every=20)
        m = rb.run_metrics(s, recorder(1500), recorder(3000, skipped=3))
        self.assertEqual(m["duration_s"], 60.0)
        self.assertEqual(m["rendered_frames"], 1500)
        self.assertEqual(m["render_skipped"], 3)  # at samples 20, 40 and 60
        self.assertAlmostEqual(m["render_skip_rate"], 3 / 1500)
        self.assertEqual(m["encoder_frames"], 1500)
        self.assertEqual(m["encoder_skipped"], 3)
        self.assertAlmostEqual(m["encoder_skip_rate"], 3 / 1500)
        self.assertEqual(m["frame_render_ms_median"], 3.0)
        self.assertEqual(m["cpu_percent_median"], 5.0)
        self.assertEqual(m["samples"], 61)

    def test_counter_going_back_means_obs_restarted(self):
        s = samples()
        s[30]["renderTotalFrames"] = 10
        with self.assertRaisesRegex(rb.RunError, "went back"):
            rb.run_metrics(s, recorder(1500), recorder(3000))

    def test_too_few_samples(self):
        with self.assertRaisesRegex(rb.RunError, "samples"):
            rb.run_metrics(samples(n=5), recorder(1500), recorder(1600))

    def test_no_rendered_frames(self):
        s = samples()
        for x in s:
            x["renderTotalFrames"] = 25_000
        with self.assertRaisesRegex(rb.RunError, "no frames"):
            rb.run_metrics(s, recorder(1500), recorder(3000))

    def test_recorder_must_stay_active(self):
        inactive = recorder(3000)
        inactive["outputActive"] = False
        with self.assertRaisesRegex(rb.RunError, "not active"):
            rb.run_metrics(samples(), recorder(1500), inactive)


class ReportTest(unittest.TestCase):
    def runs(self, c_ms=3.0, rust_ms=3.0, rust_skip_every=0, n=5):
        return [run("c", ms=c_ms) for _ in range(n)] + [
            run("rust", ms=rust_ms, render_skip_every=rust_skip_every) for _ in range(n)
        ]

    def test_medians_ranges_and_pass(self):
        rep = rb.report(self.runs(c_ms=3.0, rust_ms=3.1))
        self.assertEqual(rep["runs"], {"c": 5, "rust": 5})
        ft = rep["metrics"]["frame_render_ms_median"]
        self.assertEqual(ft["c"]["median"], 3.0)
        self.assertEqual(ft["rust"]["min"], 3.1)
        self.assertAlmostEqual(ft["ratio"], 3.1 / 3.0)
        self.assertTrue(rep["gate"]["frame_time"]["pass"])
        self.assertTrue(rep["gate"]["dropped_frames"]["pass"])
        self.assertTrue(rep["pass"])

    def test_frame_time_over_five_percent_fails(self):
        rep = rb.report(self.runs(c_ms=3.0, rust_ms=3.2))
        self.assertFalse(rep["gate"]["frame_time"]["pass"])
        self.assertFalse(rep["pass"])

    def test_new_drops_against_zero_fail(self):
        rep = rb.report(self.runs(rust_skip_every=10))
        self.assertFalse(rep["gate"]["dropped_frames"]["pass"])

    def test_too_few_runs_per_backend(self):
        with self.assertRaisesRegex(rb.RunError, "at least 5"):
            rb.report(self.runs(n=4))

    def test_mixed_conditions_are_refused(self):
        runs = self.runs()
        runs[3]["conditions"]["video"]["outputWidth"] = 1920
        with self.assertRaisesRegex(rb.RunError, "conditions differ"):
            rb.report(runs)

    def test_mixed_encoder_is_refused(self):
        runs = self.runs()
        runs[7]["conditions"]["profile"]["SimpleOutput/RecEncoder"] = "x264"
        with self.assertRaisesRegex(rb.RunError, "conditions differ"):
            rb.report(runs)

    def test_mixed_renderer_is_refused(self):
        runs = self.runs()
        runs[2]["identity"]["renderer"]["version"] = "3.3.0 NVIDIA 600.1"
        with self.assertRaisesRegex(rb.RunError, "conditions differ"):
            rb.report(runs)

    def test_two_builds_of_one_backend_are_refused(self):
        runs = self.runs()
        runs[1]["identity"]["libobs"]["sha256"] = "x" * 64
        with self.assertRaisesRegex(rb.RunError, "more than one c libobs"):
            rb.report(runs)

    def test_markdown_names_both_backends(self):
        text = rb.format_report(rb.report(self.runs(rust_ms=3.1)))
        self.assertIn("| frame_render_ms_median | 3.000 (3.000-3.000) | 3.100 (3.100-3.100) | 1.033 |", text)
        self.assertIn("PASS", text)


class EncoderEvidenceTest(unittest.TestCase):
    # OBS logs the encoder settings before the "Recording Start" marker and
    # the frame summary after it (CT104 log, 2026-10-18).
    OLD = """\
08:00:00.000: [x264 encoder: 'simple_video_stream'] preset: veryfast
08:00:00.001: ==== Recording Start ===============================================
08:00:30.000: ==== Recording Stop ================================================
"""
    LOG = """\
09:00:00.000: [obs-nvenc: 'simple_video_stream'] settings:
09:00:00.001: 	codec:        H264
09:00:00.001: 	rate_control: CBR
09:00:00.002: ---------------------------------
09:00:00.003: [FFmpeg aac encoder: 'simple_aac'] bitrate: 160, channels: 2, channel_layout: stereo, track: 1
09:00:00.004: ==== Recording Start ===============================================
09:00:00.005: [ffmpeg muxer: 'simple_file_output'] Writing file '/root/2026-10-18 09-00-00.mkv'...
09:00:36.000: Output 'simple_file_output': Total frames output: 882
09:00:36.000: Output 'simple_file_output': Total drawn frames: 899
09:00:36.001: ==== Recording Stop ================================================
09:00:37.000: later line
"""

    def test_lines_around_the_last_recording_start(self):
        lines = rb.encoder_lines(self.OLD + self.LOG, expect="obs-nvenc")
        self.assertIn("[obs-nvenc: 'simple_video_stream'] settings:", lines[0])
        self.assertTrue(any("codec:        H264" in x for x in lines))
        self.assertTrue(any("Total drawn frames: 899" in x for x in lines))
        self.assertFalse(any("x264" in x or "later line" in x for x in lines))

    def test_expected_encoder_missing(self):
        # The x264 line belongs to the earlier recording, not to this one.
        with self.assertRaisesRegex(rb.RunError, "x264"):
            rb.encoder_lines(self.OLD + self.LOG, expect="x264")

    def test_source_version_from_the_log_header(self):
        header = "".join(f"07:34:33.{i:03d}: line {i}\n" for i in range(13))
        header += "07:34:33.235: OBS-Studio-Rust 0.1.0 (based on OBS 33.0.0-beta6-1609-g9f573a9b3 (linux))\n"
        self.assertEqual(rb.source_version(header), "33.0.0-beta6-1609-g9f573a9b3")
        self.assertIsNone(rb.source_version("no header\n"))

    def test_no_recording_start(self):
        with self.assertRaisesRegex(rb.RunError, "Recording Start"):
            rb.encoder_lines("09:00:00.000: nothing\n", expect="obs-nvenc")


if __name__ == "__main__":
    unittest.main()
