#!/usr/bin/env -S uv run --script
"""The cross check can host a conversion-ratchet slice (#3150, #2625).

    uv run python -m unittest ci/test_cross_conversion_ratchet.py

A Windows- or macOS-only target is compiled by no Linux lane, so the per-plugin
opt-in `#2625` uses (`-Wconversion;-Wsign-conversion;-Wfloat-conversion` in the
target's own `CMakeLists.txt`) is never measured. The one compiler on a Linux
host that sees such a target is the cross check's clang, and it inherits the
global pins from `_obs_clang_*_options`, so the cross check needs its own opt-in
and its own pin: `ci/conversion_ratchet.py` reads `target_compile_options()` in
the plugin files, while the cross targets are declared in
`cmake/cross/CMakeLists.txt`.

The set of opted-in cross targets is read from that file, never a hand-kept
copy, so the PR that opts one in adds the marker and the pin together.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

CROSS = Path(__file__).resolve().parent.parent / "cmake" / "cross" / "CMakeLists.txt"
GROUPS = ("-Wconversion", "-Wsign-conversion", "-Wfloat-conversion")
HELPER = "_obs_cross_conversion_ratchet"
# The helper's body, up to the next top-level construct.
HELPER_BODY = re.compile(rf"^function\({HELPER}\b(?P<body>.*?)^endfunction\(\)$", re.M | re.S)
# Both helpers declare cross targets; `_obs_cross_frontend` wraps the plugin helper.
PLUGIN = re.compile(r"_obs_cross_(?:plugin|frontend)\(\s*(?P<target>[A-Za-z0-9_.+-]+)", re.S)
# A call of the helper at the start of a line, `_obs_cross_conversion_ratchet(<target>)`.
OPT_IN = re.compile(rf"^[ \t]*{HELPER}\(\s*(?P<target>[A-Za-z0-9_.+-]+)", re.M)


def source() -> str:
    return CROSS.read_text(encoding="utf-8")


def helper_body(text: str) -> str:
    match = HELPER_BODY.search(text)
    assert match, f"cmake/cross/CMakeLists.txt defines no {HELPER}()"
    return match.group("body")


def opted_in(text: str) -> set[str]:
    """The cross targets that call the helper."""
    return {match.group("target") for match in OPT_IN.finditer(text)}


class CrossConversionRatchet(unittest.TestCase):
    def test_dependency_includes_do_not_bypass_system_roots(self) -> None:
        root = CROSS.parents[2]
        tracked = subprocess.run(
            ["git", "ls-files", "libobs", "libobs-*", "frontend", "plugins", "shared"],
            cwd=root, capture_output=True, text=True, check=True,
        ).stdout.splitlines()
        offenders = []
        for name in tracked:
            path = root / name
            if path.suffix not in {".h", ".hpp", ".c", ".cpp", ".m", ".mm"}:
                continue
            if "/decklink-sdk/" in name:
                continue
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                if re.match(r'^\s*#\s*include\s*["<](?:deps/|\.deps/|plugins/decklink/(?:linux|mac|win)/decklink-sdk/)', line):
                    offenders.append(f"{name}:{number}")
        self.assertEqual(offenders, [], "use the dependency SYSTEM root, not the ordinary repo root")

    def test_dependency_headers_are_system_but_our_headers_are_not(self) -> None:
        """Exercise the native/cross policy without downloading any SDK."""
        root = CROSS.parents[2]
        policy = root / "cmake" / "third-party-headers.cmake"
        self.assertTrue(policy.is_file(), "missing third-party header policy")
        for entry in (root / "CMakeLists.txt", CROSS):
            self.assertIn("third-party-headers.cmake", entry.read_text(encoding="utf-8"))
        self.assertIsNotNone(shutil.which("cmake"))
        with tempfile.TemporaryDirectory() as tmp:
            fixture = Path(tmp)
            (fixture / "CMakeLists.txt").write_text(
                'cmake_minimum_required(VERSION 3.28)\n'
                'project(policy C)\n'
                'add_library(sample INTERFACE)\n'
                'target_include_directories(sample INTERFACE\n'
                ' "${CMAKE_SOURCE_DIR}/.deps/sdk/include"\n'
                ' "${CMAKE_SOURCE_DIR}/deps/simde"\n'
                ' "$<$<BOOL:1>:${CMAKE_SOURCE_DIR}/plugins/decklink/mac/decklink-sdk>"\n'
                ' "$<BUILD_INTERFACE:${CMAKE_SOURCE_DIR}/deps/json11>"\n'
                ' "${CMAKE_SOURCE_DIR}/libobs"\n'
                ' "${CMAKE_BINARY_DIR}/generated")\n'
                'file(WRITE "${CMAKE_BINARY_DIR}/sample.c" "#include <dependency.h>\\n")\n'
                'add_library(consumer OBJECT "${CMAKE_BINARY_DIR}/sample.c")\n'
                'target_include_directories(consumer PRIVATE "${CMAKE_SOURCE_DIR}/deps/simde")\n'
                'target_compile_options(consumer PRIVATE -Werror=conversion)\n'
                f'include("{policy.as_posix()}")\n'
                'cmake_language(DEFER CALL file GENERATE OUTPUT "${CMAKE_BINARY_DIR}/system.txt"\n'
                ' CONTENT "$<TARGET_PROPERTY:sample,INTERFACE_SYSTEM_INCLUDE_DIRECTORIES>")\n',
                encoding="utf-8",
            )
            vendor = fixture / "deps/simde"
            vendor.mkdir(parents=True)
            (vendor / "dependency.h").write_text(
                "static inline int dependency(double x) { return x; }\n", encoding="utf-8"
            )
            result = subprocess.run(
                ["cmake", "-S", str(fixture), "-B", str(fixture / "build")],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            system = (fixture / "build/system.txt").read_text(encoding="utf-8").split(";")
            self.assertEqual(set(system), {
                str(fixture / ".deps/sdk/include"), str(fixture / "deps/simde"),
                str(fixture / "deps/json11"),
                str(fixture / "plugins/decklink/mac/decklink-sdk"),
            })
            build = ["cmake", "--build", str(fixture / "build"), "--target", "consumer"]
            result = subprocess.run(build, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            ours = fixture / "libobs"
            ours.mkdir()
            (ours / "ours.h").write_text(
                "static inline int ours(double x) { return x; }\n", encoding="utf-8"
            )
            (fixture / "build/sample.c").write_text(
                f'#include "{(ours / "ours.h").as_posix()}"\n', encoding="utf-8"
            )
            result = subprocess.run(build, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0, "first-party conversion must fail")
            self.assertIn("ours.h", result.stdout + result.stderr)

    def test_the_helper_names_the_three_groups(self) -> None:
        body = helper_body(source())
        for group in GROUPS:
            self.assertIn(group, body)

    def test_the_helper_is_gated_on_clang(self) -> None:
        """MSVC has no such flags, so the alias must stay inert there."""
        body = helper_body(source())
        self.assertIn("COMPILE_LANG_AND_ID", body)
        self.assertIn("Clang", body)

    def test_the_helper_follows_the_cross_plugin_helper(self) -> None:
        """It appends to a target `_obs_cross_plugin()` already configured with
        the shared option lists, which carry the `-Wno-conversion` pins."""
        text = source()
        self.assertLess(
            text.index("function(_obs_cross_plugin"),
            text.index(f"function({HELPER}"),
            f"{HELPER}() belongs after _obs_cross_plugin()",
        )

    def test_every_opted_in_target_is_a_cross_target(self) -> None:
        text = source()
        declared = {match.group("target") for match in PLUGIN.finditer(text)}
        chosen = opted_in(text)
        self.assertTrue(chosen, f"no target calls {HELPER}()")
        self.assertLessEqual(chosen, declared, f"not declared by a cross target helper: {chosen - declared}")

    def test_the_wiring_proof_is_opted_in(self) -> None:
        """`obs-amf-test-cross` is the no-op slice: it carries no diagnostics, so
        its RED (before this marker) is "no group named", not a compiler error."""
        self.assertIn("obs-amf-test-cross", opted_in(source()))

    def test_the_measured_slices_stay_opted_in(self) -> None:
        """The slices already burned down stay opted in, so a later edit that
        drops the marker cannot quietly take them back out of the ratchet."""
        slices = {"coreaudio-encoder-cross", "obs-text-cross"}
        self.assertLessEqual(slices, opted_in(source()))

    def test_the_coreaudio_encoder_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep the Windows and macOS encoder groups measured (#2625)."""
        target = "coreaudio-encoder-cross"
        self.assertEqual(source().count(f"{HELPER}({target})"), 2)

    def test_the_nv_filters_slice_is_opted_in(self) -> None:
        """Keep all three NVIDIA filter unity groups measured (#2625)."""
        self.assertIn("nv-filters-cross", opted_in(source()))

    def test_the_nvenc_slice_is_opted_in(self) -> None:
        """Keep all four Windows NVENC unity groups measured (#2625)."""
        self.assertIn("obs-nvenc-cross", opted_in(source()))

    def test_the_win_capture_slice_is_opted_in(self) -> None:
        """Keep both Windows capture unity groups measured (#2625)."""
        self.assertIn("win-capture-cross", opted_in(source()))

    def test_the_win_dshow_slice_is_opted_in(self) -> None:
        """Keep all three Windows DirectShow unity groups measured (#2625)."""
        self.assertIn("win-dshow-cross", opted_in(source()))

    def test_the_virtualcam_module_slice_is_opted_in(self) -> None:
        """Keep both Windows virtual camera module unity groups measured (#2625)."""
        self.assertIn("virtualcam-module-cross", opted_in(source()))

    def test_the_graphics_hook_slice_is_opted_in(self) -> None:
        """Keep all nine injected Windows graphics hook groups measured (#2625)."""
        self.assertIn("graphics-hook-cross", opted_in(source()))

    def test_the_updater_slice_is_opted_in(self) -> None:
        """`updater-cross` is a real slice: its diagnostics are stated (#2625)."""
        self.assertIn("updater-cross", opted_in(source()))

    def test_the_qsv_probe_slice_is_opted_in(self) -> None:
        """Keep the Windows QSV capability probe measured (#2625)."""
        self.assertIn("obs-qsv-test-cross", opted_in(source()))

    def test_the_audio_plugin_slice_is_opted_in(self) -> None:
        """The virtual microphone HAL stays measured by the cross check (#2625)."""
        self.assertIn("obs-audio-plugin-cross", opted_in(source()))

    def test_the_mac_virtualcam_slice_is_opted_in(self) -> None:
        """Keep the virtual camera output measured on both macOS aliases (#2625)."""
        self.assertIn("mac-virtualcam-cross", opted_in(source()))

    def test_the_dal_plugin_slice_is_opted_in(self) -> None:
        """Keep the Objective-C++ DAL group measured on both macOS aliases (#2625)."""
        self.assertIn("obs-dal-plugin-cross", opted_in(source()))

    def test_the_mac_capture_slice_is_opted_in(self) -> None:
        """Keep all five capture groups measured on both macOS aliases (#2625)."""
        self.assertIn("mac-capture-cross", opted_in(source()))

    def test_the_helper_covers_objective_cxx(self) -> None:
        """DAL is Objective-C++; a CXX-only opt-in would leave its pins active."""
        self.assertIn("COMPILE_LANG_AND_ID:OBJCXX,Clang,AppleClang", helper_body(source()))

    def test_the_videotoolbox_slice_is_opted_in(self) -> None:
        """The macOS encoder stays measured by the cross check (#2625)."""
        self.assertIn("mac-videotoolbox-cross", opted_in(source()))

    def test_the_network_interface_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep both Windows and macOS network-interface groups measured (#2625)."""
        target = "obs-outputs-net-if-host-cross"
        self.assertIn(target, opted_in(source()))
        self.assertEqual(source().count(f"{HELPER}({target})"), 2)

    def test_the_rtmp_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep both Windows and POSIX RTMP host groups measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-outputs-rtmp-host-cross"), 2)


    def test_the_ffmpeg_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep the NVENC availability/module host groups measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-ffmpeg-host-cross"), 2)

    def test_the_ffmpeg_mux_helper_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep both Windows and POSIX mux-helper host groups measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-ffmpeg-mux-helper-host-cross"), 2)

    def test_the_frontend_tools_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep both frontend-tools host wrappers measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("frontend-tools-host-cross"), 2)

    def test_the_outputs_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep the Windows and POSIX output host groups measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-outputs-host-cross"), 2)

    def test_the_ffmpeg_mux_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep both mux-output host groups measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-ffmpeg-mux-host-cross"), 2)

    def test_the_python_import_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep both Python library-name host wrappers measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-scripting-python-import-host-cross"), 2)

    def test_the_lua_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep the Windows and macOS Lua module-directory hosts measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-scripting-lua-host-cross"), 2)

    def test_the_python_host_slice_is_opted_in_on_both_hosts(self) -> None:
        """Keep Python startup/home/module-directory host wrappers measured (#2625)."""
        targets = [match.group("target") for match in OPT_IN.finditer(source())]
        self.assertEqual(targets.count("obs-scripting-python-host-cross"), 2)

    def test_the_graphics_offsets_slice_is_opted_in(self) -> None:
        """Keep all Windows graphics-offset probe groups measured (#2625)."""
        self.assertIn("get-graphics-offsets-cross", opted_in(source()))

    def test_the_wasapi_slice_is_opted_in(self) -> None:
        """Keep the Windows audio capture unity group measured (#2625)."""
        self.assertIn("win-wasapi-cross", opted_in(source()))

    def test_the_qsv_encoder_slice_is_opted_in(self) -> None:
        """Keep all three Windows QSV encoder unity groups measured (#2625)."""
        self.assertIn("obs-qsv11-cross", opted_in(source()))

    def test_the_d3d11_slice_is_opted_in(self) -> None:
        """Keep the Windows Direct3D 11 unity group measured (#2625)."""
        self.assertIn("libobs-d3d11-cross", opted_in(source()))
    def test_the_amf_encoder_slice_is_opted_in(self) -> None:
        """Keep the Windows AMF texture encoders measured (#2625)."""
        self.assertIn("obs-ffmpeg-amf-cross", opted_in(source()))

    def test_the_frontend_macos_slice_is_opted_in(self) -> None:
        """Keep all macOS frontend unity groups measured on both aliases (#2625)."""
        self.assertIn("frontend-macos-cross", opted_in(source()))

    def test_the_frontend_windows_slice_is_opted_in(self) -> None:
        """Keep all six Windows frontend unity groups measured (#2625)."""
        self.assertIn("frontend-windows-cross", opted_in(source()))

    def test_the_shared_and_plugin_slices_stay_opted_in(self) -> None:
        """The win-x64 slices burned down after #3157 stay opted in, so a later
        edit that drops a marker cannot quietly take them back out of the
        ratchet: `shared-windows-cross` (the shared Windows helpers) and the two
        plugins the cross clang already compiles clean under the groups."""
        slices = {"shared-windows-cross", "obs-x264-cross", "rtmp-services-cross"}
        self.assertLessEqual(slices, opted_in(source()))


if __name__ == "__main__":
    unittest.main()
