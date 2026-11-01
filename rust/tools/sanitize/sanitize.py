#!/usr/bin/env -S uv run --script
"""Builds OBS for one sanitizer tool, runs the cmocka tests under it, then
launches OBS headless (Xvfb) and closes it with SIGINT after a timeout.

    uv run --no-project rust/tools/sanitize/sanitize.py asan|tsan|valgrind [launch-seconds]
    uv run --no-project rust/tools/sanitize/sanitize.py --list-unwind [tool]

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

import functools
import json
import os
import random
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC = HERE.parent.parent.parent
# One test binary gets 10 minutes, the same bound the shell's `timeout 600` set.
TEST_TIMEOUT = 600
# tool -> (compile flags, CMAKE_BUILD_TYPE). -ffp-contract=off: the runner's gcc
# fuses a multiply and an add into one FMA when nothing forbids it, the Tier 2
# image's gcc does not, and Rust never does, so float results would depend on
# the host (#3362).
TOOL_FLAGS = {
    "asan": ("-fsanitize=address,undefined -fno-omit-frame-pointer -ffp-contract=off", "RelWithDebInfo"),
    "tsan": ("-fsanitize=thread -fno-omit-frame-pointer -ffp-contract=off", "RelWithDebInfo"),
    "valgrind": ("-fno-omit-frame-pointer -ffp-contract=off", "Debug"),
}
# test_formatted_filename and test_utf8 hardcode -fsanitize=address,undefined
# (test/cmocka/CMakeLists.txt, obs-rust/obs-studio#159), which TSan rejects and
# valgrind can't run. They are covered by the asan pass, so the other tools
# build with -k 0 and skip them.
SKIPPED_TESTS = re.compile(r"^(test_formatted_filename|test_utf8)$")
# Where the build declares cmocka test targets, for the unwinder selection
# below: the declarative graph, one CMake fragment per test (#137), and the
# directory's own CMake files for the tests that predate the fragments.
TEST_GRAPH = "test/cmocka/build/test-graph.json"
TEST_FRAGMENTS = ("test/cmocka/CMakeLists.txt", "test/cmocka/*.cmake", "test/cmocka/tests.d/*.cmake")
# A test can reach FFmpeg when its own link libraries name one of these.
# OBS::libobs is deliberately not one of them: libobs links FFmpeg itself
# (libobs/CMakeLists.txt:15), so counting it would put every test back on the
# slow unwinder and save nothing. libobs only allocates inside FFmpeg on its
# media paths, and the tests that drive those link FFmpeg themselves.
FFMPEG_LIBRARY = re.compile(r"FFmpeg::\w+|OBS::media-playback")
# The one hand-kept part of the selection, kept here next to SKIPPED_TESTS so
# the file has a single place for per-test exceptions. It holds what the scan
# above cannot derive: test_core_module_load links no FFmpeg library, it
# dlopens every plugin module (OBS::libobs-core-modules) and reaches
# obs-ffmpeg that way, and it was also the one test unstable under the fast
# unwinder, 24 of 30 runs failing in XOpenDisplay during hotkey
# initialization (#3249).
SLOW_UNWIND_TESTS = frozenset({"test_core_module_load"})
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
# Full allocation stacks where they matter: the fast unwinder stops at
# av_malloc, because distro FFmpeg has no frame pointers (#2035), so a leak
# allocated inside FFmpeg can no longer be traced back to its OBS caller.
# The slow unwinder pays for that on every allocation in the process, and it
# is most of the time the allocation-heavy asan tests spend (#3249, measured:
# test_context_names 29 s -> 2 s, test_canvas_destroy_scene_refs 18 s -> 2 s,
# test_video_output_formats 18 s -> 4 s, test_dock_proportions 17 s -> 4 s).
# So only the tests that can reach FFmpeg keep it; see UnwindPolicy.
SLOW_UNWIND = "fast_unwind_on_malloc=0"
FAST_UNWIND = "fast_unwind_on_malloc=1"
SANITIZER_OPTIONS = {
    "asan": f"detect_leaks=1:{SLOW_UNWIND}:abort_on_error=0:halt_on_error=0",
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
    # RUN_SERIAL or a RESOURCE_LOCK: run alone, after the concurrent pool.
    serial: bool = False


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


def sanitizer_options(directory: Path, stem: str = "", fast_unwind: bool = False) -> dict[str, str]:
    """The three *_OPTIONS values with log_path=<directory>/<stem><kind>.

    The shell exported them for the whole run (log_path=<out>/<kind>) and
    overrode the log_path per test (log_path=<out>/tests/<name>.<kind>).
    `fast_unwind` swaps ASan's slow allocation unwinder for the fast one, for
    a test that cannot reach FFmpeg (#3249).
    """
    options = SANITIZER_OPTIONS
    if fast_unwind:
        options = {kind: value.replace(SLOW_UNWIND, FAST_UNWIND) for kind, value in options.items()}
    return {f"{kind.upper()}_OPTIONS": f"{value}:log_path={directory / f'{stem}{kind}'}" for kind, value in options.items()}


@dataclass(frozen=True)
class UnwindPolicy:
    """Which asan tests may use the fast allocation unwinder (#3249).

    `declared` is every test target the build declares, `ffmpeg` the ones
    whose link libraries name FFmpeg. A test the scan does not find is not in
    `declared` and keeps the slow unwinder, so an unparsed or newly added
    declaration falls back to what the whole pass did before this split.
    """

    declared: frozenset[str]
    ffmpeg: frozenset[str]

    def fast_unwind(self, tool: str, name: str) -> bool:
        """True when `name` may run with fast_unwind_on_malloc=1."""
        if tool != "asan":
            return False
        return name in self.declared and name not in self.ffmpeg and name not in SLOW_UNWIND_TESTS


def cmake_call_bodies(text: str, call: str) -> list[str]:
    """The argument text of every `call(...)` in one CMake file.

    A balanced-paren scan, because the arguments hold generator expressions
    with parentheses of their own, which a regex would stop the call on.
    """
    bodies = []
    for match in re.finditer(rf"\b{call}\s*\(", text):
        depth, index = 1, match.end()
        while index < len(text) and depth:
            depth += {"(": 1, ")": -1}.get(text[index], 0)
            index += 1
        bodies.append(text[match.end() : index - 1])
    return bodies


def first_word(body: str) -> str:
    """A CMake call's first argument: the target it acts on."""
    words = body.split()
    return words[0] if words else ""


def scan_unwind_policy(src: Path) -> UnwindPolicy:
    """Read the build's own test declarations into an UnwindPolicy.

    No build needed: both sources are checked in, so
    `sanitize.py --list-unwind` shows the selection the pass will make.
    """
    declared: set[str] = set()
    ffmpeg: set[str] = set()
    graph = src / TEST_GRAPH
    if graph.is_file():
        targets = json.loads(graph.read_text(encoding="utf-8")).get("targets", {})
        for name, spec in targets.items():
            declared.add(name)
            if FFMPEG_LIBRARY.search(json.dumps(spec.get("libraries", []))):
                ffmpeg.add(name)
    for fragment in sorted({path for pattern in TEST_FRAGMENTS for path in src.glob(pattern)}):
        body_text = fragment.read_text(encoding="utf-8", errors="replace")
        declared.update(filter(None, (first_word(b) for b in cmake_call_bodies(body_text, "add_executable"))))
        for body in cmake_call_bodies(body_text, "target_link_libraries"):
            if first_word(body) and FFMPEG_LIBRARY.search(body):
                ffmpeg.add(first_word(body))
    return UnwindPolicy(declared=frozenset(declared), ffmpeg=frozenset(ffmpeg))


@functools.lru_cache(maxsize=None)
def unwind_policy(src: Path = SRC) -> UnwindPolicy:
    """The policy, scanned once per run: run_one_test asks it per test."""
    return scan_unwind_policy(src)


def list_unwind(tool: str) -> int:
    """Print the unwinder option every declared test would run with."""
    policy = unwind_policy()
    fast = {name for name in policy.declared if policy.fast_unwind(tool, name)}
    for name in sorted(policy.declared):
        print(f"{FAST_UNWIND if name in fast else SLOW_UNWIND}\t{name}")
    slow = policy.declared - fast
    print(f"# {tool}: {len(fast)} fast, {len(slow)} slow, of {len(policy.declared)} declared tests")
    print(f"# slow: {len(policy.ffmpeg & policy.declared)} link FFmpeg, {len(SLOW_UNWIND_TESTS & policy.declared)} in SLOW_UNWIND_TESTS")
    print(f"# a ctest test this scan does not declare keeps {SLOW_UNWIND}, as the whole pass did")
    return 0


def tool_cmake_args(tool: str) -> list[str]:
    """Configure options one tool needs on top of configure_command. The
    cmocka tests that build with their own ASan/UBSan (obs_test_sanitize,
    #159) skip it when the build flags already have -fsanitize=, as asan's
    and tsan's do; valgrind's don't, and valgrind can't run an ASan binary,
    so 101 of its tests never ran (#3238)."""
    return ["-DOBS_TEST_SANITIZE_PER_TARGET=OFF"] if tool == "valgrind" else []


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
        # The conversion ratchet's clang replay (#3118) reads this tree's flags.
        "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
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
        serial = any(
            (p.get("name") == "RUN_SERIAL" and p.get("value") in (True, "ON", "TRUE", "1", 1))
            or (p.get("name") == "RESOURCE_LOCK" and p.get("value"))
            for p in properties
        )
        tests.append(
            Test(
                name=str(entry.get("name", "")),
                workdir=workdir,
                command=tuple(str(a) for a in command),
                serial=serial,
            )
        )
    return sorted(tests, key=lambda test: test.name)


# How many unit tests a pass runs side by side (#3238). tsan on every core:
# unit tests 20.3 -> 8.2 min (41275822723). valgrind with 4 jobs broke 9 tests
# on timing (41275824985); with 2 jobs on 4 cores its pool part ran 1.78x
# faster and broke none, and serial no longer fits the job's 150 minutes.
POOLED = {"asan", "tsan"}
VALGRIND_JOBS = 2


def unit_test_jobs(tool: str) -> int:
    """How many unit tests run at once: SANITIZE_JOBS when set, else every
    core for a pooled pass and VALGRIND_JOBS for valgrind (#3238)."""
    jobs = os.environ.get("SANITIZE_JOBS", "")
    if jobs.isdigit() and int(jobs) > 0:
        return int(jobs)
    return (os.cpu_count() or 1) if tool in POOLED else VALGRIND_JOBS


def private_displays(tool: str) -> bool:
    """Whether each unit test gets its own Xvfb server (#3238): whenever a
    pass runs its tests side by side, as on one shared display a test that
    adds RandR monitors breaks another (test_xshm_monitor_late)."""
    return unit_test_jobs(tool) > 1


# The shared display is :90 to :98; a private one gets the next number of a
# per-run counter, so two tests started at once never pick the same server.
PRIVATE_DISPLAY_FIRST = 100
PRIVATE_SCREEN = "-screen 0 1920x1080x24"


class DisplayNumbers:
    """Unique Xvfb server numbers for the tests of one run."""

    def __init__(self, first: int = PRIVATE_DISPLAY_FIRST):
        self._next = first
        self._lock = threading.Lock()

    def take(self) -> int:
        with self._lock:
            number = self._next
            self._next += 1
            return number


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
    ) + tool_cmake_args(tool)
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
    summary = run_tests(tool, tests, layout, jobs=unit_test_jobs(tool), private_displays=private_displays(tool))
    (tests_dir / "summary.txt").write_text("\n".join(summary) + "\n", encoding="utf-8")


def run_tests(
    tool: str, tests: list[Test], layout: Layout, runner=None, jobs: int | None = None, private_displays: bool = False
) -> list[str]:
    """Run the tests `jobs` at a time (default: every core), then the serial
    ones alone; the summary lines in test order (#3251). With
    `private_displays`, each test runs under its own Xvfb server (#3238).

    Each test writes its own `<name>.log` and sanitizer log prefix, so
    concurrent tests do not collide. Each line prints as its test finishes,
    and `durations.tsv` records every test's wall time, slowest visible first.
    """
    runner = runner or run_one_test
    tests_dir = layout.out / "tests"
    tests_dir.mkdir(parents=True, exist_ok=True)
    lines: dict[str, str] = {}
    durations: dict[str, float] = {}
    runnable = []
    for test in tests:
        command = test_command(test)
        if not command:
            continue
        if is_skipped(tool, test.name):
            lines[test.name] = f"{test.name} skipped (built with ASan)"
            continue
        runnable.append((test, command))

    numbers = DisplayNumbers() if private_displays else None

    def one(item: tuple[Test, list[str]]) -> None:
        test, command = item
        start = time.monotonic()
        if numbers:
            code = runner(tool, test, command, layout, display=numbers.take())
        else:
            code = runner(tool, test, command, layout)
        durations[test.name] = time.monotonic() - start
        lines[test.name] = f"{test.name} exit={code}"
        print(f"{lines[test.name]} ({durations[test.name]:.1f}s)", flush=True)

    with ThreadPoolExecutor(max_workers=jobs or os.cpu_count() or 1) as pool:
        list(pool.map(one, [item for item in runnable if not item[0].serial]))
    for item in runnable:
        if item[0].serial:
            one(item)
    (tests_dir / "durations.tsv").write_text(
        "".join(f"{t.name}\t{durations[t.name]:.2f}\n" for t in tests if t.name in durations), encoding="utf-8"
    )
    policy = unwind_policy()
    (tests_dir / "unwind.tsv").write_text(
        "".join(f"{t.name}\t{FAST_UNWIND if policy.fast_unwind(tool, t.name) else SLOW_UNWIND}\n" for t, _ in runnable),
        encoding="utf-8",
    )
    return [lines[t.name] for t in tests if t.name in lines]


def run_one_test(tool: str, test: Test, command: list[str], layout: Layout, display: int | None = None) -> int:
    """One test's exit status; 124 when it hit the timeout, as `timeout` did.

    With `display`, the test runs under its own Xvfb server of that number
    (xvfb-run), without the shared DISPLAY, so a test registered with
    xvfb-run-or-display.sh starts its own server too (#3238)."""
    tests_dir = layout.out / "tests"
    # ctest's own WORKING_DIRECTORY, or the build's test/cmocka as the shell's
    # `${wd:-$b/test/cmocka}` fallback.
    workdir = Path(test.workdir) if test.workdir else layout.build / "test" / "cmocka"
    env = None
    if tool == "valgrind":
        command = [*VALGRIND, f"--log-file={tests_dir / f'{test.name}.vg.log'}", *command]
    else:
        fast = unwind_policy().fast_unwind(tool, test.name)
        env = dict(os.environ, **sanitizer_options(tests_dir, f"{test.name}.", fast_unwind=fast))
    if display is not None:
        # Its own auth file, outside xvfb-run's temporary directory: with the
        # file inside, its cleanup failed now and then and xvfb-run's exit 5
        # replaced the test's.
        auth = layout.build / "xauth" / test.name
        auth.parent.mkdir(parents=True, exist_ok=True)
        command = ["xvfb-run", "-n", str(display), "-f", str(auth), "-s", PRIVATE_SCREEN, *command]
        env = {key: value for key, value in (env or os.environ).items() if key != "DISPLAY"}
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


USAGE = "usage: {name} asan|tsan|valgrind [launch-seconds] | {name} --list-unwind [tool]\n"


def parse_args(argv: list[str]) -> tuple[str, int] | None:
    """(tool, launch seconds), or None after writing the usage line."""
    tool = argv[1] if len(argv) > 1 else ""
    if tool not in TOOL_FLAGS:
        sys.stderr.write(USAGE.format(name=Path(argv[0]).name))
        return None
    if len(argv) < 3:
        return tool, 45
    try:
        return tool, int(argv[2])
    except ValueError:
        sys.stderr.write(USAGE.format(name=Path(argv[0]).name))
        return None


def main(argv: list[str]) -> int:
    # The selection only reads checked-in build files, so it needs no build.
    if len(argv) > 1 and argv[1] == "--list-unwind":
        return list_unwind(argv[2] if len(argv) > 2 else "asan")
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
