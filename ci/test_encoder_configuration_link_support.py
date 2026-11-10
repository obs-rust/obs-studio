#!/usr/bin/env -S uv run --script
"""Keep the encoder adapter demand-linked in standalone Rust consumers."""

from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(shutil.which("cmake") and shutil.which("cc"), "requires CMake and a C compiler")
class EncoderConfigurationLinkSupport(unittest.TestCase):
    def test_adapter_is_selected_only_when_referenced(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            config = source / "build/config"
            config.mkdir(parents=True)
            (config / "obsconfig.h").write_text("#define ADAPTER_VALUE 42\n")
            (source / "encoder-configuration-link-support.c").write_text(
                '#include "obsconfig.h"\n'
                "extern int video_host(void);\n"
                "int encoder_adapter(void) { return video_host() + ADAPTER_VALUE - 42; }\n"
            )
            (source / "unused.c").write_text("int main(void) { return 0; }\n")
            (source / "used.c").write_text(
                "extern int encoder_adapter(void);\n"
                "int video_host(void) { return 42; }\n"
                "int main(void) { return encoder_adapter() != 42; }\n"
            )
            fragment = ROOT / "test/cmocka/tests.d/encoder_configuration_link_support.cmake"
            (source / "CMakeLists.txt").write_text(
                "cmake_minimum_required(VERSION 3.28)\nproject(adapter_selection C)\n"
                "add_library(libobs_rust-static INTERFACE)\n"
                "add_library(OBS::caption INTERFACE IMPORTED)\n"
                "add_library(Uthash::Uthash INTERFACE IMPORTED)\n"
                "foreach(name IN ITEMS unused used)\n"
                "  add_executable(${name} ${name}.c)\n"
                "  target_link_libraries(${name} PRIVATE libobs_rust-static)\n"
                "  target_include_directories(${name} PRIVATE ${CMAKE_BINARY_DIR}/config)\n"
                "endforeach()\n"
                f'include("{fragment.as_posix()}")\n'
            )
            for command in (
                ["cmake", "-S", str(source), "-B", str(source / "build")],
                ["cmake", "--build", str(source / "build")],
                [str(source / "build/unused")],
                [str(source / "build/used")],
            ):
                result = subprocess.run(command, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
