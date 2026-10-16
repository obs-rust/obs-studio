#!/usr/bin/env -S uv run --script
"""Which CI jobs a change needs (#436): Linux always; core code adds the
Rust-libobs OBS build; macOS or Windows code adds that platform's jobs.
Platform is decided by path alone (#897): platform code lives in
platform-named files and directories, and core files carry no platform
defines (ci/platform_ifdef_ratchet.py).

    uv run python -m unittest ci/test_ci_scope.py
"""

from __future__ import annotations

import unittest

from ci_scope import scope
from platform_paths import LINUX, MACOS, WINDOWS, platforms

CPP_TESTS = {"test/cmocka/test_source_select_list.cpp"}


class CiScopeTest(unittest.TestCase):
    def check(self, files, *, core=False, macos=False, windows=False):
        got = scope(files, lambda path: path in CPP_TESTS)
        self.assertEqual((got.core, got.macos, got.windows), (core, macos, windows), files)

    def test_docs_and_linux_only_need_linux(self):
        self.check(["docs/rust-port/naming.md"])
        self.check(["plugins/linux-pipewire/pipewire.c"])
        self.check(["libobs/obs-nix-wayland.c"], core=True)
        self.check(["libobs/obs-nix.c"], core=True)
        self.check(["frontend/widgets/OBSBasicWindow.cpp"])  # "Window", not Windows

    def test_core_code(self):
        self.check(["libobs/obs-source.c"], core=True)
        self.check(["rust/obs-util/src/dstr.rs"], core=True)
        self.check(["Cargo.lock"], core=True)

    def test_core_file_content_is_not_read(self):
        # Core files carry no platform defines (the ratchet lint enforces
        # it), so a core file is Linux-only whatever it contains (#897).
        self.check(["libobs/obs.c"], core=True)
        self.check(["frontend/OBSApp.cpp"])
        self.check(["plugins/obs-ffmpeg/CMakeLists.txt"])

    def test_macos_code(self):
        self.check(["plugins/mac-capture/mac-display-capture.m"], macos=True)
        self.check(["frontend/utility/CrashHandler_MacOS.mm"])  # Qt area: follow-up, no OS Rust/Tier 2
        self.check(["libobs/util/platform-cocoa.m"], core=True, macos=True)
        self.check(["libobs/util/apple/cfstring-utils.h"], core=True, macos=True)
        self.check(["cmake/macos/compilerconfig.cmake"], macos=True)
        self.check(["plugins/coreaudio-encoder/encoder.cpp"], macos=True)
        self.check(["libobs/audio-monitoring/osx/coreaudio-output.c"], core=True, macos=True)

    def test_posix_code_reaches_macos(self):
        # macOS builds the POSIX sources and util/platform-nix.c too
        # (libobs/cmake/os-macos.cmake).
        self.check(["libobs/util/threading-posix.c"], core=True, macos=True)
        self.check(["libobs/util/pipe-posix.c"], core=True, macos=True)
        self.check(["libobs/util/platform-nix.c"], core=True, macos=True)
        self.check(["shared/ipc-util/ipc-util/pipe-posix.c"], macos=True)
        self.check(["plugins/text-freetype2/find-font-unix.c"], macos=True)
        self.check(["rust/obs-util/src/platform/unix/pipe.rs"], core=True, macos=True)
        # The rest of the nix family is Linux and BSD only.
        self.check(["libobs/util/platform-nix-dbus.c"], core=True)

    def test_windows_code(self):
        self.check(["plugins/win-capture/graphics-hook/d3d11-capture.cpp"], windows=True)
        self.check(["frontend/utility/CrashHandler_Windows.cpp"])  # Qt area: follow-up
        self.check(["libobs/util/threading-windows.h"], core=True, windows=True)
        self.check(["libobs/obs-win-crash-handler.c"], core=True, windows=True)
        self.check(["deps/w32-pthreads/pthread.c"], windows=True)
        self.check(["cmake/windows/compilerconfig.cmake"], windows=True)
        self.check(["rust/obs-util/src/platform/windows/pipe.rs"], core=True, windows=True)

    def test_rust_platform_directories_map_exactly(self):
        self.check(["rust/obs-nvenc/src/platform/macos.rs"], core=True, macos=True)
        self.check(["rust/obs-nvenc/src/platform/windows.rs"], core=True, windows=True)
        self.check(["rust/obs-media-io/src/platform/unix.rs"], core=True, macos=True)
        self.check(["rust/obs-util/src/platform/linux/x.rs"], core=True)

    def test_shared_build_and_ci_changes_need_every_platform(self):
        for path in [
            "CMakeLists.txt",
            "cmake/common/helpers_common.cmake",
            "buildspec.json",
            "libobs/cmake/rust-exports.map",
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
            got = scope([path], lambda p: p in CPP_TESTS)
            self.assertTrue(got.macos and got.windows, path)

    def test_c_cmocka_tests_stay_linux_only(self):
        self.check(["test/cmocka/test_pipe_wait_error.c"])
        self.check(["test/cmocka/tests.d/test_pipe_wait_error.cmake"])


class QtScopeTest(unittest.TestCase):
    FRAGMENTS = {
        "test/cmocka/tests.d/test_source_select_list.cmake": "find_package(Qt6 QUIET COMPONENTS Core)",
        "test/cmocka/tests.d/test_pipe_wait_error.cmake": "add_executable(test_pipe_wait_error ...)",
    }

    def qt(self, *files):
        return scope(list(files), lambda p: False, lambda p: self.FRAGMENTS.get(p, "")).qt

    def test_frontend_and_qt_tests_run_the_job(self):
        self.assertTrue(self.qt("frontend/OBSApp.cpp"))
        self.assertTrue(self.qt("shared/qt/wrappers/qt-wrappers.cpp"))
        self.assertTrue(self.qt("test/cmocka/test_source_select_list.cpp"))
        self.assertTrue(self.qt("test/cmocka/tests.d/test_source_select_list.cmake"))
        self.assertTrue(self.qt("test/cmocka/CMakeLists.txt"))
        self.assertFalse(self.qt(".github/workflows/build-project.yaml"))  # only the Qt job's own files
        self.assertTrue(self.qt("ci/linux_qt_frontend.sh"))

    def test_other_changes_skip_the_job(self):
        self.assertFalse(self.qt("docs/rust-port/naming.md"))
        self.assertFalse(self.qt("libobs/obs-source.c", "rust/obs-util/src/dstr.rs"))
        self.assertFalse(self.qt("test/cmocka/test_pipe_wait_error.c"))
        self.assertFalse(self.qt("test/cmocka/tests.d/test_pipe_wait_error.cmake"))


class PlatformPathsTest(unittest.TestCase):
    def test_classification(self):
        self.assertIsNone(platforms("libobs/obs.c"))
        self.assertIsNone(platforms("frontend/utility/platform.hpp"))  # the facade
        self.assertEqual(platforms("frontend/utility/platform-x11.cpp"), {LINUX})
        self.assertEqual(platforms("frontend/utility/platform-osx.mm"), {MACOS})
        self.assertEqual(platforms("frontend/utility/platform-windows.cpp"), {WINDOWS})
        self.assertEqual(platforms("libobs/util/threading-posix.c"), {LINUX, MACOS})
        self.assertEqual(platforms("libobs/util/platform-nix.c"), {LINUX, MACOS})
        self.assertEqual(platforms("plugins/linux-v4l2/v4l2-input.c"), {LINUX})


LIBOBS_RUST = """[package]
name = "libobs-rust"
[dependencies]
obs-util = { path = "../obs-util" }
obs-data = { path = "../obs-data" }
[dev-dependencies]
obs-c-oracle = { path = "../obs-c-oracle" }
"""
CARGO = {
    "rust/libobs-rust/Cargo.toml": LIBOBS_RUST,
    "rust/obs-data/Cargo.toml": '[dependencies]\nobs-util = { path = "../obs-util" }\n',
    "rust/obs-util/Cargo.toml": "[dependencies]\n",
}
SUBS = frozenset({"plugins/obs-websocket", "plugins/obs-browser", "deps/libdshowcapture/src"})


class OsLaneTest(unittest.TestCase):
    """The one platform rule and the OS lanes it starts (#992)."""

    def os(self, path):
        from ci_scope import os_lanes

        return set(os_lanes(path))

    def test_platform_rule_per_os(self):
        self.assertEqual(self.os("rust/obs-util/src/platform/windows/mod.rs"), {"windows"})
        self.assertEqual(self.os("rust/obs-util/src/platform/macos/mod.rs"), {"macos"})
        self.assertEqual(self.os("rust/obs-util/src/platform/linux/mod.rs"), {"linux"})
        self.assertEqual(self.os("rust/obs-util/src/platform/posix/mod.rs"), {"linux", "macos"})
        self.assertEqual(self.os("plugins/text-freetype2/text-freetype2-platform-windows.c"), {"windows"})
        self.assertEqual(self.os("shared/ipc-util/platform/posix/pipe.c"), {"linux", "macos"})

    def test_core_is_linux_only(self):
        for f in ("libobs/obs-source.c", "rust/obs-util/src/dstr.rs", "frontend/OBSApp.cpp", "shared/x/platform.c"):
            self.assertEqual(self.os(f), {"linux"}, f)

    def test_legacy_names_still_fall_back(self):
        self.assertEqual(self.os("plugins/win-capture/a.c"), {"windows"})
        self.assertEqual(self.os("libobs/obs-cocoa.m"), {"macos"})

    def test_outside_every_area_starts_no_os(self):
        self.assertEqual(self.os("docs/platform-windows.md"), set())


class LaneTableTest(unittest.TestCase):
    """One test per LANES row: adding a module is adding a row (#992)."""

    def sc(self, *files):
        return scope(list(files), read=lambda p: CARGO.get(p, ""), submodules=SUBS)

    def test_linked_crates_follow_non_dev_dependencies(self):
        from ci_scope import linked_crates

        self.assertEqual(linked_crates(lambda p: CARGO.get(p, "")), {"libobs-rust", "obs-util", "obs-data"})

    def test_tier2_rows(self):
        for f in (
            "libobs/obs-source.c",
            "test/cmocka/test_x.c",
            "deps/jansson/x.c",
            "rust/tools/linux-validate/validate.sh",
            "Cargo.lock",
            "rust/obs-data/src/lib.rs",  # linked into libobs
            "rust/libobs-rust/src/lib.rs",
        ):
            self.assertTrue(self.sc(f).tier2, f)

    def test_tier2_skips_unrelated_paths(self):
        for f in (
            "docs/ci-tooling.md",
            "frontend/OBSApp.cpp",
            "plugins/obs-filters/expander-filter.c",
            "ci/ci_scope.py",
            ".github/workflows/build-project.yaml",
            "rust/obs-gpu-status/src/lib.rs",  # not linked into libobs
            "rust/obs-c-oracle/src/lib.rs",  # dev-dependency only
        ):
            self.assertFalse(self.sc(f).tier2, f)

    def test_no_pr_path_escalates_to_full(self):
        for f in ("build-aux/appimage/build.sh", ".gitmodules", "plugins/obs-websocket", "plugins/obs-browser/x.cpp", ".github/workflows/build-project.yaml"):
            self.assertFalse(self.sc(f).full, f)

    def test_workflow_edits_start_no_platform_job(self):
        got = self.sc(".github/workflows/build-project.yaml")
        self.assertFalse(got.macos or got.windows or got.qt or got.tier2)

    def test_platform_only_libobs_file_skips_linux_tier2(self):
        self.assertFalse(self.sc("libobs/util/platform-windows.c").tier2)
        self.assertFalse(self.sc("libobs/util/platform-cocoa.m").tier2)
        self.assertTrue(self.sc("libobs/util/platform-nix.c").tier2)
        self.assertTrue(self.sc("libobs/util/threading-posix.c").tier2)

    def test_linux_targets_are_the_touched_modules(self):
        self.assertEqual(self.sc("plugins/obs-filters/expander-filter.c").linux_targets, "obs-filters")
        self.assertEqual(
            self.sc("plugins/obs-filters/a.c", "shared/happy-eyeballs/b.c", "plugins/obs-filters/c.c").linux_targets,
            "happy-eyeballs,obs-filters",
        )

    def test_linux_targets_skip_other_os_qt_and_non_modules(self):
        for f in ("plugins/win-capture/a.c", "plugins/mac-capture/a.m", "plugins/frontend-tools/a.cpp", "shared/qt/a.cpp", "plugins/CMakeLists.txt", "libobs/a.c", "docs/a.md"):
            self.assertEqual(self.sc(f).linux_targets, "", f)

    def test_qt_row_is_frontend_only(self):
        self.assertTrue(self.sc("frontend/OBSApp.cpp").qt)
        self.assertFalse(self.sc("libobs/obs-source.c").qt)
        self.assertFalse(self.sc("rust/obs-util/src/a.rs").qt)

    def test_docs_start_nothing(self):
        got = self.sc("docs/rust-port/naming.md")
        self.assertEqual(vars(got), vars(scope([])))


if __name__ == "__main__":
    unittest.main()
