#!/usr/bin/env -S uv run --script
"""The MSVC build compiles at /W4 and does not hand /WX to the linker (#2626).

    uv run python -m unittest ci/test_msvc_warning_level.py

`/WX` is not a compile flag this file sets: `CMAKE_COMPILE_WARNING_AS_ERROR`
in `cmake/common/compiler_common.cmake` makes CMake pass it to `cl.exe`
(#516). The `add_link_options(/WX)` block was dead — `link.exe` ignores
`/WX` — so it is gone and only the compile-time setting carries the intent.
The warning level itself is `/W4`, the strictest level `cl.exe` has.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WINDOWS = ROOT / "cmake" / "windows" / "compilerconfig.cmake"
COMMON = ROOT / "cmake" / "common" / "compiler_common.cmake"


def call_tokens(text: str, name: str) -> list[str]:
    """Every whitespace-separated argument of each `name(...)` call."""
    calls = re.findall(rf"{name}\(([^)]*)\)", text, re.S)
    if not calls:
        raise AssertionError(f"no {name}( call")
    return [token for body in calls for token in body.split()]


class MsvcWarningLevelTest(unittest.TestCase):
    def setUp(self) -> None:
        self.text = WINDOWS.read_text(encoding="utf-8")

    def test_compile_options_request_w4(self) -> None:
        options = call_tokens(self.text, "add_compile_options")
        self.assertIn("/W4", options)
        self.assertNotIn("/W3", options)

    def test_no_w3_anywhere(self) -> None:
        self.assertIsNone(re.search(r"(?<![\w/])/W3\b", self.text), "/W3 left in the MSVC config")

    def test_link_options_do_not_pass_wx(self) -> None:
        self.assertNotIn("/WX", call_tokens(self.text, "add_link_options"))

    def test_wx_comes_from_the_compile_time_setting(self) -> None:
        common = COMMON.read_text(encoding="utf-8")
        self.assertIn("set(CMAKE_COMPILE_WARNING_AS_ERROR ON)", common)
        self.assertNotIn("/WX", call_tokens(self.text, "add_compile_options"))


HELPERS = ROOT / "cmake" / "common" / "helpers_common.cmake"

# Every product C++ target that compiles a translation unit including
# libobs/graphics/vec{2,3,4}.h, libobs/util/profiler.h or
# frontend/api/obs-frontend-api.h, and so reports C4201 at /W4. The warnings come
# from the headers, not from the target's own code, and the nameless unions in
# them are legal C11 (6.7.2.1p13) consumed from C, so #2626 decided a per-target
# exemption for exactly these targets rather than renaming the types (an API
# break) or disabling C4201 for every C++ target.
C4201_TARGETS = {
    "aja-output-ui": "plugins/aja-output-ui/CMakeLists.txt",
    "coreaudio-encoder": "plugins/coreaudio-encoder/CMakeLists.txt",
    "decklink": "plugins/decklink/CMakeLists.txt",
    "decklink-output-ui": "plugins/decklink-output-ui/CMakeLists.txt",
    "frontend-tools": "plugins/frontend-tools/CMakeLists.txt",
    "libobs-d3d11": "libobs-d3d11/CMakeLists.txt",
    "obs-studio": "frontend/CMakeLists.txt",
    "obs-browser": "plugins/CMakeLists.txt",
    "obs-vst": "plugins/obs-vst/CMakeLists.txt",
    "obs-websocket": "plugins/CMakeLists.txt",
    "win-dshow": "plugins/win-dshow/CMakeLists.txt",
}


class MsvcNamelessUnionExemptionTest(unittest.TestCase):
    """C4201 is exempted per target, for the C++ targets that include those headers."""

    def test_helper_is_cxx_and_msvc_scoped(self) -> None:
        text = HELPERS.read_text(encoding="utf-8")
        self.assertIn("function(obs_msvc_exempt_c4201 target)", text)
        helper = text.split("function(obs_msvc_exempt_c4201 target)", 1)[1].split("endfunction()", 1)[0]
        self.assertIn("/wd4201", helper)
        self.assertIn("COMPILE_LANG_AND_ID:CXX,MSVC", helper)

    def test_helper_is_a_no_op_when_the_module_is_disabled(self) -> None:
        """ENABLE_BROWSER=OFF / ENABLE_WEBSOCKET=OFF leave no target to exempt."""
        text = HELPERS.read_text(encoding="utf-8")
        helper = text.split("function(obs_msvc_exempt_c4201 target)", 1)[1].split("endfunction()", 1)[0]
        guard = helper.find("if(NOT TARGET ${target})")
        self.assertNotEqual(guard, -1, "the helper must tolerate a target the config did not create")
        self.assertLess(guard, helper.find("target_compile_options"), "return before touching the target")

    def test_no_blanket_exemption_in_the_msvc_config(self) -> None:
        windows = WINDOWS.read_text(encoding="utf-8")
        self.assertIsNone(
            re.search(r"(?<![\w/])/wd4201\b", windows),
            "the MSVC config must not disable C4201 for every C++ target",
        )

    def test_child_configured_targets_use_inline_flags(self) -> None:
        # The child architecture build configures win-capture/graphics-hook (and
        # friends) without cmake/common/helpers_common.cmake, so an
        # obs_msvc_exempt_c4201() call there fails configuration.
        hook = (ROOT / "plugins" / "win-capture" / "graphics-hook" / "CMakeLists.txt").read_text(encoding="utf-8")
        self.assertNotIn("obs_msvc_exempt_c4201", hook)
        self.assertIn("/wd4201", hook)
        child = (ROOT / "cmake" / "windows" / "architecture.cmake").read_text(encoding="utf-8")
        self.assertIn("add_subdirectory(plugins/win-capture/graphics-hook)", child)

    def test_generated_wrapper_and_padded_queue_exemptions(self) -> None:
        obslua = (ROOT / "shared" / "obs-scripting" / "obslua" / "CMakeLists.txt").read_text(encoding="utf-8")
        for flag in ("/wd4100", "/wd4189"):
            self.assertIn(flag, obslua, "the SWIG-generated wrapper is not edited by hand")
        decklink = (ROOT / "plugins" / "decklink" / "CMakeLists.txt").read_text(encoding="utf-8")
        self.assertIn("/wd4324", decklink, "FrameQueue pads for false-sharing avoidance")

    def test_each_product_cxx_target_opts_in(self) -> None:
        missing = []
        for target, relative in sorted(C4201_TARGETS.items()):
            body = (ROOT / relative).read_text(encoding="utf-8")
            if f"obs_msvc_exempt_c4201({target})" not in body:
                missing.append(f"{relative}: {target}")
        self.assertEqual(missing, [])


class MsvcDeadInternalFunctionExemptionTest(unittest.TestCase):
    """C4505 is exempted only for the obs-browser helper target (#2626)."""

    def helper(self) -> str:
        text = HELPERS.read_text(encoding="utf-8")
        self.assertIn("function(obs_msvc_exempt_c4505 target)", text)
        return text.split("function(obs_msvc_exempt_c4505 target)", 1)[1].split("endfunction()", 1)[0]

    def test_helper_is_cxx_and_msvc_scoped(self) -> None:
        helper = self.helper()
        self.assertIn("/wd4505", helper)
        self.assertIn("COMPILE_LANG_AND_ID:CXX,MSVC", helper)

    def test_helper_is_a_no_op_when_the_module_is_disabled(self) -> None:
        helper = self.helper()
        guard = helper.find("if(NOT TARGET ${target})")
        self.assertNotEqual(guard, -1, "the helper must tolerate a target the config did not create")
        self.assertLess(guard, helper.find("target_compile_options"), "return before touching the target")

    def test_only_the_obs_browser_helper_opts_in(self) -> None:
        # The dead static is `SetHighDPIv2Scaling()` in the obs-browser
        # submodule, so the exemption belongs to that target and nowhere else;
        # deleting the function is an obs-rust/obs-browser change.
        body = (ROOT / "plugins" / "CMakeLists.txt").read_text(encoding="utf-8")
        self.assertIn("obs_msvc_exempt_c4505(obs-browser-helper)", body)
        self.assertEqual(body.count("obs_msvc_exempt_c4505("), 1)
        windows = WINDOWS.read_text(encoding="utf-8")
        self.assertIsNone(
            re.search(r"(?<![\w/])/wd4505\b", windows),
            "the MSVC config must not disable C4505 for every C++ target",
        )


OUTPUTS = ROOT / "plugins" / "obs-outputs" / "CMakeLists.txt"
DSHOW = ROOT / "deps" / "libdshowcapture" / "CMakeLists.txt"


class MsvcVendoredDependencyExemptionTest(unittest.TestCase):
    """The /W4-only findings left in vendored code are suppressed, not edited (#516)."""

    def test_librtmp_exempts_its_w4_only_classes_on_msvc(self) -> None:
        text = OUTPUTS.read_text(encoding="utf-8")
        block = text.split("set_source_files_properties(", 1)[1].split("\n)", 1)[0]
        self.assertIn("librtmp/rtmp.c", block)
        self.assertRegex(
            block,
            r"COMPILE_LANG_AND_ID:C,MSVC>:/wd4244;/wd4245;/wd4456;/wd4702;/wd4706>",
            "librtmp needs C4244, C4245, C4456, C4702 and C4706 at /W4, scoped to the C language",
        )

    def test_libdshowcapture_exempts_its_shadowed_locals_on_msvc_cxx(self) -> None:
        text = DSHOW.read_text(encoding="utf-8")
        self.assertRegex(
            text,
            r"COMPILE_LANG_AND_ID:CXX,MSVC>:/wd4244;/wd4456;/wd4458>",
            "device.cpp:483 and encoder.cpp/output-filter.cpp/ElgatoUVCDevice.cpp "
            "are vendored; C4244/C4456/C4458 stay suppressed",
        )


if __name__ == "__main__":
    unittest.main()
