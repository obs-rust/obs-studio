#!/usr/bin/env -S uv run --script
"""Standalone Rust-static fixtures must not eagerly link the core global."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class CorePrivateDataLinkSupport(unittest.TestCase):
    @unittest.skipUnless(shutil.which("cmake") and shutil.which("cc"), "requires CMake and C")
    def test_real_adapter_is_available_without_eager_core_linkage(self):
        with tempfile.TemporaryDirectory(prefix="obs-private-data-link-") as directory:
            source = Path(directory)
            (source / "main.c").write_text("int main(void) { return 0; }\n")
            (source / "CMakeLists.txt").write_text(
                f'''cmake_minimum_required(VERSION 3.28)
project(private_data_link C)
set(CMAKE_SOURCE_DIR "{ROOT.as_posix()}")
set(CMAKE_CURRENT_SOURCE_DIR "{(ROOT / 'test/cmocka').as_posix()}")
foreach(dependency libobs_rust-static OBS::caption Uthash::Uthash OBS::w32-pthreads)
  add_library(${{dependency}} INTERFACE IMPORTED)
endforeach()
foreach(consumer test_video_render_size_rust test_source_link_steps_rust)
  add_executable(${{consumer}} "{(source / 'main.c').as_posix()}")
  target_link_libraries(${{consumer}} PRIVATE libobs_rust-static)
endforeach()
include("{(ROOT / 'test/cmocka/tests.d/core_private_data_link_support.cmake').as_posix()}")
function(check_adapter_selection)
  foreach(consumer test_video_render_size_rust test_source_link_steps_rust)
    get_target_property(sources ${{consumer}} SOURCES)
    if(sources MATCHES "core-private-data-link-support[.]c")
      message(FATAL_ERROR "${{consumer}} eagerly links the adapter and its unresolved obs global")
    endif()
    get_target_property(links ${{consumer}} LINK_LIBRARIES)
    set(found FALSE)
    foreach(link IN LISTS links)
      if(TARGET "${{link}}")
        get_target_property(kind "${{link}}" TYPE)
        get_target_property(adapter_sources "${{link}}" SOURCES)
        if(kind STREQUAL "STATIC_LIBRARY" AND adapter_sources MATCHES "core-private-data-link-support[.]c")
          set(found TRUE)
        endif()
      endif()
    endforeach()
    if(NOT found)
      message(FATAL_ERROR "${{consumer}} cannot select the real adapter on demand")
    endif()
  endforeach()
endfunction()
cmake_language(DEFER CALL check_adapter_selection)
'''
            )
            result = subprocess.run(
                ["cmake", "-S", str(source), "-B", str(source / "build")],
                capture_output=True, text=True, timeout=30,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
