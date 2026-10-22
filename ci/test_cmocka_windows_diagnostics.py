#!/usr/bin/env -S uv run --script
"""cmocka tests built on Windows must not call fail_msg() (#1580, #1713).

    uv run python -m unittest ci/test_cmocka_windows_diagnostics.py

The Windows Tier 2 cmocka (vcpkg cmocka 2.0.2) does not export the private
cm_print_error() that fail_msg() expands to there, so a test that calls it
stops the build with LNK2019 before CTest runs: test_output_delay_e2e (#1580,
fixed in #1591) and test_video_output_plan (#1713). Those tests print with
fprintf(stderr, ...) and then call plain fail() instead.

The scan covers every test source named by add_executable() in
test/cmocka/CMakeLists.txt and test/cmocka/tests.d/*.cmake, unless an
enclosing if() keeps it off Windows (OS_LINUX, UNIX, NOT WIN32, a plugin
target that plugins/CMakeLists.txt only adds on other platforms, ...). An
else() or a condition it cannot read counts as built on Windows. Comments
that mention fail_msg() do not count. The second test compiles the actual
test_video_output_plan diagnostic helpers without a cm_print_error
definition, which reproduces the Windows unresolved-symbol link failure.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from test_settings_platform_facade import function  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
CMOCKA = ROOT / "test/cmocka"
FAIL_MSG = re.compile(r"\bfail_msg\s*\(")
COMMENT = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)
SOURCE = re.compile(r"[\w${}./-]+\.(?:c|cc|cpp|cxx|m|mm)\b")
# Platform words that are false on Windows.
NOT_WINDOWS = {"OS_LINUX", "OS_MACOS", "OS_FREEBSD", "OS_OPENBSD", "UNIX", "APPLE", "LINUX"}
NOT_ON_WINDOWS = {"NOT WIN32", "NOT OS_WINDOWS", "NOT MSVC"}


def non_windows_plugins() -> set[str]:
    """Plugin targets that plugins/CMakeLists.txt adds only off Windows."""
    text = (ROOT / "plugins/CMakeLists.txt").read_text()
    found = set()
    for name, platforms in re.findall(r"add_(?:core_)?module\((\S+)\s+PLATFORMS([^)]*)\)", text):
        if "WINDOWS" not in platforms.split():
            found.add(name.removeprefix("linux-").removeprefix("mac-"))
    return found


def excludes_windows(condition: str, plugins: set[str]) -> bool:
    """A condition that is false on Windows: one AND term that is, or ORs of only such words."""
    condition = " ".join(condition.split())
    for term in re.split(r"\s+AND\s+", condition.strip("() ")):
        term = term.strip("() ")
        if term in NOT_ON_WINDOWS:
            return True
        words = re.split(r"\s+OR\s+", term)
        if all(w.strip("() ") in NOT_WINDOWS for w in words):
            return True
        target = re.fullmatch(r"TARGET OBS::(\S+)", term)
        if target and target.group(1) in plugins:
            return True
    return False


def balanced(text: str, start: int) -> int:
    """The index after the ) that closes the ( at text[start]."""
    depth = 0
    for i in range(start, len(text)):
        depth += (text[i] == "(") - (text[i] == ")")
        if depth == 0:
            return i + 1
    raise ValueError("unbalanced parentheses")


def windows_sources(cmake: Path, plugins: set[str]) -> set[Path]:
    """Sources of the executables that cmake defines on Windows."""
    text = re.sub(r"#[^\n]*", "", cmake.read_text())
    stack: list[bool] = []
    found = set()
    for match in re.finditer(r"\b(if|elseif|else|endif|add_executable)\s*\(", text, re.I):
        word = match.group(1).lower()
        end = balanced(text, match.end() - 1)
        args = text[match.end() : end - 1]
        if word == "if":
            stack.append(excludes_windows(args, plugins))
        elif word in ("elseif", "else"):
            stack[-1] = False
        elif word == "endif":
            stack.pop()
        elif not any(stack):
            for name in SOURCE.findall(args):
                name = name.replace("${CMAKE_SOURCE_DIR}", str(ROOT))
                name = name.replace("${CMAKE_CURRENT_SOURCE_DIR}", str(CMOCKA))
                if "${" not in name:
                    found.add((CMOCKA / name).resolve())
    return found


def fail_msg_calls(path: Path) -> int:
    return len(FAIL_MSG.findall(COMMENT.sub("", path.read_text(errors="replace"))))


class CmockaWindowsDiagnosticsTest(unittest.TestCase):
    maxDiff = None

    def test_guard_reader(self):
        plugins = non_windows_plugins()
        self.assertIn("alsa", plugins)
        self.assertNotIn("media-playback", plugins)
        for condition in ("OS_LINUX", "OS_LINUX AND TARGET OBS::libobs-opengl", "UNIX AND TARGET OBS::libobs",
                          "TARGET OBS::outputs AND NOT WIN32", "OS_LINUX OR OS_FREEBSD", "TARGET OBS::alsa",
                          "(OS_LINUX OR OS_FREEBSD) AND ENABLE_PULSEAUDIO"):
            self.assertTrue(excludes_windows(condition, plugins), condition)
        for condition in ("TARGET Qt6::Widgets", "TARGET OBS::media-playback", "OS_LINUX OR OS_WINDOWS",
                          "NOT OS_LINUX", "OS_WINDOWS", "TARGET OBS::libobs"):
            self.assertFalse(excludes_windows(condition, plugins), condition)

    def test_windows_built_tests_do_not_call_fail_msg(self):
        plugins = non_windows_plugins()
        sources = set()
        for cmake in [CMOCKA / "CMakeLists.txt", *sorted((CMOCKA / "tests.d").glob("*.cmake"))]:
            sources |= windows_sources(cmake, plugins)
        names = {path.name for path in sources}
        # The scan sees the tests the Windows Tier 2 log builds and links ...
        for name in ("test_base.c", "test_output_delay_e2e.c", "test_video_output_plan.c",
                     "test_media_network_timeout.c"):
            self.assertIn(name, names)
        # ... and none that it never builds there.
        for name in ("test_video_output_formats.c", "test_alsa_capture_format.c",
                     "test_rtmp_send_thread_leak.c", "test_pipe_create2_exact_path.c"):
            self.assertNotIn(name, names)
        calls = {str(path.relative_to(ROOT)): n for path in sorted(sources)
                 if path.is_file() and (n := fail_msg_calls(path))}
        self.assertEqual(calls, {}, "fprintf(stderr, ...) and plain fail() instead of fail_msg() "
                         "(Windows cmocka does not export cm_print_error)")

    def test_video_output_plan_diagnostics_link_and_still_fail(self):
        compiler = shutil.which("cc")
        if not compiler:
            self.skipTest("C compiler unavailable; Windows Tier 2 remains required")
        text = (CMOCKA / "test_video_output_plan.c").read_text()
        internal = (ROOT / "libobs/obs-internal.h").read_text()
        helpers = "\n".join(function(text, signature) for signature in (
            "static bool same_float(", "static bool same_tech(", "static void check_plan(",
            "static void techniques_exist_in_the_effect(", "static void size_check_edges(",
            "static void output_size_check_edges("))
        structs = function(internal, "struct obs_video_plane {") + ";\n" + \
            function(internal, "struct obs_video_output_plan {") + ";\n"
        preamble = r'''
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define NUM_CHANNELS 3
#define N(a) (sizeof(a) / sizeof((a)[0]))
#define fail() do { exit(77); } while (0)
#define skip() do { exit(78); } while (0)
#define assert_true(value) do { if (!(value)) exit(79); } while (0)
#define assert_non_null(value) assert_true((value) != NULL)
#define assert_int_equal(a, b) assert_true((a) == (b))
/* Model the observed Windows link contract: a declaration is available,
 * but no cm_print_error definition is supplied to the linker. */
void cm_print_error(const char *format, ...);
#define fail_msg(format, ...) do { cm_print_error("ERROR: " format "\n", __VA_ARGS__); fail(); } while (0)
enum video_format { VIDEO_FORMAT_NONE, VIDEO_FORMAT_NV12 };
enum video_colorspace { VIDEO_CS_DEFAULT, VIDEO_CS_709 };
enum gs_color_format { GS_UNKNOWN, GS_R8, GS_R8G8, GS_BGRA };
enum gs_color_space { GS_CS_SRGB };
'''
        stubs = r'''
static const enum video_format all_formats[] = {VIDEO_FORMAT_NV12};
static const enum video_colorspace all_colorspaces[] = {VIDEO_CS_709};
static const char *mode = "valid";
static bool obs_video_size_valid(uint32_t w, uint32_t h) {
    if (strcmp(mode, "size") == 0) return true;
    return w >= 2 && w <= 32768 && h >= 2 && h <= 32768;
}
static bool obs_video_output_size_valid(uint32_t w, uint32_t h) {
    bool valid = obs_video_size_valid(w, h);
    return strcmp(mode, "output-size") == 0 ? valid : valid && w != 2 && w != 3;
}
static struct obs_video_output_plan obs_video_output_plan(enum video_format format, enum video_colorspace cs,
                                                          uint32_t width, uint32_t height) {
    (void)format; (void)cs;
    struct obs_video_output_plan plan;
    memset(&plan, 0, sizeof(plan));
    plan.conversion_needed = true;
    plan.conversion_techs[0] = strcmp(mode, "technique") == 0 ? "NV12_MISSING" : "NV12_Y";
    plan.conversion_techs[1] = "NV12_UV";
    plan.conversion_width_i = 1.0f / (float)width;
    plan.planes[0].width = width; plan.planes[0].height = height; plan.planes[0].format = GS_R8;
    plan.planes[1].width = width / 2; plan.planes[1].height = height / 2; plan.planes[1].format = GS_R8G8;
    plan.texture_format = GS_BGRA;
    return plan;
}
static struct obs_video_output_plan expected_plan(enum video_format format, enum video_colorspace cs,
                                                  uint32_t width, uint32_t height) {
    struct obs_video_output_plan plan = obs_video_output_plan(format, cs, width, height);
    if (strcmp(mode, "plan") == 0) plan.planes[1].height++;
    return plan;
}
'''
        main = r'''
int main(int argc, char **argv) {
    if (argc != 2) return 90;
    mode = argv[1];
    check_plan(VIDEO_FORMAT_NV12, VIDEO_CS_709, 64, 64);
    techniques_exist_in_the_effect(NULL);
    size_check_edges(NULL);
    output_size_check_edges(NULL);
    return 0;
}
'''
        with tempfile.TemporaryDirectory(prefix="obs-plan-diagnostic-") as tmp:
            source, binary, effect = Path(tmp) / "probe.c", Path(tmp) / "probe", Path(tmp) / "effect"
            effect.write_text("\ntechnique NV12_Y\n{\n}\n\ntechnique NV12_UV\n{\n}\n")
            define = '#define FORMAT_CONVERSION_EFFECT "%s"\n' % effect.as_posix()
            source.write_text(preamble + define + structs + stubs + helpers + main)
            result = subprocess.run([compiler, "-std=c11", "-Wall", "-Wextra", "-Werror", str(source),
                                     "-o", str(binary)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            for case, diagnostic in (
                ("valid", ""),
                ("plan", "plan differs for format 1, colorspace 1, 64x64"),
                ("technique", "technique NV12_MISSING is not in format_conversion.effect"),
                ("size", "size_valid(0, 0) is not 0"),
                ("output-size", "output_size_valid(2, 2) is not 0"),
            ):
                with self.subTest(case=case):
                    result = subprocess.run([str(binary), case], capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0 if case == "valid" else 77, result.stderr)
                    self.assertIn(diagnostic, result.stderr)


if __name__ == "__main__":
    unittest.main()
