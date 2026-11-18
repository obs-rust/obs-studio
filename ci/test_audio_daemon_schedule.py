"""Check private audio-daemon scheduling in the generated Tier 2 registry (#4944)."""

import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
CMOCKA = ROOT / "test/cmocka"
WRAPPERS = ("test_pulse_monitor_device.py", "test_pipewire_cursor_hdr.py", "test_pipewire_context_fail.py")
FRAGMENTS = ("test_pulse_default_follow", "test_pulse_stream_restart", "test_pipewire_wrapper",
             "test_pipewire_screencast_resume", "test_pipewire_screencast_shm")


@unittest.skipUnless(all(shutil.which(tool) for tool in ("cmake", "ctest", "cc")), "requires CMake/CTest/cc")
class AudioDaemonScheduling(unittest.TestCase):
    def test_private_daemons_share_a_lock_with_or_without_a_display_wrapper(self):
        graph = json.loads((CMOCKA / "build/test-graph.json").read_text())
        tests = {name: test for name, test in graph["tests"].items()
                 if any(Path(arg).name in WRAPPERS for arg in test.get("command", []))
                 or name == "test_v4l2_vmic_server"}
        self.assertGreaterEqual(len(tests), 15)
        # Keep the real graph test registrations; replace only compilation inputs.
        targets = {test["target"]: {"sources": ["stub.c"]} for test in tests.values()}
        for display_wrapper in ("", "/usr/bin/xvfb-run"):
            with self.subTest(display_wrapper=display_wrapper), tempfile.TemporaryDirectory() as directory:
                source = Path(directory)
                (source / "stub.c").write_text("int fixture;\n")
                (source / "graph.json").write_text(json.dumps(
                    {"schema": graph["schema"], "targets": targets, "tests": tests}))
                cmake = '''cmake_minimum_required(VERSION 3.28)
project(audio_daemon_schedule C)
enable_testing()
set(OS_LINUX TRUE)
set(ENABLE_PULSEAUDIO TRUE)
set(HAVE_OBS_PULSEAUDIO TRUE)
set(HAVE_OBS_PIPEWIRE TRUE)
set(HAVE_OBS_V4L2 TRUE)
set(HAVE_PRIVATE_DAEMON_TEST_OWNER TRUE)
set(HAVE_PIPEWIRE_CAMERA_TEST_OWNER TRUE)
set(HAVE_V4L2_VMIC_TEST_OWNER TRUE)
set(HAVE_X11_DISPLAY TRUE)
function(add_executable target)
  _add_executable(${target} "${CMAKE_CURRENT_SOURCE_DIR}/stub.c")
endfunction()
function(find_package)
endfunction()
function(pkg_check_modules prefix)
  set(${prefix}_FOUND TRUE PARENT_SCOPE)
  add_library(PkgConfig::${prefix} INTERFACE IMPORTED)
endfunction()
add_library(fixture INTERFACE)
set_target_properties(fixture PROPERTIES INTERFACE_INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}")
foreach(alias OBS::libobs OBS::pulseaudio OBS::pipewire PulseAudio::PulseAudio
    Libdrm::Libdrm PipeWire::PipeWire gio::gio OBS::glad X11::X11)
  add_library(${alias} ALIAS fixture)
endforeach()
add_library(linux-pulseaudio MODULE stub.c)
add_library(libobs-opengl MODULE stub.c)
add_library(OBS::libobs-opengl ALIAS libobs-opengl)
add_custom_target(cmocka-tests)
set(OBS_OUTPUT_DIR "${CMAKE_BINARY_DIR}/output")
add_test(NAME unrelated COMMAND "${CMAKE_COMMAND}" -E true)
'''
                cmake += f'set(XVFB_RUN "{display_wrapper}")\n'
                cmake += f'set(OBS_TEST_XVFB_RUN "{display_wrapper}")\n'
                for fragment in FRAGMENTS:
                    cmake += f'include("{CMOCKA}/tests.d/{fragment}.cmake")\n'
                cmake += f'include("{CMOCKA}/native-test-graph.cmake")\n'
                cmake += 'obs_native_test_graph("${CMAKE_CURRENT_SOURCE_DIR}/graph.json" "${CMAKE_CURRENT_SOURCE_DIR}")\n'
                cmake += 'set_property(TEST test_pulse_monitor_device APPEND PROPERTY RESOURCE_LOCK existing)\n'
                cmake += f'include("{CMOCKA}/parallel-environment.cmake")\n'
                (source / "CMakeLists.txt").write_text(cmake)
                build = source / "build"
                result = subprocess.run(["cmake", "-S", str(source), "-B", str(build)],
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                result = subprocess.run(["ctest", "--test-dir", str(build), "--show-only=json-v1"],
                                        capture_output=True, text=True, check=True)
                registry = json.loads(result.stdout)["tests"]
                expected = set(tests) | set(FRAGMENTS) | {"test_pulse_default_follow_pipewire"}
                self.assertEqual({test["name"] for test in registry}, expected | {"unrelated"})
                for test in registry:
                    properties = {prop["name"]: prop["value"] for prop in test["properties"]}
                    with self.subTest(test=test["name"]):
                        locks = properties.get("RESOURCE_LOCK", [])
                        self.assertEqual("obs_audio_daemon" in locks, test["name"] in expected)
                        self.assertFalse(properties.get("RUN_SERIAL"))
                        if test["name"] in tests:
                            self.assertEqual(properties["TIMEOUT"], tests[test["name"]]["timeout_seconds"])
                        if test["name"] == "test_pulse_monitor_device":
                            self.assertIn("existing", locks)

    def test_shared_policy_runs_after_both_registration_sources(self):
        cmake = (CMOCKA / "CMakeLists.txt").read_text()
        policy = cmake.index('include("${CMAKE_CURRENT_SOURCE_DIR}/parallel-environment.cmake")')
        self.assertGreater(policy, cmake.index('include("${_fragment}")'))
        self.assertGreater(policy, cmake.index('obs_native_test_graph("${CMAKE_CURRENT_SOURCE_DIR}/build/test-graph.json"'))


if __name__ == "__main__":
    unittest.main()
