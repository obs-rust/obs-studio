#!/usr/bin/env -S uv run --script
"""The Rust .deb installs next to the distribution's obs-studio (#2296):
on Linux its helpers take branded names, and every caller looks for the
name its build installed. Upstream builds and the native build keep the
upstream names (the source default).

    uv run --no-project ci/test_linux_helper_names.py
"""

from __future__ import annotations

import importlib.util
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HELPERS = {
    "OBS_FFMPEG_MUX_NAME": ("obs-ffmpeg-mux", "plugins/obs-ffmpeg/ffmpeg-mux/CMakeLists.txt"),
    "OBS_NVENC_TEST_NAME": ("obs-nvenc-test", "plugins/obs-nvenc/obs-nvenc-test/CMakeLists.txt"),
    "OBS_VST3_SCANNER_NAME": ("obs-vst3-scanner", "plugins/obs-vst3/vst3-scan/CMakeLists.txt"),
}
# Posix callers that start a helper, and the CMake file that builds them.
CALLERS = {
    "OBS_FFMPEG_MUX_NAME": [("plugins/obs-ffmpeg/obs-ffmpeg-mux-host-posix.c.h", "plugins/obs-ffmpeg/CMakeLists.txt")],
    "OBS_NVENC_TEST_NAME": [
        ("plugins/obs-nvenc/nvenc-helpers.c.h", "plugins/obs-nvenc/CMakeLists.txt"),
        ("frontend/utility/platform-x11.cpp.hpp", "frontend/cmake/os-linux.cmake"),
    ],
    "OBS_VST3_SCANNER_NAME": [
        ("plugins/obs-vst3/hosts/linux/vst3-host-linux.cpp.hpp", "plugins/obs-vst3/CMakeLists.txt"),
    ],
}


def text(rel: str) -> str:
    return (ROOT / rel).read_text()


class LinuxHelperNamesTest(unittest.TestCase):
    def test_rust_brand_names_the_helpers(self):
        defaults = text("cmake/linux/defaults.cmake")
        rust = defaults.split("if(OBS_BRAND_RUST)", 1)[1].split("endif()", 1)[0]
        for var, (upstream, _) in HELPERS.items():
            with self.subTest(var=var):
                self.assertIn(f"set({var} obs-studio-rust-{upstream.removeprefix('obs-')})", rust)

    def test_helpers_take_the_name(self):
        for var, (_, cmake) in HELPERS.items():
            with self.subTest(var=var):
                self.assertRegex(text(cmake), rf"OUTPUT_NAME\s+\"?\$\{{{var}\}}")

    def test_callers_use_the_name_with_the_upstream_default(self):
        for var, callers in CALLERS.items():
            upstream = HELPERS[var][0]
            for source, cmake in callers:
                with self.subTest(source=source):
                    body = text(source)
                    self.assertIn(f'#define {var} "{upstream}"', body)
                    self.assertNotRegex(body, rf'os_get_executable_path_ptr\("{upstream}"\)|return "{upstream}";')
                    self.assertRegex(text(cmake), rf'{var}="\$\{{{var}\}}"')

    def test_packaged_deb_installs_only_branded_commands(self):
        spec = importlib.util.spec_from_file_location("app_identity_check", ROOT / "ci/app_identity_check.py")
        check = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(check)
        check.check_deb_commands(["obs-studio-rust-gui", "obs-studio-rust-ffmpeg-mux", "obs-studio-rust-vst3-scanner"])
        with self.assertRaises(RuntimeError):
            check.check_deb_commands(["obs-studio-rust-gui", "obs-vst3-scanner"])


if __name__ == "__main__":
    unittest.main()
