#!/usr/bin/env -S uv run --script
"""Batch 2 native contracts must carry portable header dependencies."""
import json
import re
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
    def test_scene_archive_consumers_supply_getter_logging(self):
        # MSVC keeps the getters' blog reference in the scene archive member
        # pulled by these pure rule tests (#3891, Windows LNK2019).
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        stub = "test/cmocka/scene-getter-log-link-stub.c"
        for name in ("test_scene_audio_window_steps_rust", "test_scene_render_plan_steps_rust"):
            with self.subTest(name=name):
                self.assertEqual(graph["targets"][name]["sources"], [
                    stub, "test/cmocka/" + name.removesuffix("_rust") + ".c",
                ])

    def test_all_scene_static_consumers_have_blog_provider(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        consumers = set()
        stub = "test/cmocka/scene-getter-log-link-stub.c"
        for name, target in graph["targets"].items():
            libraries = target.get("libraries", [])
            if "libobs_rust-static" not in libraries:
                continue
            sources = target.get("sources", [])
            scene_sources = [source for source in sources if isinstance(source, str)
                             and ("obs-scene" in (ROOT / source).read_text()
                                  or "obs_scene" in (ROOT / source).read_text())]
            if scene_sources:
                consumers.add(name)
                with self.subTest(target=name):
                    self.assertTrue("OBS::libobs" in libraries or stub in sources,
                                    "standalone scene archive consumer needs blog")
        self.assertTrue({"test_scene_audio_window_steps_rust",
                         "test_scene_render_plan_steps_rust"} <= consumers)

        # Registrations not yet migrated into the graph still link libobs
        # (some also carry the abort-only stub). Check their actual commands.
        legacy = set()
        for fragment in (ROOT / "test/cmocka/tests.d").glob("test_scene*.cmake"):
            text = fragment.read_text()
            for match in re.finditer(r"target_link_libraries\(\s*(test_scene\w+_rust)\s+([^)]*)\)", text):
                name, libraries = match.groups()
                if "libobs_rust-static" in libraries:
                    legacy.add(name)
                    with self.subTest(target=name):
                        self.assertIn("OBS::libobs", libraries)
        self.assertIn("test_scene_item_names_steps_rust", legacy | consumers)
    def test_source_boolean_queries_supply_standalone_field_adapter(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        consumers = [name for name, target in graph["targets"].items()
                     if "test/cmocka/source-async-flag-getter-link-stubs.c" in target["sources"]]
        self.assertEqual(len(consumers), 23)
        for name in consumers:
            with self.subTest(target=name):
                target = graph["targets"][name]
                self.assertIn("test/cmocka/source-boolean-query-link-stubs.c", target["sources"])
                self.assertIn("test/cmocka/source-getter-log-link-stub.c", target["sources"])
                self.assertIn("OBS::libobs", target["header_libraries"])
                self.assertIn("SIMDe::SIMDe", target["header_libraries"])
                self.assertIn({"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"},
                              target["libraries"])
        legacy = (ROOT / "test/cmocka/tests.d/source_boolean_query_link_support.cmake").read_text()
        self.assertIn("source-boolean-query-link-stubs.c", legacy)
        self.assertIn("source-getter-log-link-stub.c", legacy)
        self.assertIn("$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>", legacy)

    def test_source_state_getters_supply_standalone_field_adapter(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        consumers = [name for name, target in graph["targets"].items()
                     if "test/cmocka/source-async-flag-getter-link-stubs.c" in target["sources"]]
        self.assertEqual(len(consumers), 23)
        for name in consumers:
            with self.subTest(target=name):
                target = graph["targets"][name]
                self.assertIn("test/cmocka/source-state-getter-link-stubs.c", target["sources"])
                self.assertIn("test/cmocka/source-getter-log-link-stub.c", target["sources"])
                self.assertIn("OBS::libobs", target["header_libraries"])
                self.assertIn("SIMDe::SIMDe", target["header_libraries"])
                self.assertIn({"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"},
                              target["libraries"])
        legacy = (ROOT / "test/cmocka/tests.d/source_state_getter_link_support.cmake").read_text()
        self.assertIn("source-state-getter-link-stubs.c", legacy)
        self.assertIn("source-getter-log-link-stub.c", legacy)
        self.assertIn("$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>", legacy)

    def test_deinterlace_link_support_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        dependency = {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
        supported = [name for name, target in graph["targets"].items()
                     if "test/cmocka/source-deinterlace-getter-link-stubs.c" in target["sources"]]
        self.assertEqual(len(supported), 23)
        for name in supported:
            with self.subTest(name=name):
                self.assertIn(dependency, graph["targets"][name]["libraries"])
        legacy = (ROOT / "test/cmocka/tests.d/source_deinterlace_getter_link_support.cmake").read_text()
        self.assertIn("$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>", legacy)

    def test_async_flag_link_support_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        dependency = {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
        supported = [name for name, target in graph["targets"].items()
                     if "test/cmocka/source-async-flag-getter-link-stubs.c" in target["sources"]]
        self.assertEqual(len(supported), 23)
        for name in supported:
            with self.subTest(name=name):
                self.assertIn(dependency, graph["targets"][name]["libraries"])
        legacy = (ROOT / "test/cmocka/tests.d/source_async_flag_getter_link_support.cmake").read_text()
        self.assertIn("$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>", legacy)

    def test_filter_relationship_link_support_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        dependency = {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
        supported = [name for name, target in graph["targets"].items()
                     if "test/cmocka/source-filter-relationship-getter-link-stubs.c" in target["sources"]]
        self.assertEqual(len(supported), 23)
        for name in supported:
            with self.subTest(name=name):
                self.assertIn(dependency, graph["targets"][name]["libraries"])
        legacy = (ROOT / "test/cmocka/tests.d/source_filter_relationship_getter_link_support.cmake").read_text()
        self.assertIn("$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>", legacy)

    def test_scene_static_consumers_supply_private_mutex_adapters(self):
        # Hidden libobs adapters cannot resolve from its shared library. These
        # pure-rule consumers must never call them; the abort-only stub proves it.
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        stub = "test/cmocka/scene-getter-log-link-stub.c"
        text = (ROOT / stub).read_text()
        for symbol in ("obs_sceneitem_fields_lock", "obs_sceneitem_fields_unlock"):
            self.assertIn(symbol, text)
        for name, target in graph["targets"].items():
            if "libobs_rust-static" not in target.get("libraries", []):
                continue
            sources = target.get("sources", [])
            if any(isinstance(source, str) and "obs_scene" in (ROOT / source).read_text()
                   for source in sources):
                with self.subTest(target=name):
                    self.assertIn(stub, sources)
        legacy = set()
        for fragment in (ROOT / "test/cmocka/tests.d").glob("test_scene*.cmake"):
            text = fragment.read_text()
            for match in re.finditer(r"add_executable\(\s*(test_scene\w+_rust)\s+([^)]*)\)", text):
                name, sources = match.groups()
                legacy.add(name)
                with self.subTest(target=name):
                    if name == "test_scene_item_crop_rust":
                        self.assertIn("scene-item-fields-link-support.c", sources)
                        self.assertNotIn("scene-getter-log-link-stub.c", sources)
                    else:
                        self.assertIn("scene-getter-log-link-stub.c", sources)
        self.assertEqual(legacy, {"test_scene_item_crop_rust"})

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
                               "ENABLE_WAYLAND", "HAVE_WAYLAND_CLIENT_TEST_SDK", "HAVE_WAYLAND_SERVER_TEST_SDK", "HAVE_NIX_WAYLAND_KEYS_TEST_SDK",
                               "HAVE_PIPEWIRE_FRAME_LAYOUT_SDK", "HAVE_PULSE_DEFAULT_RETRY_SDK",
                               "HAVE_OBS_V4L2", "HAVE_V4L2_BUSY_SDK", "HAVE_V4L2_CAPTURE_SDK", "HAVE_V4L2_UDEV_SDK", "HAVE_FREETYPE_PLATFORM_SDK", "HAVE_OBS_TEXT_FREETYPE2", "HAVE_FREETYPE_FILE_SDK",
                               "HAVE_LINUX_CAPTURE", "HAVE_PIPEWIRE_HEADLESS_SDK", "HAVE_PIPEWIRE_LOOP_TEST_SDK", "HAVE_PYTHON_EMBED_TEST_SDK", "HAVE_QSV_TEST_SDK", "HAVE_TEXT_FILE_OWNER", "HAVE_HOTKEY_BACKGROUND_JSON", "HAVE_PLUGIN_GETTER_OWNERS", "HAVE_MBEDTLS_TEST_SDK", "HAVE_RTMPS_TEST_SDK", "HAVE_LIBRIST_TEST_SDK", "HAVE_CURL_TEST_SDK",
                               "HAVE_FONTCONFIG_TEST_OWNER", "HAVE_FRONTEND_WARN_TEST_OWNER", "HAVE_FRONTEND_PLAIN_TEST_OWNER", "HAVE_X264_TEST_OWNER", "ENABLE_CORE_MODULES", "HAVE_PREFLIGHT_TEST_ARCHIVE", "HAVE_SCRIPTING_DEFS_TEST_SDK", "HAVE_MEDIA_PLAYBACK_TEST_OWNER", "HAVE_TRANSITIONS_TEST_OWNER", "HAVE_STARTUP_GLAD", "HAVE_SRT_TEST_SDK", "HAVE_MEDIA_PACKET_TEST_SDK", "C_COMPILER_CLANG", "HAVE_OBS_FFMPEG", "HAVE_OBS_QSV11", "HAVE_OBS_OPENGL"), False)
        facts.update(HAVE_X11_DISPLAY=False, HAVE_QT_WIDGETS_TEST_OWNER=False, HAVE_QT_WIDGETS_TESTLIB_OWNER=False,
                     HAVE_PLUGIN_MANAGER_TEST_OWNER=False, HAVE_VST_EDITOR_TEST_OWNER=False,
                     HAVE_PRIVATE_DAEMON_TEST_OWNER=False, HAVE_PIPEWIRE_CAMERA_TEST_OWNER=False,
                     HAVE_V4L2_VMIC_TEST_OWNER=False)
        facts.update(dict.fromkeys(("OS_LINUX", "UNIX", "HAVE_LIBOBS", "HAVE_LIBOBS_RUST_STATIC",
                                    "HAVE_HAPPY_EYEBALLS", "HAVE_OBS_OUTPUTS", "HAVE_OBS_FILTERS",
                                     "HAVE_OBS_ALSA", "HAVE_OBS_PIPEWIRE", "HAVE_OBS_PULSEAUDIO"), True))

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
            self.assertEqual(len(inventories[True, hevc]) - len(inventories[False, hevc]), 6)
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
