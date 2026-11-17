#!/usr/bin/env -S uv run --script
"""Reconfiguring the real Tier 2 link-support fragments must leave Ninja idle."""

import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parent.parent
FRAGMENTS = ROOT / "test/cmocka/tests.d"


@unittest.skipUnless(shutil.which("cmake") and shutil.which("ninja") and shutil.which("cc"), "requires CMake, Ninja and C")
class ConfigureChurnTest(unittest.TestCase):
    def test_link_support_reconfigure_has_zero_edges(self):
        with tempfile.TemporaryDirectory(prefix="obs-configure-churn-") as directory:
            source = Path(directory)
            build = source / "build"
            (source / "main.c").write_text("int main(void) { return 0; }\n")
            (source / "CMakeLists.txt").write_text(
                f'''cmake_minimum_required(VERSION 3.28)
project(configure_churn C)
set(CMAKE_SOURCE_DIR "{ROOT.as_posix()}")
set(CMAKE_CURRENT_SOURCE_DIR "{(ROOT / 'test/cmocka').as_posix()}")
foreach(dependency libobs_rust-static OBS::caption Uthash::Uthash SIMDe::SIMDe OBS::w32-pthreads)
  add_library(${{dependency}} INTERFACE IMPORTED)
endforeach()
foreach(component avcodec avformat avutil swscale)
  add_library(FFmpeg::${{component}} INTERFACE IMPORTED)
endforeach()
foreach(consumer test_source_configure_steps_rust test_scene_configure_steps_rust test_graphics_steps_rust)
  add_executable(${{consumer}} EXCLUDE_FROM_ALL "{(source / 'main.c').as_posix()}")
  target_link_libraries(${{consumer}} PRIVATE libobs_rust-static)
endforeach()
file(GLOB fragments "{FRAGMENTS.as_posix()}/*_link_support.cmake")
foreach(fragment IN LISTS fragments)
  file(READ "${{fragment}}" fragment_text)
  if(fragment_text MATCHES "file[(][ \n]*(WRITE|APPEND|CONFIGURE|GENERATE)")
    include("${{fragment}}")
  endif()
endforeach()
# Run after the fragments' deferred generators. Only observe their outputs:
# no OBS SDK, Rust toolchain or compilation of the real adapters is needed.
function(observe_generated_inputs)
  file(GLOB inputs "${{CMAKE_CURRENT_BINARY_DIR}}/*.c")
  foreach(input IN LISTS inputs)
    get_filename_component(name "${{input}}" NAME)
    add_custom_command(OUTPUT "${{name}}.observed"
      COMMAND "${{CMAKE_COMMAND}}" -E touch "${{name}}.observed" DEPENDS "${{input}}")
    list(APPEND stamps "${{name}}.observed")
  endforeach()
  add_custom_target(observe DEPENDS ${{stamps}})
endfunction()
cmake_language(DEFER CALL observe_generated_inputs)
'''
            )

            def run(*args):
                result = subprocess.run(args, capture_output=True, text=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                return result.stdout + result.stderr

            configure = ("cmake", "-S", str(source), "-B", str(build), "-G", "Ninja")
            run(*configure)
            inputs = sorted(build.glob("*.c"))
            self.assertGreaterEqual(len(inputs), 12, "fixture did not exercise the link-support generators")
            contents = {path: path.read_bytes() for path in inputs}
            # Backdate inputs before building, avoiding sleeps and coarse-clock races.
            old = time.time_ns() - 10_000_000_000
            for path in inputs:
                os.utime(path, ns=(old, old))
            run("ninja", "-C", str(build), "observe")
            run(*configure)
            dry_run = run("ninja", "-C", str(build), "-n", "-d", "explain", "observe")
            edges = re.findall(r"^\[\d+/\d+\]", dry_run, re.MULTILINE)
            self.assertEqual(len(edges), 0, dry_run)
            self.assertEqual({path: path.read_bytes() for path in inputs}, contents)

    def test_test_fragments_do_not_rewrite_generated_inputs(self):
        # Includes extracted headers and runtime fixtures beyond link support.
        offenders = [path.name for path in FRAGMENTS.glob("*.cmake")
                     if re.search(r"\bfile\(\s*(?:WRITE|APPEND)\b", path.read_text(), re.IGNORECASE)]
        self.assertEqual(offenders, [], "use a content-preserving CMake writer")


if __name__ == "__main__":
    unittest.main()
