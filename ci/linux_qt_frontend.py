#!/usr/bin/env -S uv run --script
"""Fast-path Linux Qt frontend build (obs-rust/obs-studio#992).

Configures and links the OBS Qt frontend (the obs-studio target) against the
PREBUILT Qt prefix from obs-studio-libs (never compiles Qt), then builds and
runs the Qt-linked cmocka tests (tests.d fragments that find Qt6).

    uv run --no-project ci/linux_qt_frontend.py QT_PREFIX FFMPEG_PREFIX BUILD_DIR

Env: CCACHE_DIR (optional), CTEST_JOBS (default 4).
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import time
from pathlib import Path

SRC = Path(__file__).resolve().parent.parent
FRAGMENTS = SRC / "test" / "cmocka" / "tests.d"
USAGE = "usage: {name} QT_PREFIX FFMPEG_PREFIX BUILD_DIR"
# Test execution reads the build tree; keep it short and explicit.
ENABLE_FLAGS = [
    "-DENABLE_UNIT_TESTS=ON",
    "-DENABLE_RUST_LIBOBS=OFF",
    "-DENABLE_FRONTEND=ON",
    "-DENABLE_SCRIPTING=OFF",
    "-DENABLE_BROWSER=OFF",
    "-DENABLE_WEBSOCKET=OFF",
    "-DENABLE_AJA=OFF",
    "-DENABLE_WEBRTC=OFF",
    "-DENABLE_NVENC=OFF",
    "-DENABLE_QSV11=OFF",
    "-DENABLE_VST=OFF",
    "-DENABLE_WAYLAND=OFF",
    "-DENABLE_VLC=OFF",
    "-DENABLE_NEW_MPEGTS_OUTPUT=OFF",
    "-DENABLE_NATIVE_NVENC=OFF",
]
# The shared Qt facade, exercised against the same real prefix, offscreen.
# Supplying the prefix makes missing Qt a failure rather than a local skip.
# The command the real-Qt facade tests run with (ci/ on PYTHONPATH, #2311).
UNITTEST = "uv run python -m unittest"
REAL_QT_TESTS = [
    "ci.test_shared_qt_platform.SharedQtPlatformTest.test_real_qt_offscreen",
    "ci.test_obsapp_theme_platform.OBSAppThemePlatformTest.test_real_qt_signal_delivery",
    "ci.test_dialog_platform_facade.DialogPlatformFacadeTest.test_real_qt_display_notifications",
    "ci.test_vst_platform_facade.VstPlatformFacadeTest.test_real_qt_policy",
    "ci.test_widget_platform_facade.WidgetPlatformFacadeTest.test_live_x11_wayland_window_handles",
]
# The two forms `ninja -t targets all` gives a built executable: phony (from
# the cmocka test fragment) and the linker rule.
TARGET = re.compile(r"^([A-Za-z0-9_]*): phony$|^([A-Za-z0-9_]*): CXX_EXECUTABLE_LINKER")


def checked(argv: list[str], **kwargs) -> subprocess.CompletedProcess:
    """Run a command like `set -e` would: a failure keeps its exit code."""
    result = subprocess.run(argv, capture_output=True, text=True, **kwargs)
    if result.returncode != 0:
        sys.stdout.write(result.stdout)
        sys.stderr.write(result.stderr)
        sys.exit(result.returncode)
    return result


def logged(argv: list[str], log: Path) -> None:
    """Run a command with its output in both the log file and this console."""
    result = checked(argv)
    log.write_text(result.stdout, encoding="utf-8")
    sys.stdout.write(result.stdout)
    sys.stderr.write(result.stderr)


def qt_fragments() -> list[str]:
    """The cmocka fragments that find Qt6, as the shell's glob and grep gave them."""
    return sorted(
        path.stem for path in FRAGMENTS.glob("*.cmake") if "Qt6" in path.read_text(encoding="utf-8")
    )


def existing(tests: list[str], targets: set[str]) -> tuple[list[str], list[str]]:
    """(built, skipped): a fragment can skip its target when a Qt module is
    missing from the prebuilt archive (Qt6::Test is: test_hotkey_capture_mnemonic
    is not built, #992)."""
    built = [test for test in tests if test in targets]
    return built, [test for test in tests if test not in targets]


def ctest_regex(tests: list[str]) -> str:
    return "^(" + "|".join(tests) + ")$"


def phase(name: str, start: float) -> None:
    """Literal phase wall times distinguish compilation from test execution in
    overlap measurements (#1581)."""
    print(f"qt-phase phase={name} seconds={int(time.monotonic() - start)}", flush=True)


def run(qt: Path, ff: Path, build: Path) -> None:
    os.environ["PKG_CONFIG_PATH"] = f"{ff}/lib/pkgconfig" + (
        f":{os.environ['PKG_CONFIG_PATH']}" if os.environ.get("PKG_CONFIG_PATH") else ""
    )
    os.environ["LD_LIBRARY_PATH"] = f"{ff}/lib" + (
        f":{os.environ['LD_LIBRARY_PATH']}" if os.environ.get("LD_LIBRARY_PATH") else ""
    )

    start = time.monotonic()
    log = build / "build.configure.log"
    logged(
        ["cmake", "-S", str(SRC), "-B", str(build), "-G", "Ninja",
         "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
         f"-DCMAKE_PREFIX_PATH={qt};{ff}", f"-DQt6_DIR={qt}/lib/cmake/Qt6",
         "-DCMAKE_C_COMPILER_LAUNCHER=ccache", "-DCMAKE_CXX_COMPILER_LAUNCHER=ccache",
         *ENABLE_FLAGS],
        log,
    )
    phase("configure", start)

    # The distro Qt must not be what got linked (#992).
    configure = log.read_text(encoding="utf-8")
    for line in configure.splitlines():
        if re.search(r"^-- Found Qt6|Qt6_DIR", line):
            print(line)
    cache = build / "CMakeCache.txt"
    cache_text = cache.read_text(encoding="utf-8") if cache.exists() else ""
    if not any(line.startswith(f"Qt6Widgets_DIR:PATH={qt}/") for line in cache_text.splitlines()):
        print("Qt6Widgets_DIR is not in the prebuilt prefix:", file=sys.stderr)
        for line in cache_text.splitlines():
            if re.match(r"^Qt6.*_DIR", line):
                print(line, file=sys.stderr)
        sys.exit(1)

    for line in [l for l in configure.splitlines() if re.search(r"FFmpeg|avcodec", l, re.IGNORECASE)][:5]:
        print(line)
    prefix = checked(["pkg-config", "--variable=prefix", "libavcodec"]).stdout.strip()
    if prefix != str(ff):
        print(f"libavcodec is not from {ff} ({prefix})", file=sys.stderr)
        sys.exit(1)
    print(checked(["pkg-config", "--modversion", "libavcodec"]).stdout.strip())

    # Qt-linked tests: tests.d fragments that find Qt6.
    targets = checked(["ninja", "-C", str(build), "-t", "targets", "all"]).stdout.splitlines()
    have = {next(group for group in match.groups() if group) for match in map(TARGET.match, targets) if match}
    built, skipped = existing(qt_fragments(), have)
    for test in skipped:
        print(f"== skipped (no target, missing Qt module?): {test}")
    print(f"== Qt-linked tests: {' '.join(built)}")

    start = time.monotonic()
    checked(["ninja", "-C", str(build), "obs-studio", *built])
    phase("build", start)

    os.environ["LD_LIBRARY_PATH"] = f"{qt}/lib:{os.environ['LD_LIBRARY_PATH']}"
    os.environ["QT_PLUGIN_PATH"] = f"{qt}/plugins"
    # The tests import their ci/ siblings by name (#2311).
    checked([*UNITTEST.split(), *REAL_QT_TESTS],
            env=dict(os.environ, OBS_QT_TEST_PREFIX=str(qt), PYTHONPATH="ci"))

    start = time.monotonic()
    checked([
        "xvfb-run", "-a", "ctest", "--test-dir", str(build), "--output-on-failure",
        "-j", os.environ.get("CTEST_JOBS") or "4", "-R", ctest_regex(built),
    ])
    phase("test", start)


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        print(USAGE.format(name=Path(argv[0]).name), file=sys.stderr)
        return 2
    qt = Path(argv[1]).resolve()
    ff = Path(argv[2]).resolve()
    build = Path(argv[3])
    if not (qt / "lib" / "cmake" / "Qt6" / "Qt6Config.cmake").is_file():
        print(f"no Qt6 in {qt}", file=sys.stderr)
        return 1
    build.mkdir(parents=True, exist_ok=True)
    run(qt, ff, build)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
