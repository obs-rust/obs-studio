#!/usr/bin/env -S uv run --script
"""Configure the real visibility fixture fragment for each host condition."""

import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
NAME = "test_scene_item_set_visibility"
FRAGMENT = ROOT / "test/cmocka/tests.d" / f"{NAME}.cmake"


@unittest.skipUnless(shutil.which("cmake") and shutil.which("ctest"), "CMake/CTest required")
class SceneVisibilityRegistration(unittest.TestCase):
    def configure(self, host, graphics=True):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        source = Path(temporary.name)
        build = source / "build"
        # Configure the unchanged real C fixture; no graphics code executes here.
        shutil.copyfile(ROOT / "test/cmocka" / f"{NAME}.c", source / f"{NAME}.c")
        backend = (
            "add_library(libobs-opengl SHARED IMPORTED)\n"
            "add_library(OBS::libobs-opengl ALIAS libobs-opengl)\n"
            'set_target_properties(libobs-opengl PROPERTIES IMPORTED_LOCATION "${CMAKE_BINARY_DIR}/opengl.so")\n'
            if graphics else ""
        )
        flags = "".join(f"set(OS_{name} {'ON' if host == name else 'OFF'})\n"
                        for name in ("LINUX", "MACOS", "WINDOWS"))
        (source / "CMakeLists.txt").write_text(
            "cmake_minimum_required(VERSION 3.28)\n"
            "project(visibility_registration C)\nenable_testing()\n"
            "set(CMAKE_BUILD_TYPE RelWithDebInfo)\n"
            "add_library(OBS::libobs INTERFACE IMPORTED)\n"
            "add_library(OBS::w32-pthreads INTERFACE IMPORTED)\n"
            "add_custom_target(cmocka-tests)\n"
            + flags + backend + f'include("{FRAGMENT.as_posix()}")\n'
            + "get_target_property(dependencies cmocka-tests MANUALLY_ADDED_DEPENDENCIES)\n"
            + 'file(WRITE "${CMAKE_BINARY_DIR}/dependencies.txt" "${dependencies}")\n'
        )
        configured = subprocess.run(["cmake", "-S", str(source), "-B", str(build)],
                                    capture_output=True, text=True, timeout=60)
        self.assertEqual(configured.returncode, 0, configured.stdout + configured.stderr)
        listed = subprocess.run(["ctest", "--test-dir", str(build), "-C", "RelWithDebInfo",
                                 "--show-only=json-v1"], capture_output=True, text=True, timeout=30)
        self.assertEqual(listed.returncode, 0, listed.stdout + listed.stderr)
        return build, json.loads(listed.stdout)["tests"]

    def test_linux_fixture_has_an_existing_target_working_directory(self):
        build, tests = self.configure("LINUX")
        self.assertEqual([test["name"] for test in tests], [NAME])
        properties = {entry["name"]: entry["value"] for entry in tests[0]["properties"]}
        self.assertEqual(Path(properties["WORKING_DIRECTORY"]), build)
        self.assertTrue(Path(properties["WORKING_DIRECTORY"]).is_dir())
        self.assertIn(NAME, (build / "dependencies.txt").read_text().split(";"))

    def test_macos_does_not_register_the_linux_opengl_fixture(self):
        _, tests = self.configure("MACOS")
        self.assertEqual(tests, [])

    def test_windows_does_not_register_the_linux_opengl_fixture(self):
        _, tests = self.configure("WINDOWS")
        self.assertEqual(tests, [])

    def test_missing_opengl_does_not_register_an_unbuilt_test(self):
        _, tests = self.configure("LINUX", graphics=False)
        self.assertEqual(tests, [])


if __name__ == "__main__":
    unittest.main()
