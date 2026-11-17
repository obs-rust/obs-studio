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


class ConfigureChurnTest(unittest.TestCase):
    @unittest.skipUnless(shutil.which("cmake") and shutil.which("ninja") and shutil.which("cc"), "requires CMake, Ninja and C")
    def test_link_support_reconfigure_has_zero_edges(self):
        with tempfile.TemporaryDirectory(prefix="obs-configure-churn-") as directory:
            source = Path(directory)
            build = source / "build"
            (source / "main.c").write_text("int main(void) { return 0; }\n")
            (source / "header.in").write_text('@chosen@ @literal@ ${literal} $<CONFIG> ; "quoted"\n')
            (source / "CMakeLists.txt").write_text(
                f'''cmake_minimum_required(VERSION 3.28)
project(configure_churn C)
include("{(ROOT / 'test/cmocka/write-if-different.cmake').as_posix()}")
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
  if(fragment_text MATCHES "obs_test_write_file|file[(][ \n]*(WRITE|APPEND|CONFIGURE|GENERATE)")
    include("${{fragment}}")
  endif()
endforeach()
set(literal "would be substituted")
obs_test_write_file("${{CMAKE_CURRENT_BINARY_DIR}}/literal.c" [==[@literal@ ${{literal}} $<CONFIG> ; "quoted"
]==])
include("{(ROOT / 'test/cmocka/native-test-configured-headers.cmake').as_posix()}")
function(_obs_test_graph_fields)
endfunction()
_obs_test_graph_configured_header(
  [==[{{"targets":{{"test_graphics_steps_rust":{{"configured_headers":{{"literal.h":{{"input":"header.in","values":{{"chosen":"replacement"}}}}}}}}}}}}]==]
  test_graphics_steps_rust "{source.as_posix()}" literal.h)
# Run after the fragments' deferred generators. Only observe their outputs:
# no OBS SDK, Rust toolchain or compilation of the real adapters is needed.
function(observe_generated_inputs)
  file(GLOB inputs "${{CMAKE_CURRENT_BINARY_DIR}}/*.c")
  file(GLOB headers "${{CMAKE_CURRENT_BINARY_DIR}}/native-tests/*/configured/*.h")
  list(APPEND inputs ${{headers}})
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
            inputs = sorted([*build.glob("*.c"), *build.glob("native-tests/*/configured/*.h")])
            self.assertGreaterEqual(len(inputs), 24, "fixture did not exercise the link-support generators")
            contents = {path: path.read_bytes() for path in inputs}
            self.assertEqual(contents[build / "literal.c"], b'@literal@ ${literal} $<CONFIG> ; "quoted"\n')
            header = build / "native-tests/test_graphics_steps_rust/configured/literal.h"
            self.assertEqual(contents[header], b'replacement @literal@ ${literal} $<CONFIG> ; "quoted"\n')
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
            # A real content change must still regenerate the input and its edge.
            (build / "literal.c").write_text("changed\n")
            run(*configure)
            self.assertEqual((build / "literal.c").read_bytes(), contents[build / "literal.c"])
            changed = run("ninja", "-C", str(build), "-n", "-d", "explain", "observe")
            self.assertEqual(len(re.findall(r"^\[\d+/\d+\]", changed, re.MULTILINE)), 1, changed)

    def test_test_fragments_do_not_rewrite_generated_inputs(self):
        # Includes extracted headers and runtime fixtures beyond link support.
        offenders = [path.name for path in FRAGMENTS.glob("*.cmake")
                     if re.search(r"\bfile\(\s*(?:WRITE|APPEND)\b", path.read_text(), re.IGNORECASE)]
        self.assertEqual(offenders, [], "use a content-preserving CMake writer")


if __name__ == "__main__":
    unittest.main()
