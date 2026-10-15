#!/usr/bin/env -S uv run --script
"""Which CI jobs a change needs (#436): Linux always; core code adds the
Rust-libobs OBS build; macOS or Windows code adds that platform's jobs.

    uv run python -m unittest ci/test_ci_scope.py
"""

from __future__ import annotations

import unittest

from ci_scope import scope


def diff_for(path, hunk_header, *changed):
    """Real `git diff -U0` text for one hunk (lines start with + or -)."""
    return (
        f"diff --git a/{path} b/{path}\nindex 1111111..2222222 100644\n"
        f"--- a/{path}\n+++ b/{path}\n{hunk_header}\n" + "\n".join(changed) + "\n"
    )


NEW_TEST_DIFF = (
    "diff --git a/test/cmocka/tests.d/test_foo.cmake b/test/cmocka/tests.d/test_foo.cmake\n"
    "new file mode 100644\nindex 0000000..3333333\n--- /dev/null\n"
    "+++ b/test/cmocka/tests.d/test_foo.cmake\n@@ -0,0 +1,{n} @@\n{body}\n"
)


def new_test(*lines):
    body = "\n".join("+" + line for line in lines)
    return NEW_TEST_DIFF.format(n=len(lines), body=body), "\n".join(lines) + "\n"


SHARED_C = """#include "obs.h"

static int plain(void)
{
	return 1;
}

#ifdef __APPLE__
static int mac_only(void)
{
	return 2;
}
#else
static int not_mac(void)
{
	return 3;
}
#endif

#ifdef _WIN32
static int win_only(void)
{
	return 4;
}
#endif
"""


class CiScopeTest(unittest.TestCase):
    def check(self, files, *, core=False, macos=False, windows=False, diff="", text=None):
        got = scope(files, diff, lambda path: text)
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

    def test_platform_hunk_in_shared_c_file(self):
        path = "libobs/obs.c"
        # Added line inside `#ifdef __APPLE__` (line 10), no macro in the hunk.
        mac = diff_for(path, "@@ -9,0 +10 @@", "+\tint added = 0;")
        self.check([path], core=True, macos=True, diff=mac, text=SHARED_C)
        win = diff_for(path, "@@ -21,0 +22 @@", "+\tint added = 0;")
        self.check([path], core=True, windows=True, diff=win, text=SHARED_C)
        # `#else` branch of __APPLE__ is where Windows (not macOS) compiles.
        other = diff_for(path, "@@ -15,0 +16 @@", "+\tint added = 0;")
        self.check([path], core=True, windows=True, diff=other, text=SHARED_C)

    def test_branches_excluding_linux_reach_macos_and_windows(self):
        # `#else` of `#ifdef __linux__` and `#ifndef __linux__` are the code
        # macOS and Windows compile (#897).
        path = "frontend/components/ColorSourceToolbar.cpp"
        text = "int a;\n#ifdef __linux__\nint l;\n#else\nint o;\n#endif\n#ifndef __linux__\nint n;\n#endif\n"
        diff = diff_for(path, "@@ -4,0 +5 @@", "+int added;")
        self.check([path], macos=True, windows=True, diff=diff, text=text)
        diff = diff_for(path, "@@ -7,0 +8 @@", "+int added;")
        self.check([path], macos=True, windows=True, diff=diff, text=text)
        # Inside the Linux branch itself: Linux only.
        diff = diff_for(path, "@@ -2,0 +3 @@", "+int added;")
        self.check([path], diff=diff, text=text)
        # `#else` after both macOS and Windows branches: Linux only.
        text = "#ifdef _WIN32\nint w;\n#elif defined(__APPLE__)\nint m;\n#else\nint o;\n#endif\n"
        diff = diff_for(path, "@@ -5,0 +6 @@", "+int added;")
        self.check([path], diff=diff, text=text)

    def test_hunk_adding_a_linux_else_reaches_macos_and_windows(self):
        # The block opens inside the hunk, so no context line names it.
        path = "libobs/obs.c"
        added = ["#ifdef __linux__", "int l;", "#else", "int o;", "#endif"]
        text = "int a;\n" + "\n".join(added) + "\n"
        diff = diff_for(path, "@@ -1,0 +2,5 @@", *["+" + line for line in added])
        self.check([path], core=True, macos=True, windows=True, diff=diff, text=text)
        # The same hunk with only a Linux branch stays Linux only.
        added = ["#ifdef __linux__", "int l;", "#endif"]
        text = "int a;\n" + "\n".join(added) + "\n"
        diff = diff_for(path, "@@ -1,0 +2,3 @@", *["+" + line for line in added])
        self.check([path], core=True, diff=diff, text=text)

    def test_cmake_else_of_linux_reaches_macos_and_windows(self):
        path = "plugins/obs-ffmpeg/CMakeLists.txt"
        text = "if(OS_LINUX)\n  a()\nelse()\n  b()\nendif()\n"
        diff = diff_for(path, "@@ -3,0 +4 @@", "+  c()")
        self.check([path], macos=True, windows=True, diff=diff, text=text)

    def test_hunk_naming_a_platform_macro(self):
        path = "libobs/obs.c"
        diff = diff_for(path, "@@ -3,0 +4,3 @@ static int plain(void)", "+#ifdef _WIN32", "+\tx();", "+#endif")
        self.check([path], core=True, windows=True, diff=diff, text=SHARED_C)
        diff = diff_for(path, "@@ -3 +3 @@", "-\treturn 1;", "+\treturn TARGET_OS_OSX;")
        self.check([path], core=True, macos=True, diff=diff, text=SHARED_C)

    def test_plain_shared_c_edit_needs_only_linux(self):
        path = "libobs/obs.c"
        diff = diff_for(path, "@@ -5 +5 @@ static int plain(void)", "-\treturn 1;", "+\treturn 5;")
        self.check([path], core=True, diff=diff, text=SHARED_C)
        # After the closing #endif of the platform blocks nothing is open.
        diff = diff_for(path, "@@ -26,0 +27 @@", "+int tail;")
        self.check([path], core=True, diff=diff, text=SHARED_C + "int tail;\n")

    def test_platform_if_in_cmake(self):
        path = "plugins/obs-ffmpeg/CMakeLists.txt"
        text = "add_library(x)\nif(OS_MACOS)\n  target_link_libraries(x a)\nendif()\ninstall(x)\n"
        diff = diff_for(path, "@@ -2,0 +3 @@", "+  target_link_libraries(x b)")
        self.check([path], macos=True, diff=diff, text=text)
        diff = diff_for(path, "@@ -4,0 +5 @@", "+install(y)")
        self.check([path], diff=diff, text=text)

    def test_tests_d_registration_of_c_test_stays_linux_only(self):
        diff, text = new_test("add_cmocka_test(test_foo test_foo.c)")
        self.check(["test/cmocka/tests.d/test_foo.cmake"], diff=diff, text=text)

    def test_tests_d_with_platform_condition_reaches_that_platform(self):
        diff, text = new_test("if(OS_MACOS)", "  add_cmocka_test(test_foo test_foo.c)", "endif()")
        self.check(["test/cmocka/tests.d/test_foo.cmake"], macos=True, diff=diff, text=text)
        diff, text = new_test("if(WIN32)", "  add_cmocka_test(test_foo test_foo.c)", "endif()")
        self.check(["test/cmocka/tests.d/test_foo.cmake"], windows=True, diff=diff, text=text)

    def test_tests_d_registering_a_cpp_test_reaches_every_platform(self):
        diff, text = new_test("add_cmocka_test(test_foo test_foo.cpp)")
        self.check(["test/cmocka/tests.d/test_foo.cmake"], macos=True, windows=True, diff=diff, text=text)

    def test_rust_platform_directories_map_exactly(self):
        self.check(["rust/obs-nvenc/src/platform/macos.rs"], core=True, macos=True)
        self.check(["rust/obs-nvenc/src/platform/windows.rs"], core=True, windows=True)
        self.check(["rust/obs-media-io/src/platform/unix.rs"], core=True, macos=True)
        self.check(["rust/obs-util/src/platform/mod.rs"], core=True, macos=True, windows=True)


if __name__ == "__main__":
    unittest.main()
