"""Exercise production/reference selection and installation through real CMake."""

import ctypes
import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


@unittest.skipUnless(shutil.which("cmake") and shutil.which("ninja"), "CMake and Ninja required")
class HybridDefaultTest(unittest.TestCase):
    def configure(self, directory, *options):
        source = Path(directory)
        (source / "common.c").write_text("int common(void) { return 7; }\n")
        (source / "reference.c").write_text("int implementation(void) { return 1; }\n")
        # Stand in for the Rust staticlib's exported ABI, exercising the real
        # selection module without downloading a toolchain or OBS dependencies.
        (source / "hybrid.c").write_text("int implementation(void) { return 2; }\n")
        (source / "CMakeLists.txt").write_text(
            'cmake_minimum_required(VERSION 3.28)\nproject(Selection C)\nset(CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON)\n'
            f'include("{ROOT}/libobs/cmake/implementation.cmake")\n'
            'obs_libobs_create_targets()\n'
            'target_sources(libobs-objects PRIVATE common.c)\n'
            'obs_libobs_add_c_sources(reference.c)\n'
            'if(OBS_LIBOBS_RUST_TARGET)\n'
            '  target_sources(${OBS_LIBOBS_RUST_TARGET} PRIVATE hybrid.c)\n'
            'endif()\n'
            'foreach(target IN LISTS LIBOBS_SHARED_TARGETS)\n'
            '  target_link_libraries(${target} PRIVATE libobs-objects)\n'
            'endforeach()\n'
            'if(TARGET libobs-c-reference)\n'
            '  set_target_properties(libobs-c-reference PROPERTIES OUTPUT_NAME obs\n'
            '    LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/c-reference" RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/c-reference")\n'
            '  file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/reference.txt" CONTENT "$<TARGET_FILE:libobs-c-reference>")\n'
            'endif()\n'
            'set_target_properties(libobs PROPERTIES OUTPUT_NAME obs)\n'
            'file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/production.txt" CONTENT "$<TARGET_FILE:libobs>")\n'
            'install(TARGETS libobs LIBRARY DESTINATION lib RUNTIME DESTINATION lib)\n'
        )
        build = source / "build"
        subprocess.run(["cmake", "-S", str(source), "-B", str(build), "-G", "Ninja", *options], check=True, capture_output=True, text=True)
        return build

    def build(self, build, *targets):
        result = subprocess.run(["cmake", "--build", str(build), *targets], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_default_build_and_install_use_hybrid_without_reference_objects(self):
        with tempfile.TemporaryDirectory() as directory:
            build = self.configure(directory)
            self.build(build)
            inputs = subprocess.check_output(["ninja", "-C", str(build), "-t", "inputs", "all"], text=True)
            self.assertNotIn("reference.c.o", inputs)
            self.assertEqual(ctypes.CDLL((build / "production.txt").read_text()).implementation(), 2)
            prefix = Path(directory) / "install"
            subprocess.run(["cmake", "--install", str(build), "--prefix", str(prefix)], check=True, capture_output=True)
            self.assertEqual(ctypes.CDLL(str(prefix / "lib" / Path((build / "production.txt").read_text()).name)).implementation(), 2)
            self.assertFalse((build / "reference.txt").exists() and Path((build / "reference.txt").read_text()).exists())

    def test_reference_is_explicit_and_does_not_replace_installed_hybrid(self):
        with tempfile.TemporaryDirectory() as directory:
            build = self.configure(directory, "-DOBS_BUILD_C_REFERENCE=ON")
            self.build(build)
            self.assertFalse((build / "reference.txt").exists() and Path((build / "reference.txt").read_text()).exists())
            self.build(build, "--target", "libobs-c-reference")
            self.assertEqual(ctypes.CDLL((build / "reference.txt").read_text()).implementation(), 1)
            self.assertEqual(ctypes.CDLL((build / "production.txt").read_text()).implementation(), 2)

    def test_explicit_rollback_uses_c_without_a_rust_target(self):
        with tempfile.TemporaryDirectory() as directory:
            build = self.configure(directory, "-DOBS_LIBOBS_BACKEND=C")
            self.build(build)
            self.assertEqual(ctypes.CDLL((build / "production.txt").read_text()).implementation(), 1)
            inputs = subprocess.check_output(["ninja", "-C", str(build), "-t", "inputs", "all"], text=True)
            self.assertNotIn("hybrid.c.o", inputs)

    def test_unknown_backend_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaises(subprocess.CalledProcessError):
                self.configure(directory, "-DOBS_LIBOBS_BACKEND=typo")

    def test_default_gives_pinned_ported_files_an_include_owner(self):
        # A test that compiles one ported implementation file itself (test_avc
        # pins obs-avc.c.h) takes that file's include directories from the
        # target that owns its group (cmake/common/groups.cmake). In the default
        # hybrid no C target compiles the ported groups, which left the file
        # without libobs's usage requirements (caption/caption.h not found).
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            # groups.cmake takes the repo root from its own location.
            (source / "cmake" / "common").mkdir(parents=True)
            shutil.copy(ROOT / "cmake" / "common" / "groups.cmake", source / "cmake" / "common")
            (source / "libobs" / "build").mkdir(parents=True)
            (source / "libobs" / "build" / "mini.ported+.h.c").write_text(
                '#include "libobs/build/mini.pch.h"\n#include "libobs/mini.c.h"\n'
            )
            (source / "libobs" / "build" / "mini.pch.h").write_text("#pragma once\n#include <owner.h>\n")
            (source / "libobs" / "mini.c.h").write_text(
                '#include "libobs/build/mini.pch.h"\nint mini(void) { return OWNER_VALUE; }\n'
            )
            (source / "owner-include").mkdir()
            (source / "owner-include" / "owner.h").write_text("#define OWNER_VALUE 3\n")
            (source / "common.c").write_text("int common(void) { return 7; }\n")
            (source / "main.c").write_text("int mini(void);\nint main(void) { return mini() == 3 ? 0 : 1; }\n")
            (source / "CMakeLists.txt").write_text(
                'cmake_minimum_required(VERSION 3.28)\nproject(Owner C)\n'
                'include("${CMAKE_SOURCE_DIR}/cmake/common/groups.cmake")\n'
                f'include("{ROOT}/libobs/cmake/implementation.cmake")\n'
                'obs_libobs_create_targets()\n'
                'target_sources(libobs-objects PRIVATE common.c)\n'
                'target_include_directories(libobs-objects PUBLIC "${CMAKE_SOURCE_DIR}/owner-include")\n'
                'obs_libobs_add_c_sources("${CMAKE_SOURCE_DIR}/libobs/build/mini.ported+.h.c")\n'
                'target_link_libraries(libobs PRIVATE libobs-objects)\n'
                'add_executable(pinned main.c libobs/mini.c.h)\n'
                'target_link_libraries(pinned PRIVATE libobs)\n'
                '_obs_groups_all("${CMAKE_SOURCE_DIR}")\n'
                '_obs_target_impl_languages(pinned)\n'
            )
            build = source / "build"
            subprocess.run(["cmake", "-S", str(source), "-B", str(build), "-G", "Ninja"], check=True, capture_output=True, text=True)
            self.build(build, "--target", "pinned")
            self.assertEqual(subprocess.run([str(build / "pinned")]).returncode, 0)
