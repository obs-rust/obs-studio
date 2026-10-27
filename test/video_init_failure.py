#!/usr/bin/env -S uv run --script
"""Regression for #633: a failed EGL/video initialization leaks nothing (#962).

    uv run test/video_init_failure.py /absolute/path/to/obs /absolute/output/dir

Run an ASan/LSan frontend build without an EGL vendor. Requires `Xvfb` and
`xdotool`. Logs and the isolated profile remain in the output directory.

Ported off the 60-line shell script `video-init-failure.sh`. It is a standalone
developer harness with no caller in the tree, so nothing else changes when it
moves. The shell's steps are kept in order and in spirit:

    coproc Xvfb -displayfd 1 ...        # a private display, read once ready
    "$app" --multi --disable-shutdown-check --disable-updater --verbose
    timeout 90 xdotool search ... '^Error$' | head -1
    xdotool keydown --window "$window" Return
    wait "$app"                          # 0 and 255 are both acceptable
    grep -q 'Failed to initialize video' "$out/app.log"
    grep -qE 'eglInitialize.*failed|eglGet.*Display.*failed|...' "$out/app.log"
    grep -q 'Number of memory leaks: 0' "$out/app.log"
    grep -lE 'OBSBasic::OBSBasic|os_cpu_usage_info_start' "$out"/asan.*
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from collections.abc import Sequence
from pathlib import Path

WINDOW_TITLE = "^Error$"
SEARCH_TIMEOUT = 90  # seconds; the shell wrapped xdotool in `timeout 90`

APP_FLAGS = [
    "--multi",
    "--disable-shutdown-check",
    "--disable-updater",
    "--verbose",
]

FAILED_VIDEO = "Failed to initialize video"
EGL_FAILURE = re.compile(
    r"eglInitialize.*failed|eglGet.*Display.*failed|Failed to get EGL display"
)
NO_LEAKS = "Number of memory leaks: 0"
LSAN_LEAK = re.compile(r"OBSBasic::OBSBasic|os_cpu_usage_info_start")

PASS_LINE = "PASS: failed EGL initialization exits with zero OBS allocations"


def resolved(path: str) -> Path:
    """`realpath`/`realpath -m`: canonicalize without requiring the path."""
    return Path(os.path.realpath(path))


def prepare(out: Path) -> None:
    """`mkdir -p "$out/profile" "$out/runtime"` plus the runtime's mode 700."""
    (out / "profile").mkdir(parents=True, exist_ok=True)
    runtime = out / "runtime"
    runtime.mkdir(parents=True, exist_ok=True)
    runtime.chmod(0o700)


def environment(app: Path, out: Path, display: str) -> dict[str, str]:
    """The environment the shell exported before launching the app."""
    env = dict(os.environ)
    env["DISPLAY"] = display
    env["QT_QPA_PLATFORM"] = "xcb"
    env["XDG_CONFIG_HOME"] = str(out / "profile")
    env["XDG_RUNTIME_DIR"] = str(out / "runtime")
    env["__EGL_VENDOR_LIBRARY_FILENAMES"] = str(out / "no-egl-vendor.json")
    # $(dirname "$app")/../lib is not canonicalized by the shell, so keep it.
    lib = app.parent / ".." / "lib"
    existing = env.get("LD_LIBRARY_PATH")
    env["LD_LIBRARY_PATH"] = f"{lib}{':' + existing if existing else ''}"
    env["ASAN_OPTIONS"] = f"detect_leaks=1:log_path={out}/asan"
    env["LSAN_OPTIONS"] = "report_objects=1:exitcode=0"
    return env


def start_xvfb(out: Path) -> tuple[subprocess.Popen[bytes], str]:
    """Start a private Xvfb and read the display number it reports."""
    log = (out / "xvfb.log").open("wb")
    proc = subprocess.Popen(
        ["Xvfb", "-displayfd", "1", "-screen", "0", "1280x720x24"],
        stdout=subprocess.PIPE,
        stderr=log,
    )
    assert proc.stdout is not None
    display_number = proc.stdout.readline().decode().strip()
    return proc, display_number


def launch(app: Path, env: dict[str, str], app_log: Path) -> subprocess.Popen[bytes]:
    """Run the app from its own directory, its output going to `app.log`."""
    with app_log.open("wb") as sink:
        return subprocess.Popen(
            [str(app), *APP_FLAGS],
            cwd=app.parent,
            env=env,
            stdout=sink,
            stderr=subprocess.STDOUT,
        )


def search_window(env: dict[str, str]) -> str | None:
    """`timeout 90 xdotool search --sync --onlyvisible --name '^Error$' | head -1`."""
    try:
        result = subprocess.run(
            ["xdotool", "search", "--sync", "--onlyvisible", "--name", WINDOW_TITLE],
            env=env,
            timeout=SEARCH_TIMEOUT,
            capture_output=True,
            text=True,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return None
    lines = result.stdout.splitlines()
    return lines[0] if lines else None


def dismiss(env: dict[str, str]) -> bool:
    """Find the error dialog and press Return on it; False if it never appears."""
    window = search_window(env)
    if window is None:
        return False
    subprocess.run(
        ["xdotool", "keydown", "--window", window, "Return"],
        env=env,
        check=False,
    )
    return True


def dump(path: Path) -> None:
    """`cat "$out/app.log"` before a failure exit."""
    try:
        sys.stdout.write(path.read_text(errors="replace"))
    except OSError:
        pass


def leaks_found(out: Path) -> bool:
    """`grep -lE 'OBSBasic::OBSBasic|os_cpu_usage_info_start' "$out"/asan.*`."""
    return any(
        LSAN_LEAK.search(asan.read_text(errors="replace"))
        for asan in sorted(out.glob("asan.*"))
    )


def check_run(app_log: Path, out: Path) -> int:
    """The shell's three log assertions and the LSan scan; 0 when they hold."""
    text = app_log.read_text(errors="replace")
    if FAILED_VIDEO not in text or not EGL_FAILURE.search(text):
        return 1
    if NO_LEAKS not in text:
        dump(app_log)
        print(
            "FAIL: video initialization failure leaked an OBS allocation",
            file=sys.stderr,
        )
        return 1
    # The allocation check covers libobs ownership; LSan independently checks
    # that the CPU sampler and frontend window were released.
    if leaks_found(out):
        print("FAIL: LSan found a frontend initialization leak", file=sys.stderr)
        return 1
    return 0


def stop(children: Sequence[subprocess.Popen[bytes] | None]) -> None:
    """Kill what is still running (the shell's EXIT trap)."""
    for child in children:
        if child is None or child.poll() is not None:
            continue
        child.terminate()
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            child.kill()


def main(args: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if args is None else args)
    if not argv or not argv[0]:
        print("OBS executable required", file=sys.stderr)
        return 1
    if len(argv) < 2 or not argv[1]:
        print("output directory required", file=sys.stderr)
        return 1

    app = resolved(argv[0])
    out = resolved(argv[1])
    prepare(out)

    app_log = out / "app.log"
    xvfb: subprocess.Popen[bytes] | None = None
    proc: subprocess.Popen[bytes] | None = None
    try:
        xvfb, display_number = start_xvfb(out)
        env = environment(app, out, f":{display_number}")
        proc = launch(app, env, app_log)
        if not dismiss(env):
            return 1

        status = proc.wait()
        if status not in (0, 255):
            dump(app_log)
            return 1
        proc = None
        code = check_run(app_log, out)
        if code:
            return code
    finally:
        stop((proc, xvfb))

    print(PASS_LINE)
    return 0


if __name__ == "__main__":
    sys.exit(main())
