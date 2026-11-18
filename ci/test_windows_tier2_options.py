#!/usr/bin/env -S uv run --script
"""Exercise the cross-only option adapter using CMake's generated commands."""

import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(shutil.which("clang") and shutil.which("ninja"), "needs clang/Ninja")
class DriverOptionsTests(unittest.TestCase):
    def commands(self, frontend):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "test.c").write_text("int main(void) { return 0; }\n")
            (root / "CMakeLists.txt").write_text(
                'cmake_minimum_required(VERSION 3.30)\nproject(options C)\n'
                f'set(CMAKE_C_COMPILER_FRONTEND_VARIANT {frontend})\n'
                'set(CMAKE_C_SIMULATE_ID MSVC)\n'
                f'include("{ROOT}/cmake/cross/windows-tier2-options.cmake")\n'
                'add_executable(probe test.c)\n'
                'target_compile_options(probe PRIVATE /O1 /fp:strict /std:c11 '
                '/experimental:c11atomics /clang:-fcommon /Dprobe=1 "$<$<CONFIG:Release>:/O1>")\n'
                'target_link_options(probe PRIVATE /FORCE:MULTIPLE /INCREMENTAL:NO)\n'
                'obs_windows_tier2_driver_options("${CMAKE_CURRENT_SOURCE_DIR}")\n'
                'file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/link.txt" '
                'CONTENT "$<TARGET_PROPERTY:probe,LINK_OPTIONS>")\n'
            )
            result = subprocess.run(
                ["cmake", "-S", str(root), "-B", str(root / "build"), "-G", "Ninja",
                 "-DCMAKE_C_COMPILER=clang", "-DCMAKE_BUILD_TYPE=Release",
                 "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON"],
                capture_output=True, text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            command = json.loads((root / "build/compile_commands.json").read_text())[0]["command"]
            return command, (root / "build/link.txt").read_text()

    def test_native_layout_keeps_unchanged_tests_out_of_generic_unity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cross = root / "cmake/cross"
            tests = root / "test/cmocka"
            cross.mkdir(parents=True)
            tests.mkdir(parents=True)
            for name in ("first", "second"):
                (tests / f"{name}.c").write_text("static int collision; int " + name + "(void) { return collision; }\n")
            (tests / "CMakeLists.txt").write_text("add_library(characterization OBJECT first.c second.c)\n")
            (cross / "CMakeLists.txt").write_text(
                'cmake_minimum_required(VERSION 3.30)\nproject(layout C)\n'
                f'list(PREPEND CMAKE_MODULE_PATH "{ROOT}/cmake/common")\n'
                'set(OBS_SOURCE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../..")\n'
                'include(pch)\ninclude(groups)\n'
                f'include("{ROOT}/cmake/cross/windows-tier2-options.cmake")\n'
                'add_subdirectory("${OBS_SOURCE_ROOT}/test/cmocka" tests)\n'
                'obs_windows_tier2_native_layout()\n'
            )
            result = subprocess.run(
                ["cmake", "-S", str(cross), "-B", str(root / "build"), "-G", "Ninja",
                 "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON"], capture_output=True, text=True,
            )
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            commands = json.loads((root / "build/compile_commands.json").read_text())
            self.assertEqual({"first.c", "second.c"}, {Path(c["file"]).name for c in commands})
            subprocess.run(["cmake", "--build", str(root / "build")], check=True, capture_output=True)

    def test_gnu_frontend_receives_equivalent_options(self):
        command, link = self.commands("GNU")
        for option in ("-Os", "-ffp-model=strict", "-std=c11", "-fcommon", "-Dprobe=1"):
            self.assertIn(option, command)
        for option in ("/O1", "/fp:", "/std:", "/experimental:", "/clang:"):
            self.assertNotIn(option, command)
        self.assertEqual("LINKER:/FORCE:MULTIPLE;LINKER:/INCREMENTAL:NO", link)

    def test_msvc_frontend_retains_original_options(self):
        command, link = self.commands("MSVC")
        for option in ("/O1", "/fp:strict", "/std:c11", "/experimental:c11atomics", "/clang:-fcommon", "/Dprobe=1"):
            self.assertIn(option, command)
        self.assertNotIn("-ffp-model", command)
        self.assertEqual("/FORCE:MULTIPLE;/INCREMENTAL:NO", link)


if __name__ == "__main__":
    unittest.main()
