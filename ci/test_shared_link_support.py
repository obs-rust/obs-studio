#!/usr/bin/env -S uv run --script
"""Count native support compiles and preserve context and link boundaries."""

import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "test/cmocka/shared-link-support.cmake"


@unittest.skipUnless(shutil.which("cmake") and shutil.which("cc"), "requires CMake and C compiler")
class SharedLinkSupport(unittest.TestCase):
    def test_compiles_once_per_complete_context_and_keeps_eager_objects(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            for name, value in (("first", 11), ("second", 22)):
                (source / name).mkdir()
                (source / name / "value.h").write_text(f"#define HEADER_VALUE {value}\n")
            (source / "probe-link-support.c").write_text(
                '#include "value.h"\n'
                "#ifndef EXTRA\n#define EXTRA 0\n#endif\n"
                "int eager_symbol = 7;\n"
                "int support_value(void) { return HEADER_VALUE + EXTRA; }\n"
            )
            (source / "main.c").write_text(
                "extern int support_value(void);\nextern int eager_symbol;\n"
                "int __wrap_support_value(void) { return 99; }\n"
                "int main(void) { return eager_symbol != 7 || support_value() != EXPECTED; }\n"
            )
            (source / "archive-link-support.c").write_text(
                "extern int unavailable_host(void);\n"
                "int unused_adapter(void) { return unavailable_host(); }\n"
            )
            lines = [
                "cmake_minimum_required(VERSION 3.28)", "project(shared_support C)",
                "set(CMAKE_EXPORT_COMPILE_COMMANDS ON)",
                "add_library(libobs_rust-static INTERFACE)",
                "add_library(existing_support STATIC archive-link-support.c)",
                "foreach(name IN ITEMS same_a same_b define ordered flag wrapped)",
                "  add_executable(${name} main.c probe-link-support.c)",
                "  target_link_libraries(${name} PRIVATE libobs_rust-static existing_support)",
                "  target_include_directories(${name} PRIVATE first second)",
                "  target_compile_definitions(${name} PRIVATE EXPECTED=11)",
                "endforeach()",
                "set_property(TARGET define PROPERTY COMPILE_DEFINITIONS EXPECTED=14 EXTRA=3)",
                'set_property(TARGET ordered PROPERTY INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/second;${CMAKE_CURRENT_SOURCE_DIR}/first")',
                "set_property(TARGET ordered PROPERTY COMPILE_DEFINITIONS EXPECTED=22)",
                "target_compile_options(flag PRIVATE -fno-omit-frame-pointer)",
                "set_property(TARGET wrapped PROPERTY COMPILE_DEFINITIONS EXPECTED=99)",
                "target_link_options(wrapped PRIVATE -Wl,--wrap=support_value)",
                # Existing builds are the RED baseline; this also tests the shared rule's integration.
                f'include("{HELPER.as_posix()}" OPTIONAL)',
                "if(COMMAND obs_share_test_link_support)",
                "  cmake_language(DEFER CALL obs_share_test_link_support)",
                "endif()",
            ]
            (source / "CMakeLists.txt").write_text("\n".join(lines) + "\n")
            build = source / "build"
            self.run_command(["cmake", "-S", str(source), "-B", str(build)])
            commands = json.loads((build / "compile_commands.json").read_text())
            support = [row for row in commands if row["file"].endswith("/probe-link-support.c")]
            self.assertEqual(len(support), 5, f"expected 5 contexts, got {len(support)} support compiles")
            self.run_command(["cmake", "--build", str(build), "--parallel", "2"])
            for name in ("same_a", "same_b", "define", "ordered", "flag", "wrapped"):
                self.run_command([str(build / name)])
            # A changed header must rebuild the shared object for both consumers.
            (source / "first/value.h").write_text("#define HEADER_VALUE 11\n/* changed */\n")
            self.run_command(["cmake", "--build", str(build), "--parallel", "2"])

    def run_command(self, command):
        result = subprocess.run(command, capture_output=True, text=True, check=False, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
