#!/usr/bin/env -S uv run --script
"""Exercise the standalone Rust-static support contract with real native links."""

from pathlib import Path
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

    def check_fixture(self, baseline_host):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            files = {
                "main.c": "int rust_entry(void); int bmalloc(void); int main(void) { return rust_entry() != 17 || bmalloc() != 7; }\n",
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

    def run_command(self, command):
        result = subprocess.run(command, capture_output=True, text=True, check=False, timeout=60)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
