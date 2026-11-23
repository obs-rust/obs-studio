#!/usr/bin/env -S uv run --script
"""#3648: the unchanged shutdown fixture needs MSVC's C11 atomics mode."""

from pathlib import Path
import json
import shutil
import subprocess
import tempfile
import unittest


from native_test_graph import load_graph

ROOT = Path(__file__).resolve().parents[1]
GRAPH = ROOT / "test/cmocka/build/test-graph.d"
CONSUMER = ROOT / "test/cmocka/native-test-graph.cmake"


@unittest.skipUnless(shutil.which("cmake"), "CMake is required for target configuration")
class ShutdownSceneMsvcTest(unittest.TestCase):
    def test_target_enables_c11_atomics_only_for_msvc(self):
        for msvc in (False, True):
            with self.subTest(msvc=msvc), tempfile.TemporaryDirectory() as temporary:
                source = Path(temporary)
                # Configure the actual graph entry without compiling a fake test.
                # Native Tier 2 compiles the unchanged fixture on each host.
                graph = load_graph(GRAPH)
                name = "test_shutdown_leaked_scenes"
                selected = {"schema": 1, "targets": {name: graph["targets"][name]},
                            "tests": {name: graph["tests"][name]}}
                (source / "graph.json").write_text(json.dumps(selected))
                (source / "CMakeLists.txt").write_text(
                    "cmake_minimum_required(VERSION 3.28)\n"
                    "project(shutdown_settings C)\nenable_testing()\n"
                    "add_library(OBS::libobs INTERFACE IMPORTED)\n"
                    "add_library(OBS::native-test-cmocka INTERFACE IMPORTED)\n"
                    "set(OS_LINUX OFF)\nset(OS_WINDOWS ON)\nset(HAVE_X11_DISPLAY OFF)\n"
                    "add_custom_target(cmocka-tests)\n"
                    f"set(MSVC {'ON' if msvc else 'OFF'})\n"
                    f'include("{CONSUMER.as_posix()}")\n'
                    f'obs_native_test_graph("${{CMAKE_CURRENT_SOURCE_DIR}}/graph.json" "{ROOT.as_posix()}")\n'
                    "get_target_property(options test_shutdown_leaked_scenes COMPILE_OPTIONS)\n"
                    "get_test_property(test_shutdown_leaked_scenes TIMEOUT timeout)\n"
                    'file(WRITE "${CMAKE_BINARY_DIR}/settings.txt" "${options}\n${timeout}\n")\n'
                )
                build = source / "build"
                result = subprocess.run(
                    ["cmake", "-S", str(source), "-B", str(build)],
                    capture_output=True, text=True, timeout=60,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                options, timeout = (build / "settings.txt").read_text().splitlines()
                self.assertEqual(timeout, "30")
                if msvc:
                    self.assertIn("/std:c11", options.split(";"))
                    self.assertIn("/experimental:c11atomics", options.split(";"))
                else:
                    self.assertNotIn("/std:c11", options.split(";"))
                    self.assertNotIn("/experimental:c11atomics", options.split(";"))


if __name__ == "__main__":
    unittest.main()
