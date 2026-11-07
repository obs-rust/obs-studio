#!/usr/bin/env -S uv run --script
"""Batch 2 native contracts must carry portable header dependencies."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Core registrations from #1390 batch 2 (#3847); the graph has no batch markers.
BATCH2_CORE_TESTS = frozenset("""
test_async_delay_pacing
test_async_upload_steps_rust
test_audio_monitor_delay_steps_rust
test_avcapture_audio_planes
test_coreaudio_device_rules_rust
test_coreaudio_pacing_steps_rust
test_coreaudio_retry
test_coreaudio_zero_channels
test_deinterlace_draw_plan_steps_rust
test_deinterlace_pacing_steps_rust
test_deinterlace_select_steps_rust
test_display_reinit
test_dynamics_threshold
test_effect_parser_lifecycle
test_expander_unity
test_fade_to_color_mix
test_ffmpeg_mux_write_packet
test_graphics_host_exports
test_graphics_host_functions
test_happy_eyeballs
test_happy_eyeballs_gai_error
test_image_exif_orientation
test_image_file
test_image_file_version
test_image_file_version_filetime
test_image_source_host
test_ingest_refresh_wait
test_media_remux
test_monitor_speaker_layout_rust
test_monitor_video_sync_rust
test_monitor_wasapi_steps
test_monitor_wasapi_steps_rust
test_nvenc_caps_cache
test_obs_outputs_net_if_host
test_oss_sndstat
test_pulse_monitor_format
test_qsv_host
test_remux_audio_timestamps
test_scene_audio_window_steps_rust
test_scene_render_plan_steps_rust
test_sck_stall
test_sck_stop_wait
test_source_audio_input_steps_rust
test_source_audio_place_steps_rust
test_source_audio_render_steps_rust
test_source_audio_timing_steps_rust
test_source_color_space_steps_rust
test_source_draw_plan_steps_rust
test_source_frame_copy_steps_rust
test_source_media_admission_steps_rust
test_source_monitoring_steps_rust
test_source_ptm_gate_steps_rust
test_source_render_route_steps_rust
test_source_size_route_steps_rust
test_source_video_tick_steps_rust
test_source_volume_apply_steps_rust
test_transition_load_steps_rust
test_transition_render_steps_rust
test_transition_start_steps_rust
test_vaapi_probe_policy
test_video_scaler_direct
""".split())


class NativeB2Portability(unittest.TestCase):
    def test_filter_pch_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        dependency = {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
        for name in ("test_async_delay_pacing", "test_dynamics_threshold", "test_expander_unity"):
            with self.subTest(name=name):
                self.assertIn(dependency, graph["targets"][name]["libraries"])

    def test_evaluated_linux_core_inventory(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        self.assert_linux_core_inventory(graph)

    def test_core_inventory_allows_additional_test(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        graph["targets"]["test_synthetic_core"] = {}
        graph["tests"]["test_synthetic_core"] = {"target": "test_synthetic_core"}
        self.assert_linux_core_inventory(graph)

    def assert_linux_core_inventory(self, graph):
        facts = dict.fromkeys(("OS_WINDOWS", "OS_MACOS", "OS_FREEBSD", "OS_OPENBSD", "MSVC",
                               "HAVE_PIPEWIRE_FRAME_LAYOUT_SDK", "HAVE_PULSE_DEFAULT_RETRY_SDK",
                               "C_COMPILER_CLANG", "HAVE_OBS_FFMPEG", "HAVE_OBS_QSV11", "HAVE_OBS_OPENGL"), False)
        facts.update(dict.fromkeys(("OS_LINUX", "UNIX", "HAVE_LIBOBS", "HAVE_LIBOBS_RUST_STATIC",
                                    "HAVE_HAPPY_EYEBALLS", "HAVE_OBS_OUTPUTS", "HAVE_OBS_FILTERS",
                                    "HAVE_OBS_PIPEWIRE", "HAVE_OBS_PULSEAUDIO"), True))

        def enabled(requirement):
            if isinstance(requirement, bool):
                return requirement
            if isinstance(requirement, str):
                return facts[requirement]
            if "not" in requirement:
                return not enabled(requirement["not"])
            operator = "all" if "all" in requirement else "any"
            values = [enabled(value) for value in requirement[operator]]
            return (all if operator == "all" else any)(values)

        def selected_tests():
            return {name for name, test in graph["tests"].items()
                    if test.get("suite", "core") == "core"
                    and enabled(test.get("requires", True))
                    and enabled(graph["targets"][test["target"]].get("requires", True))}

        inventories = {}
        sdk_tests = {"test_pipewire_frame_layout", "test_pulse_default_retry"}
        for pulse in (False, True):
            for hevc in (False, True):
                with self.subTest(pulse=pulse, hevc=hevc):
                    facts.update(ENABLE_PULSEAUDIO=pulse, ENABLE_HEVC=hevc,
                                 HAVE_PIPEWIRE_FRAME_LAYOUT_SDK=False,
                                 HAVE_PULSE_DEFAULT_RETRY_SDK=False)
                    selected = selected_tests()
                    inventories[pulse, hevc] = selected
                    self.assertTrue(sdk_tests.isdisjoint(selected))
                    # Both SDK additions are independent of core PulseAudio/HEVC.
                    facts.update(HAVE_PIPEWIRE_FRAME_LAYOUT_SDK=True,
                                 HAVE_PULSE_DEFAULT_RETRY_SDK=True)
                    sdk_selected = selected_tests()
                    self.assertTrue(sdk_tests <= sdk_selected)
                    self.assertEqual(sdk_selected, selected | sdk_tests)
                    self.assertEqual(len(sdk_selected), len(selected) + 2)

        for hevc in (False, True):
            self.assertEqual(len(inventories[True, hevc]) - len(inventories[False, hevc]), 5)
        for pulse in (False, True):
            self.assertEqual(len(inventories[pulse, True]) - len(inventories[pulse, False]), 1)
        self.assertGreaterEqual(len(inventories[False, True]), 417)
        self.assertTrue(BATCH2_CORE_TESTS <= inventories[True, True],
                        BATCH2_CORE_TESTS - inventories[True, True])
        consumer = (ROOT / "rust/obs-build/src/validation.rs").read_text()
        self.assertIn('("HAVE_OBS_OPENGL".into(), false)', consumer)

    def test_standalone_fixture_uses_header_only_simde_finder(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        for name, target in graph["targets"].items():
            self.assertNotIn("simde", target.get("header_packages", []), name)
            self.assertNotIn("simde", target.get("packages", []), name)
        source = (ROOT / "rust/obs-build/tests/native.rs").read_text()
        self.assertFalse('pkg_check_modules(SIMDE' in source)
        self.assertFalse('Dependency::package(&build.target, "simde")' in source)


if __name__ == "__main__":
    unittest.main()
