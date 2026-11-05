#!/usr/bin/env -S uv run --script
"""#3648: the unchanged shutdown fixture needs MSVC's C11 atomics mode."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
REGISTRATION = ROOT / "test/cmocka/tests.d/test_shutdown_leaked_scenes.cmake"


@unittest.skipUnless(shutil.which("cmake"), "CMake is required for target configuration")
class ShutdownSceneMsvcTest(unittest.TestCase):
    def test_target_enables_c11_atomics_only_for_msvc(self):
        for msvc in (False, True):
            with self.subTest(msvc=msvc), tempfile.TemporaryDirectory() as temporary:
                source = Path(temporary)
                # Only configure the real registration; native Tier 2 compiles
                # the unchanged C fixture on each supported host.
                (source / "test_shutdown_leaked_scenes.c").write_text("int main(void) { return 0; }\n")
                (source / "CMakeLists.txt").write_text(
                    "cmake_minimum_required(VERSION 3.28)\n"
                    "project(shutdown_settings C)\nenable_testing()\n"
                    "add_library(OBS::libobs INTERFACE IMPORTED)\n"
                    "add_custom_target(cmocka-tests)\n"
                    f"set(MSVC {'ON' if msvc else 'OFF'})\n"
                    f'include("{REGISTRATION.as_posix()}")\n'
                    "get_target_property(standard test_shutdown_leaked_scenes C_STANDARD)\n"
                    "get_target_property(required test_shutdown_leaked_scenes C_STANDARD_REQUIRED)\n"
                    "get_target_property(options test_shutdown_leaked_scenes COMPILE_OPTIONS)\n"
                    "get_test_property(test_shutdown_leaked_scenes TIMEOUT timeout)\n"
                    'file(WRITE "${CMAKE_BINARY_DIR}/settings.txt" "${standard}\n${required}\n${options}\n${timeout}\n")\n'
                )
                build = source / "build"
                result = subprocess.run(
                    ["cmake", "-S", str(source), "-B", str(build)],
                    capture_output=True, text=True, timeout=60,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                standard, required, options, timeout = (build / "settings.txt").read_text().splitlines()
                self.assertEqual(timeout, "30")
                if msvc:
                    self.assertEqual(standard, "11")
                    self.assertEqual(required, "ON")
                    self.assertIn("/experimental:c11atomics", options.split(";"))
                else:
                    self.assertNotIn("/experimental:c11atomics", options.split(";"))


if __name__ == "__main__":
    unittest.main()
