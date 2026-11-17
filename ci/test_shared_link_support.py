#!/usr/bin/env -S uv run --script
"""Count native support compiles and preserve context and link boundaries."""

import json
from pathlib import Path
import shutil
import shlex
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
                "int support_value(void) { return HEADER_VALUE + EXTRA; }\n"
            )
            (source / "main.c").write_text(
                '#include "value.h"\n'
                "#ifndef EXTRA\n#define EXTRA 0\n#endif\n"
                "#ifndef EXPECTED\n#define EXPECTED (HEADER_VALUE + EXTRA)\n#endif\n"
                "extern int support_value(void);\nint eager_effect;\n"
                "int __wrap_support_value(void) { return 99; }\n"
                "int main(void) { return eager_effect != 7 || support_value() != EXPECTED; }\n"
            )
            (source / "eager-link-support.c").write_text(
                "extern int eager_effect;\n"
                "__attribute__((constructor)) static void eager_init(void) { eager_effect = 7; }\n"
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
                "add_library(context_headers INTERFACE)",
                "target_include_directories(context_headers INTERFACE first)",
                'target_include_directories(existing_support INTERFACE "$<TARGET_PROPERTY:context_headers,INTERFACE_INCLUDE_DIRECTORIES>")',
                'target_sources(existing_support INTERFACE "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/first/value.h>")',
                "foreach(name IN ITEMS same_a same_b define ordered flag wrapped)",
                "  add_executable(${name} main.c)",
                "  target_link_libraries(${name} PRIVATE libobs_rust-static existing_support)",
                "  target_include_directories(${name} PRIVATE first second)",
                "endforeach()",
                # CMake removes these duplicate usage requirements before compiling.
                "target_link_libraries(same_b PRIVATE existing_support)",
                "target_include_directories(same_b PRIVATE first second)",
                "target_compile_definitions(define PRIVATE EXTRA=3)",
                'set_property(TARGET ordered PROPERTY INCLUDE_DIRECTORIES "${CMAKE_CURRENT_SOURCE_DIR}/second;${CMAKE_CURRENT_SOURCE_DIR}/first")',
                "target_compile_options(flag PRIVATE -fno-omit-frame-pointer)",
                "set_property(TARGET wrapped PROPERTY COMPILE_DEFINITIONS EXPECTED=99)",
                "target_link_options(wrapped PRIVATE -Wl,--wrap=support_value)",
                "function(add_deferred_support)",
                "  foreach(name IN ITEMS same_a same_b define ordered flag wrapped)",
                "    target_sources(${name} PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/probe-link-support.c ${CMAKE_CURRENT_SOURCE_DIR}/eager-link-support.c)",
                "  endforeach()",
                "endfunction()", "cmake_language(DEFER CALL add_deferred_support)",
                # Existing builds are the RED baseline; this also tests the shared rule's integration.
                'if(SHARE_SUPPORT)',
                f'include("{HELPER.as_posix()}" OPTIONAL)',
                "if(COMMAND obs_share_test_link_support)",
                "  cmake_language(DEFER CALL obs_share_test_link_support)",
                "endif()", "endif()",
            ]
            (source / "CMakeLists.txt").write_text("\n".join(lines) + "\n")
            build = source / "build"
            self.run_command(["cmake", "-S", str(source), "-B", str(build), "-DSHARE_SUPPORT=OFF"])
            baseline = json.loads((build / "compile_commands.json").read_text())
            self.run_command(["cmake", "-S", str(source), "-B", str(build), "-DSHARE_SUPPORT=ON"])
            commands = json.loads((build / "compile_commands.json").read_text())
            support = [row for row in commands if row["file"].endswith("/probe-link-support.c")]
            self.assertEqual(len(support), 5, f"expected 5 contexts, got {len(support)} support compiles")
            # The output path is the only compiler argument allowed to change.
            def contexts(rows):
                result = set()
                for row in rows:
                    args = shlex.split(row["command"])
                    index = args.index("-o")
                    del args[index:index + 2]
                    result.add(tuple(args))
                return result
            self.assertEqual(contexts(baseline), contexts(commands))
            self.run_command(["cmake", "--build", str(build), "--parallel", "2"])
            for name in ("same_a", "same_b", "define", "ordered", "flag", "wrapped"):
                self.run_command([str(build / name)])
            # A changed header must rebuild the shared object for both consumers.
            (source / "first/value.h").write_text("#define HEADER_VALUE 11\n/* changed */\n")
            self.run_command(["cmake", "--build", str(build), "--parallel", "2"])

    def test_conditional_dependency_keeps_consumer_sensitive_definitions(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "context-link-support.c").write_text(
                "#ifndef EXECUTABLE_CONTEXT\n#error lost consumer context\n#endif\n"
                "int support(void) { return 0; }\n"
            )
            (source / "main.c").write_text("int support(void); int main(void) { return support(); }\n")
            (source / "ordinary-link-support.c").write_text("int support(void) { return 0; }\n")
            (source / "interface.c").write_text("int interface_symbol = 7;\n")
            (source / "CMakeLists.txt").write_text(
                "cmake_minimum_required(VERSION 3.28)\nproject(conditional_context C)\n"
                "set(CMAKE_EXPORT_COMPILE_COMMANDS ON)\n"
                "add_library(libobs_rust-static INTERFACE)\n"
                "add_library(context INTERFACE)\n"
                'target_compile_definitions(context INTERFACE "$<$<STREQUAL:$<TARGET_PROPERTY:TYPE>,EXECUTABLE>:EXECUTABLE_CONTEXT>")\n'
                "foreach(name IN ITEMS test_a test_b)\n"
                "  add_executable(${name} main.c context-link-support.c)\n"
                '  target_link_libraries(${name} PRIVATE libobs_rust-static "$<BUILD_INTERFACE:context>")\n'
                "endforeach()\n"
                "add_library(compiled_interface INTERFACE)\n"
                'target_sources(compiled_interface INTERFACE "${CMAKE_CURRENT_SOURCE_DIR}/interface.c")\n'
                "foreach(name IN ITEMS test_c test_d)\n"
                "  add_executable(${name} main.c ordinary-link-support.c)\n"
                "  target_link_libraries(${name} PRIVATE libobs_rust-static compiled_interface)\n"
                "endforeach()\n"
                f'include("{HELPER.as_posix()}")\n'
                "cmake_language(DEFER CALL obs_share_test_link_support)\n"
            )
            build = source / "build"
            self.run_command(["cmake", "-S", str(source), "-B", str(build)])
            commands = json.loads((build / "compile_commands.json").read_text())
            self.assertEqual(sum(row["file"].endswith("/context-link-support.c") for row in commands), 2)
            self.assertEqual(sum(row["file"].endswith("/ordinary-link-support.c") for row in commands), 2)
            self.run_command(["cmake", "--build", str(build), "--parallel", "2"])
            self.run_command([str(build / "test_a")])
            self.run_command([str(build / "test_b")])

    def run_command(self, command):
        result = subprocess.run(command, capture_output=True, text=True, check=False, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
