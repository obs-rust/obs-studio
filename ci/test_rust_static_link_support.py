#!/usr/bin/env -S uv run --script
"""Exercise the standalone Rust-static support contract with real native links."""

from pathlib import Path
import json
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HELPER = ROOT / "test/cmocka/rust-static-link-support.cmake"


@unittest.skipUnless(shutil.which("cmake") and shutil.which("cc"), "requires CMake and C compiler")
class RustStaticLinkSupport(unittest.TestCase):
    def test_shared_consumer_transitive_host_and_static_allocator(self):
        self.check_fixture(False)

    def test_allocator_member_is_selected_before_shared_import_provider(self):
        self.check_fixture(True)

    def test_non_msvc_preserves_declared_shared_binding(self):
        self.check_fixture(False, False)

    def test_real_logger_members_preserve_fixture_owned_guards(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            (source / "build").mkdir()
            (source / "build/libobs-util.pch.h").write_text(
                "#include <stdarg.h>\n#include <stdio.h>\n#include <stdlib.h>\n"
                "typedef void (*log_handler_t)(int, const char *, va_list, void *);\n"
                "#define LOG_DEBUG 100\n#define LOG_INFO 200\n#define LOG_WARNING 300\n#define LOG_ERROR 400\n"
                "#define UNUSED_PARAMETER(x) (void)(x)\n#define OBS_NORETURN _Noreturn\n"
            )
            manifest = json.loads((ROOT / "test/cmocka/build/rust-static-support.json").read_text())
            objects = []
            for member in manifest["sources"]:
                if not Path(member).name.startswith("base-"):
                    continue
                output = source / (Path(member).stem + ".o")
                self.run_command(["cc", "-std=c11", "-I", str(source), "-I", str(ROOT),
                                  "-c", str(ROOT / member), "-o", str(output)])
                objects.append(str(output))
            archive = source / "support.a"
            self.run_command(["ar", "rcs", str(archive), *objects])
            main = source / "main.c"
            main.write_text(
                '#include "build/libobs-util.pch.h"\n#include <string.h>\n'
                "static int calls, guards;\n"
                "static void handler(int level, const char *fmt, va_list args, void *p) "
                "{ (void)level; (void)fmt; (void)args; (void)p; ++calls; }\n"
                "void base_get_log_handler(log_handler_t *h, void **p) { *h = handler; *p = NULL; }\n"
                "int base_begin_crash(void (**h)(const char *, va_list, void *), void **p, int *d) "
                "{ (void)h; (void)p; *d = 1; return 0; }\n"
                "void blogva(int, const char *, va_list);\n"
                "void base_log_literal(log_handler_t, void *, int, const char *);\n"
                "log_handler_t base_default_log_handler(void);\n"
                "#ifdef FIXTURE_GUARDS\n"
                "void blog(int level, const char *fmt, ...) { (void)level; (void)fmt; ++guards; }\n"
                "_Noreturn void bcrash(const char *fmt, ...) { (void)fmt; exit(77); }\n"
                "#else\nvoid blog(int, const char *, ...);\n_Noreturn void bcrash(const char *, ...);\n#endif\n"
                "static void forward(const char *fmt, ...) "
                "{ va_list args; va_start(args, fmt); blogva(100, fmt, args); va_end(args); }\n"
                "int main(int argc, char **argv) { (void)argv; if (argc > 1) bcrash(\"guard\"); "
                "forward(\"log\"); base_log_literal(handler, NULL, 100, \"literal\"); "
                "blog(100, \"public\"); if (!base_default_log_handler()) return 1;\n"
                "#ifdef FIXTURE_GUARDS\nreturn calls != 2 || guards != 1;\n"
                "#else\nreturn calls != 3;\n#endif\n}\n"
            )
            for guarded in (False, True):
                executable = source / ("guarded" if guarded else "real")
                self.run_command(["cc", "-std=c11", *(["-DFIXTURE_GUARDS"] if guarded else []),
                                  str(main), str(archive), "-o", str(executable)])
                self.run_command([str(executable)])
                result = subprocess.run([str(executable), "crash"], capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 77 if guarded else 0, result.stderr)

    def check_fixture(self, baseline_host, msvc=True):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            files = {
                "main.c": "int rust_entry(void); int bmalloc(void); int main(void) { return rust_entry() != "
                + ("17 || bmalloc() != 7" if msvc else "110 || bmalloc() != 100") + "; }\n",
                "rust.c": "int private_adapter(void); int error_data_add(void); int rust_entry(void) { return error_data_add() + private_adapter(); }\n",
                "allocator.c": "int bmalloc(void) { return 7; } int error_data_add(void) { return bmalloc(); }\n",
                "import.c": "int bmalloc(void) { return 100; } int error_data_add(void) { return bmalloc(); }\n",
                "adapter.c": "int host_value(void); int private_adapter(void) { return host_value(); }\n",
                "unused.c": "int unavailable(void); int unused_adapter(void) { return unavailable(); }\n",
                "host.c": "int host_value(void) { return 10; }\n",
            }
            for name, content in files.items():
                (source / name).write_text(content)
            (source / "CMakeLists.txt").write_text(
                "cmake_minimum_required(VERSION 3.28)\nproject(rust_support C)\n"
                # Model the MSVC import boundary with host-linkable providers.
                + f"set(MSVC {'TRUE' if msvc else 'FALSE'})\n"
                +
                "add_library(host SHARED host.c)\n"
                "add_library(host_owner INTERFACE)\n"
                'target_link_libraries(host_owner INTERFACE host "$<$<TARGET_EXISTS:Optional::Host>:Optional::Host>")\n'
                "add_library(libobs_rust-static STATIC rust.c allocator.c)\n"
                "add_library(imported_obs SHARED import.c)\n"
                "add_library(OBS::libobs ALIAS imported_obs)\n"
                "add_library(support STATIC adapter.c unused.c)\n"
                + ("target_link_libraries(support PRIVATE host_owner)\n" if baseline_host else "")
                +
                "add_library(indirect INTERFACE)\n"
                "target_link_libraries(indirect INTERFACE libobs_rust-static)\n"
                "foreach(name IN ITEMS direct indirect_consumer conditional_consumer)\n"
                "  add_executable(${name} main.c)\n"
                "  target_link_libraries(${name} PRIVATE OBS::libobs)\n"
                "endforeach()\n"
                "target_link_libraries(direct PRIVATE libobs_rust-static)\n"
                "target_link_libraries(indirect_consumer PRIVATE indirect)\n"
                'target_link_libraries(conditional_consumer PRIVATE "$<$<BOOL:ON>:indirect>")\n'
                "file(WRITE ${CMAKE_CURRENT_BINARY_DIR}/reference.c \"int main(void) { return 0; }\\n\")\n"
                "add_executable(reference ${CMAKE_CURRENT_BINARY_DIR}/reference.c)\n"
                'target_link_libraries(reference PRIVATE "$<$<BOOL:OFF>:libobs_rust-static>")\n'
                f'include("{HELPER.as_posix()}" OPTIONAL)\n'
                "if(COMMAND obs_rust_static_consumers)\n"
                "  obs_rust_static_consumers(support host_owner)\n"
                "else()\n"
                # Baseline models the old per-target archive with no host interface.
                "  target_link_libraries(direct PRIVATE support)\n"
                "  target_link_libraries(indirect_consumer PRIVATE support)\n"
                "endif()\n"
                'file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/reference-links.txt" CONTENT "$<TARGET_GENEX_EVAL:reference,$<TARGET_PROPERTY:reference,LINK_LIBRARIES>>")\n'
                'foreach(name IN ITEMS direct indirect_consumer conditional_consumer)\n'
                '  file(GENERATE OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/${name}-links.txt" CONTENT "$<TARGET_GENEX_EVAL:${name},$<TARGET_PROPERTY:${name},LINK_LIBRARIES>>")\n'
                'endforeach()\n'
            )
            build = source / "build"
            self.run_command(["cmake", "-S", str(source), "-B", str(build)])
            reference_links = (build / "reference-links.txt").read_text()
            self.assertNotIn("libobs_rust-static", reference_links)
            self.assertNotIn("support", reference_links)
            targets = ("direct",) if baseline_host else ("direct", "indirect_consumer", "conditional_consumer")
            self.run_command(["cmake", "--build", str(build), "--parallel", "2", "--target", *targets])
            for name in targets:
                self.run_command([str(build / name)])
                if msvc:
                    links = (build / f"{name}-links.txt").read_text().split(";")
                    self.assertEqual(links[:2], ["libobs_rust-static", "support"])

    def run_command(self, command):
        result = subprocess.run(command, capture_output=True, text=True, check=False, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
