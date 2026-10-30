#!/usr/bin/env -S uv run --script
"""Builds OBS for one sanitizer tool, runs the cmocka tests under it, then
launches OBS headless (Xvfb) and closes it with SIGINT after a timeout.

    uv run --no-project rust/tools/sanitize/sanitize.py asan|tsan|valgrind [launch-seconds]

Ubuntu: install deps with rust/tools/sanitize/install_ubuntu_deps.py first.
NixOS:  nix-shell rust/tools/sanitize/shell.nix --run "uv run --no-project rust/tools/sanitize/sanitize.py asan"
Summarize the results with rust/tools/sanitize/triage.py <out>/<tool>.

Environment:
  SAN_OUT      output root (default rust/tools/sanitize/out); results go to
               $SAN_OUT/<tool>, the build tree to $SAN_OUT/build-<tool>
  RUST_LIBOBS  ON (default) uses the hybrid; OFF explicitly selects C rollback.
  SKIP_TESTS   non-empty: only relaunch the app, keep earlier test results

Ported off the 178-line shell script in #962; sanitizers.yaml runs it through
`uv run --no-project`. Tested by ci/test_sanitize_harness.py.
"""

from __future__ import annotations

import json
import os
import random
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC = HERE.parent.parent.parent
# One test binary gets 10 minutes, the same bound the shell's `timeout 600` set.
TEST_TIMEOUT = 600
# tool -> (compile flags, CMAKE_BUILD_TYPE)
TOOL_FLAGS = {
    "asan": ("-fsanitize=address,undefined -fno-omit-frame-pointer", "RelWithDebInfo"),
    "tsan": ("-fsanitize=thread -fno-omit-frame-pointer", "RelWithDebInfo"),
    "valgrind": ("-fno-omit-frame-pointer", "Debug"),
}
# test_formatted_filename and test_utf8 hardcode -fsanitize=address,undefined
# (test/cmocka/CMakeLists.txt, obs-rust/obs-studio#159), which TSan rejects and
# valgrind can't run. They are covered by the asan pass, so the other tools
# build with -k 0 and skip them.
SKIPPED_TESTS = re.compile(r"^(test_formatted_filename|test_utf8)$")
VALGRIND = (
    "valgrind",
    "--error-exitcode=99",
    "--child-silent-after-fork=yes",
    "--errors-for-leak-kinds=definite",
    "--leak-check=full",
    "--show-leak-kinds=definite",
    "--num-callers=40",
    "--track-origins=yes",
    "--fullpath-after=",
)
# Full allocation stacks: distro FFmpeg has no frame pointers, and the fast
# unwinder stops at av_malloc (#2035).
SANITIZER_OPTIONS = {
    "asan": "detect_leaks=1:fast_unwind_on_malloc=0:abort_on_error=0:halt_on_error=0",
    "ubsan": "print_stacktrace=1:halt_on_error=0",
    "tsan": "halt_on_error=0:second_deadlock_stack=1",
}
OBS_ARGS = ("--disable-shutdown-check", "--disable-updater", "--verbose")


@dataclass(frozen=True)
class Test:
    """One registered ctest test, as `ctest --show-only=json-v1` reports it."""

    name: str
    workdir: str
    command: tuple[str, ...]


@dataclass(frozen=True)
class Layout:
    """Where one tool's run keeps its artifacts."""

    root: Path
    out: Path
    build: Path
    inst: Path
    bin_dir: Path

    @classmethod
    def for_tool(cls, tool: str, build_type: str) -> Layout:
        root = Path(os.environ.get("SAN_OUT") or HERE / "out")
        build = root / f"build-{tool}"
        return cls(
            root=root,
            out=root / tool,
            build=build,
            inst=build / "rundir" / build_type,
            bin_dir=root / "bin",
        )


def sanitizer_options(directory: Path, stem: str = "") -> dict[str, str]:
    """The three *_OPTIONS values with log_path=<directory>/<stem><kind>.

    The shell exported them for the whole run (log_path=<out>/<kind>) and
    overrode the log_path per test (log_path=<out>/tests/<name>.<kind>).
    """
    return {f"{kind.upper()}_OPTIONS": f"{value}:log_path={directory / f'{stem}{kind}'}" for kind, value in SANITIZER_OPTIONS.items()}


def configure_command(*, src: Path, build: Path, build_type: str, flags: str, backend: str, version: str) -> list[str]:
    """The `cmake` configure line for one tool's build."""
    command = [
        "cmake",
        "-S",
        str(src),
        "-B",
        str(build),
        "-G",
        "Ninja",
        f"-DCMAKE_BUILD_TYPE={build_type}",
        "-DENABLE_UNIT_TESTS=ON",
        f"-DOBS_LIBOBS_BACKEND={backend}",
        "-DOBS_BRAND_RUST=OFF",
        "-DCMAKE_C_COMPILER_LAUNCHER=zccache",
        "-DCMAKE_CXX_COMPILER_LAUNCHER=zccache",
        f"-DCMAKE_C_FLAGS={flags}",
        f"-DCMAKE_CXX_FLAGS={flags}",
        f"-DCMAKE_EXE_LINKER_FLAGS={flags} -Wl,--enable-new-dtags",
        f"-DCMAKE_SHARED_LINKER_FLAGS={flags} -Wl,--enable-new-dtags",
        f"-DCMAKE_MODULE_LINKER_FLAGS={flags} -Wl,--enable-new-dtags",
        "-DCMAKE_POSITION_INDEPENDENT_CODE=ON",
        "-DENABLE_BROWSER=OFF",
        "-DENABLE_WEBSOCKET=OFF",
        "-DENABLE_AJA=OFF",
        "-DENABLE_WEBRTC=OFF",
        "-DENABLE_NVENC=OFF",
        "-DENABLE_QSV11=OFF",
        "-DENABLE_VST=OFF",
        "-DENABLE_SCRIPTING=OFF",
        "-DENABLE_NEW_MPEGTS_OUTPUT=OFF",
        "-DENABLE_VLC=OFF",
    ]
    if version:
        command.append(f"-DOBS_VERSION_OVERRIDE={version}")
    return command


def ctest_tests(payload: str) -> list[Test]:
    """The registered tests in a `ctest --show-only=json-v1` payload, by name.

    Only tests with a command are runnable (the shell's `select(.command)`),
    so plugin modules named test_* are not run (#2045). A test's
    WORKING_DIRECTORY is where ctest runs it: libobs finds its data relative
    to it (#2043).
    """
    tests = []
    for entry in json.loads(payload).get("tests", []):
        command = entry.get("command")
        if not command:
            continue
        properties = entry.get("properties") or []
        workdir = next((str(p.get("value", "")) for p in properties if p.get("name") == "WORKING_DIRECTORY"), "")
        tests.append(Test(name=str(entry.get("name", "")), workdir=workdir, command=tuple(str(a) for a in command)))
    return sorted(tests, key=lambda test: test.name)


def test_command(test: Test) -> list[str]:
    """The test's own command line, or [] when the command cannot run it.

    `ctest --show-only` reports a wrapper (xvfb-run) in front of the binary;
    the display is ours, so the wrapper is dropped: the command starts at the
    first argument whose basename is the test's name.
    """
    command = list(test.command)
    while command and Path(command[0]).name != test.name:
        command = command[1:]
    if not command or not os.access(command[0], os.X_OK):
        return []
    return command


def is_skipped(tool: str, name: str) -> bool:
    """Only the asan pass builds the two hardcoded-ASan tests (#159)."""
    return tool != "asan" and bool(SKIPPED_TESTS.match(name))


def launch_timing(tool: str, secs: int) -> tuple[int, int]:
    """(seconds before SIGINT, seconds of grace before SIGKILL).

    Shutdown under valgrind is slow and valgrind runs the app slower, so its
    launch window is 4x the requested one (#462).
    """
    return (secs * 4, 240) if tool == "valgrind" else (secs, 60)


def launch_command(tool: str, binary: str, out: Path) -> list[str]:
    """OBS under the tool, or under valgrind writing <out>/app.vg.log."""
    command = [binary, *OBS_ARGS]
    if tool == "valgrind":
        return [*VALGRIND, f"--log-file={out / 'app.vg.log'}", *command]
    return command


def tail(path: Path, lines: int) -> str:
    return "\n".join(path.read_text(encoding="utf-8", errors="replace").splitlines()[-lines:])


def error_lines(path: Path, limit: int = 40) -> str:
    """The shell's `grep -E error <log> | head -40`."""
    matches = [line for line in path.read_text(encoding="utf-8", errors="replace").splitlines() if "error" in line]
    return "\n".join(matches[:limit])


def zccache_refused_uncached(path: Path) -> bool:
    """True when the build log shows zccache's daemon dead in its recovery
    cool-down, where it refuses to compile uncached (`code=125`).

    That is a cache-daemon failure, not a failure of the tree: the pass must
    rebuild with `ZCCACHE_DISABLE=1` instead of reporting a red sanitizer run.
    """
    text = path.read_text(encoding="utf-8", errors="replace")
    return "daemon recovery already failed" in text and "refusing to run" in text and "uncached" in text


def run_logged(command: list[str], log: Path, cwd: Path | None = None) -> bool:
    """Run a command with its output in a log file; True when it exits 0."""
    with log.open("wb") as handle:
        return subprocess.run(command, cwd=cwd, stdout=handle, stderr=subprocess.STDOUT).returncode == 0


def git_describe() -> str:
    """The version OBS reports, unless the caller set OBS_VERSION_OVERRIDE."""
    done = subprocess.run(
        ["git", "-C", str(SRC), "describe", "--always", "--tags", "--match", "[0-9]*.[0-9]*.[0-9]*"],
        capture_output=True,
        text=True,
    )
    return done.stdout.strip()


def prepare_toolchain(layout: Layout) -> bool:
    """Install the pinned zccache (install_zccache.py checks its SHA-256) as
    the compiler launcher and point ZCCACHE_CACHE_DIR at the dir CI saves."""
    installer = SRC / "rust/tools/linux-validate/install_zccache.py"
    done = subprocess.run(
        ["uv", "run", "--no-project", str(installer), str(layout.bin_dir)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if done.returncode != 0:
        return False
    os.environ["PATH"] = f"{layout.bin_dir}{os.pathsep}{os.environ['PATH']}"
    os.environ["ZCCACHE_CACHE_DIR"] = os.environ.get("ZCCACHE_CACHE_DIR") or str(layout.root / "zccache")
    return True


def stop_zccache() -> None:
    """Stop the daemon, so its index is flushed into the saved cache dir."""
    try:
        subprocess.run(["zccache", "stop"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    except OSError:
        pass


def start_display() -> subprocess.Popen[bytes]:
    """One headless display for the unit tests (several start OBS, which needs
    one on Linux) and the app launch."""
    display = f":{90 + random.randrange(9)}"
    server = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1920x1080x24"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    os.environ["DISPLAY"] = display
    os.environ["LIBGL_ALWAYS_SOFTWARE"] = "1"
    os.environ["QT_QPA_PLATFORM"] = "xcb"
    time.sleep(2)
    return server


def stop_display(server: subprocess.Popen[bytes]) -> None:
    """Stop the X server, SIGKILLing it if SIGTERM does not land."""
    server.terminate()
    try:
        server.wait(timeout=10)
    except subprocess.TimeoutExpired:
        server.kill()


def build_obs(tool: str, flags: str, build_type: str, layout: Layout) -> Path | None:
    """Configure and build one tool's tree; the libobs dir, or None on failure."""
    backend = "C" if os.environ.get("RUST_LIBOBS", "ON") == "OFF" else "HYBRID"
    print(f"== [{tool}] configure + build ({backend} libobs)", flush=True)
    # Every build makes libobs-with-rust. Install the toolchain
    # rust-toolchain.toml pins instead of relying on rustup's deprecated
    # auto-install (rust-lang/rustup#4836).
    rustup_log = layout.out / "rustup.log"
    if not run_logged(["rustup", "toolchain", "install"], log=rustup_log, cwd=SRC):
        print(tail(rustup_log, 20), flush=True)
        return None
    version = os.environ.get("OBS_VERSION_OVERRIDE") or git_describe()
    configure_log = layout.out / "configure.log"
    configure = configure_command(
        src=SRC, build=layout.build, build_type=build_type, flags=flags, backend=backend, version=version
    )
    if not run_logged(configure, log=configure_log):
        print(tail(configure_log, 40), flush=True)
        return None
    # The other tools build with -k 0: their skips (SKIPPED_TESTS) are targets
    # that cannot compile under them, so a failed target is expected there.
    build_log = layout.out / "build.log"
    build_command = ["cmake", "--build", str(layout.build), "--", "-k", "0"]
    if not run_logged(build_command, log=build_log) and tool == "asan":
        if not zccache_refused_uncached(build_log):
            print(error_lines(build_log), flush=True)
            return None
        # The zccache daemon died and sits in its recovery cool-down, refusing
        # to compile uncached: the tree is fine, the cache is not. Build once
        # more without the daemon rather than failing the pass (#3086).
        print("== [asan] zccache daemon unavailable; rebuilding uncached (ZCCACHE_DISABLE=1)", flush=True)
        os.environ["ZCCACHE_DISABLE"] = "1"
        if not run_logged(build_command, log=build_log):
            print(error_lines(build_log), flush=True)
            return None
    obs = layout.inst / "bin" / "obs"
    libraries = sorted(layout.inst.rglob("libobs.so.*"))
    if not os.access(obs, os.X_OK) or not libraries:
        print(error_lines(build_log), flush=True)
        return None
    return libraries[0].parent


def run_unit_tests(tool: str, layout: Layout) -> None:
    """Run each registered test as ctest would, under the tool."""
    print(f"== [{tool}] unit tests", flush=True)
    tests_dir = layout.out / "tests"
    tests_dir.mkdir(parents=True, exist_ok=True)
    listing = subprocess.run(
        ["ctest", "--test-dir", str(layout.build), "--show-only=json-v1"],
        capture_output=True,
        text=True,
    ).stdout
    tests = ctest_tests(listing)
    (tests_dir / "ctest.tsv").write_text(
        "".join(f"{t.name}\t{t.workdir}\t{chr(31).join(t.command)}\n" for t in tests), encoding="utf-8"
    )
    summary = []
    for test in tests:
        command = test_command(test)
        if not command:
            continue
        if is_skipped(tool, test.name):
            summary.append(f"{test.name} skipped (built with ASan)")
            continue
        summary.append(f"{test.name} exit={run_one_test(tool, test, command, layout)}")
    print("\n".join(summary), flush=True)
    (tests_dir / "summary.txt").write_text("\n".join(summary) + "\n", encoding="utf-8")


def run_one_test(tool: str, test: Test, command: list[str], layout: Layout) -> int:
    """One test's exit status; 124 when it hit the timeout, as `timeout` did."""
    tests_dir = layout.out / "tests"
    # ctest's own WORKING_DIRECTORY, or the build's test/cmocka as the shell's
    # `${wd:-$b/test/cmocka}` fallback.
    workdir = Path(test.workdir) if test.workdir else layout.build / "test" / "cmocka"
    env = None
    if tool == "valgrind":
        command = [*VALGRIND, f"--log-file={tests_dir / f'{test.name}.vg.log'}", *command]
    else:
        env = dict(os.environ, **sanitizer_options(tests_dir, f"{test.name}."))
    with (tests_dir / f"{test.name}.log").open("wb") as handle:
        try:
            done = subprocess.run(
                command,
                cwd=workdir,
                stdin=subprocess.DEVNULL,
                stdout=handle,
                stderr=subprocess.STDOUT,
                env=env,
                timeout=TEST_TIMEOUT,
            )
        except subprocess.TimeoutExpired:
            return 124
    return done.returncode


def run_app_launch(tool: str, secs: int, layout: Layout, libdir: Path) -> int:
    """Launch OBS headless, SIGINT it after the window, and record the exit."""
    print(f"== [{tool}] app launch ({secs}s)", flush=True)
    # PCRE2 JIT runs code from a dual-mapped memfd that valgrind cannot follow
    # (SEGV in /memfd:sljit).
    os.environ["QT_ENABLE_REGEXP_JIT"] = "0"
    time.sleep(2)
    # A fresh profile each launch, so first-run paths are exercised every time.
    home = Path(tempfile.mkdtemp(prefix="profile.", dir=layout.out))
    os.environ["HOME"] = str(home)
    existing = os.environ.get("LD_LIBRARY_PATH")
    os.environ["LD_LIBRARY_PATH"] = f"{libdir}{os.pathsep}{existing}" if existing else str(libdir)
    window, grace = launch_timing(tool, secs)
    binary = str(layout.inst / "bin" / "obs")
    command = launch_command(tool, binary, layout.out)
    with (layout.out / "app.log").open("wb") as handle:
        session = subprocess.Popen(
            ["dbus-run-session", "--", *command], cwd=layout.inst / "bin", stdout=handle, stderr=subprocess.STDOUT
        )
    time.sleep(1)
    # OBS, or valgrind running it; dbus-run-session's other child is dbus-daemon.
    app = pgrep_child(session.pid, binary)
    wait_for_exit(session, window)
    if app is not None:
        signal_pid(app, signal.SIGINT)
    status = "closed"
    if wait_for_exit(session, grace):
        status = "exited"
    elif app is not None and pid_alive(app):
        capture_hang(layout, app, session.pid)
        status = "killed"
    line = f"app exit={session.wait()} ({status})"
    print(line, flush=True)
    (layout.out / "app-exit.txt").write_text(line + "\n", encoding="utf-8")
    logs = home / ".config" / "obs-studio" / "logs"
    if logs.is_dir():
        shutil.copytree(logs, layout.out / "obs-logs", dirs_exist_ok=True)
    return 0


def pgrep_child(parent: int, pattern: str) -> int | None:
    """`pgrep -P <parent> -f <pattern> | head -1`."""
    done = subprocess.run(["pgrep", "-P", str(parent), "-f", pattern], capture_output=True, text=True)
    pids = done.stdout.split()
    return int(pids[0]) if pids else None


def pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except OSError:
        return False
    return True


def signal_pid(pid: int, number: int) -> None:
    try:
        os.kill(pid, number)
    except OSError:
        pass


def wait_for_exit(session: subprocess.Popen[bytes], seconds: int) -> bool:
    """Wait up to `seconds` for the session to end; True when it ended."""
    for _ in range(seconds):
        if session.poll() is not None:
            return True
        time.sleep(1)
    return session.poll() is not None


def capture_hang(layout: Layout, app: int, session: int) -> None:
    """Still running after SIGINT + grace: record where it hangs (#462).

    gdb needs sudo to attach to a non-child under ptrace_scope=1.
    """
    gdb = ["gdb", "-batch", "-ex", "set pagination off", "-ex", "info threads", "-ex", "thread apply all bt", "-p", str(app)]
    backtrace = layout.out / "hang-backtrace.txt"
    with backtrace.open("wb") as handle:
        attached = subprocess.run(["sudo", "-n", *gdb], stdout=handle, stderr=subprocess.STDOUT)
    if attached.returncode != 0:
        with backtrace.open("wb") as handle:
            subprocess.run(gdb, stdout=handle, stderr=subprocess.STDOUT)
    signal_pid(app, signal.SIGKILL)
    signal_pid(session, signal.SIGKILL)


def parse_args(argv: list[str]) -> tuple[str, int] | None:
    """(tool, launch seconds), or None after writing the usage line."""
    tool = argv[1] if len(argv) > 1 else ""
    if tool not in TOOL_FLAGS:
        sys.stderr.write(f"usage: {Path(argv[0]).name} asan|tsan|valgrind [launch-seconds]\n")
        return None
    if len(argv) < 3:
        return tool, 45
    try:
        return tool, int(argv[2])
    except ValueError:
        sys.stderr.write(f"usage: {Path(argv[0]).name} asan|tsan|valgrind [launch-seconds]\n")
        return None


def main(argv: list[str]) -> int:
    parsed = parse_args(argv)
    if parsed is None:
        return 2
    tool, secs = parsed
    flags, build_type = TOOL_FLAGS[tool]
    layout = Layout.for_tool(tool, build_type)
    try:
        layout.root.mkdir(parents=True, exist_ok=True)
    except OSError as error:
        sys.stderr.write(f"{error}\n")
        return 1
    if not os.environ.get("SKIP_TESTS"):
        shutil.rmtree(layout.out, ignore_errors=True)
    layout.out.mkdir(parents=True, exist_ok=True)
    if not prepare_toolchain(layout):
        return 1
    try:
        libdir = build_obs(tool, flags, build_type, layout)
        if libdir is None:
            return 1
        os.environ.update(sanitizer_options(layout.out))
        server = start_display()
        try:
            if not os.environ.get("SKIP_TESTS"):
                run_unit_tests(tool, layout)
            return run_app_launch(tool, secs, layout, libdir)
        finally:
            stop_display(server)
    finally:
        stop_zccache()


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
