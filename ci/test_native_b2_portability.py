#!/usr/bin/env -S uv run --script
"""Batch 2 native contracts must carry portable header dependencies."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class NativeB2Portability(unittest.TestCase):
    def test_filter_pch_imports_windows_pthreads(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        dependency = {"name": "OBS::w32-pthreads", "requires": "OS_WINDOWS"}
        for name in ("test_async_delay_pacing", "test_dynamics_threshold", "test_expander_unity"):
            with self.subTest(name=name):
                self.assertIn(dependency, graph["targets"][name]["libraries"])

    def test_evaluated_linux_core_inventory(self):
        graph = json.loads((ROOT / "test/cmocka/build/test-graph.json").read_text())
        facts = dict.fromkeys(("OS_WINDOWS", "OS_MACOS", "OS_FREEBSD", "OS_OPENBSD", "MSVC",
                               "ENABLE_WAYLAND", "HAVE_WAYLAND_CLIENT_TEST_SDK", "HAVE_WAYLAND_SERVER_TEST_SDK",
                               "HAVE_PIPEWIRE_FRAME_LAYOUT_SDK", "HAVE_PULSE_DEFAULT_RETRY_SDK",
                               "HAVE_OBS_V4L2", "HAVE_V4L2_BUSY_SDK", "HAVE_V4L2_CAPTURE_SDK", "HAVE_V4L2_UDEV_SDK", "HAVE_FREETYPE_PLATFORM_SDK", "HAVE_OBS_TEXT_FREETYPE2", "HAVE_FREETYPE_FILE_SDK",
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

        for pulse in (False, True):
            for hevc in (False, True):
                facts.update(ENABLE_PULSEAUDIO=pulse, ENABLE_HEVC=hevc)
                selected = [name for name, test in graph["tests"].items()
                            if test.get("suite", "core") == "core"
                            and enabled(test.get("requires", True))
                            and enabled(graph["targets"][test["target"]].get("requires", True))]
                self.assertEqual(len(selected), 417 + 5 * pulse - (not hevc))
        # Both SDK-enabled additions are independent of core PulseAudio/HEVC.
        facts.update(HAVE_PIPEWIRE_FRAME_LAYOUT_SDK=True, HAVE_PULSE_DEFAULT_RETRY_SDK=True)
        selected = [name for name, test in graph["tests"].items()
                    if test.get("suite", "core") == "core"
                    and enabled(test.get("requires", True))
                    and enabled(graph["targets"][test["target"]].get("requires", True))]
        self.assertIn("test_pipewire_frame_layout", selected)
        self.assertIn("test_pulse_default_retry", selected)
        self.assertEqual(len(selected), 424)
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
