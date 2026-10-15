#!/usr/bin/env -S uv run --script
"""Which CI jobs a change needs (#436): Linux always; core code adds the
Rust-libobs OBS build; macOS or Windows code adds that platform's jobs.

    uv run python -m unittest ci/test_ci_scope.py
"""

from __future__ import annotations

import unittest

from ci_scope import scope


class CiScopeTest(unittest.TestCase):
    def check(self, files, *, core=False, macos=False, windows=False):
        got = scope(files)
        self.assertEqual((got.core, got.macos, got.windows), (core, macos, windows), files)

    def test_docs_and_linux_only_need_linux(self):
        self.check(["docs/rust-port/naming.md"])
        self.check(["plugins/linux-pipewire/pipewire.c"])
        self.check(["libobs/obs-nix-wayland.c"], core=True)
        self.check(["frontend/widgets/OBSBasicWindow.cpp"])  # "Window", not Windows

    def test_core_code(self):
        self.check(["libobs/obs-source.c"], core=True)
        self.check(["rust/obs-util/src/dstr.rs"], core=True)
        self.check(["Cargo.lock"], core=True)

    def test_macos_code(self):
        self.check(["plugins/mac-capture/mac-display-capture.m"], macos=True)
        self.check(["frontend/utility/CrashHandler_MacOS.mm"], macos=True)
        self.check(["libobs/util/platform-cocoa.m"], core=True, macos=True)
        self.check(["libobs/util/apple/cfstring-utils.h"], core=True, macos=True)
        self.check(["cmake/macos/compilerconfig.cmake"], macos=True)
        self.check(["plugins/coreaudio-encoder/encoder.cpp"], macos=True)

    def test_windows_code(self):
        self.check(["plugins/win-capture/graphics-hook/d3d11-capture.cpp"], windows=True)
        self.check(["frontend/utility/CrashHandler_Windows.cpp"], windows=True)
        self.check(["libobs/util/threading-windows.h"], core=True, windows=True)
        self.check(["libobs/obs-win-crash-handler.c"], core=True, windows=True)
        self.check(["deps/w32-pthreads/pthread.c"], windows=True)
        self.check(["cmake/windows/compilerconfig.cmake"], windows=True)
        self.check(["rust/obs-util/src/platform/windows/pipe.rs"], core=True, windows=True)

    def test_unix_rust_platform_code_reaches_macos(self):
        self.check(["rust/obs-util/src/platform/unix/pipe.rs"], core=True, macos=True)

    def test_shared_build_and_ci_changes_need_every_platform(self):
        for path in [
            "CMakeLists.txt",
            "cmake/common/helpers_common.cmake",
            ".github/workflows/build-project.yaml",
            "buildspec.json",
            "rust/obs-util/src/platform/mod.rs",
        ]:
            got = scope([path])
            self.assertTrue(got.macos and got.windows, path)


    def test_cpp_cmocka_tests_and_test_build_config_need_every_platform(self):
        # Each platform's Tier 2 compiles them with its own compiler (#610).
        for path in [
            "test/cmocka/test_source_select_list.cpp",
            "test/cmocka/tests.d/test_source_select_list.cmake",
            "test/cmocka/CMakeLists.txt",
        ]:
            got = scope([path])
            self.assertTrue(got.macos and got.windows, path)

    def test_c_cmocka_tests_stay_linux_only(self):
        self.check(["test/cmocka/test_pipe_wait_error.c"])


if __name__ == "__main__":
    unittest.main()
