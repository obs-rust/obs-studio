#!/usr/bin/env -S uv run --script
"""The sanitizer harness builds, tests and launches OBS for one tool (#962), in Python.

    uv run python -m unittest ci/test_sanitize_harness.py

The rule lives in rust/tools/sanitize/sanitize.py since the port off the
178-line shell script, and sanitizers.yaml runs it through
`uv run --no-project`. This drives the port's decisions over a canned
`ctest --show-only=json-v1` payload and stub paths: the per-tool build flags
and build type, the argument check, the ctest-to-per-test mapping (a wrapper in
front of the binary is dropped, an unregistered command or a non-executable
binary is not run), the two tests only the asan pass may run, the per-test
sanitizer log paths, the configure line's zccache launcher and libobs backend,
and the valgrind launch window.
"""

from __future__ import annotations

import importlib.util
import io
import json
import os
import stat
import sys
import tempfile
import threading
import time
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SANITIZE = ROOT / "rust" / "tools" / "sanitize"
MODULE = SANITIZE / "sanitize.py"
SHELL_SCRIPT = SANITIZE / "sanitize.sh"
DEPS_SHELL_SCRIPT = SANITIZE / "install-ubuntu-deps.sh"

# `ctest --show-only=json-v1`, trimmed: the properties list, a test with no
# command (not runnable), and a wrapper (xvfb-run) in front of the binary.
CTEST_JSON = json.dumps(
    {
        "tests": [
            {
                "name": "test_utf8",
                "command": ["/b/test/cmocka/test_utf8", "--verbose"],
                "properties": [{"name": "WORKING_DIRECTORY", "value": "/b/test/cmocka"}],
            },
            {
                "name": "test_core",
                "command": ["xvfb-run", "-a", "/b/test/cmocka/test_core"],
                "properties": [{"name": "WORKING_DIRECTORY", "value": "/b/test/cmocka"}],
            },
            {"name": "test_no_command", "properties": []},
            {"name": "test_no_workdir", "command": ["/b/test/cmocka/test_no_workdir"]},
        ]
    }
)


def load_module():
    """rust/tools/sanitize/sanitize.py by path: `uv run python -m unittest
    ci/test_sanitize_harness.py` does not put that directory on sys.path, and no
    file in this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("sanitize_harness", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def write_stub(directory: Path, name: str, body: str) -> Path:
    """A stub for one tool the harness shells out to."""
    path = directory / name
    path.write_text(f"#!/bin/sh\n{body}", encoding="utf-8")
    path.chmod(0o755)
    return path


# `cmake --build <build> -- -k 0` leaves the app and libobs where the shell
# expected them; the configure call needs nothing.
CMAKE_STUB = """\
if [ "$1" = "--build" ]; then
  d="$2/rundir/RelWithDebInfo"
  mkdir -p "$d/bin" "$d/lib"
  printf '#!/bin/sh\\nexit 0\\n' > "$d/bin/obs"
  chmod +x "$d/bin/obs"
  : > "$d/lib/libobs.so.1"
fi
exit 0
"""
# `ctest --show-only=json-v1`: one registered test, run from the stub dir.
CTEST_STUB = """\
cat <<'JSON'
{"tests": [{"name": "test_core", "command": ["@BINARY@"],
            "properties": [{"name": "WORKING_DIRECTORY", "value": "@WORKDIR@"}]}]}
JSON
exit 0
"""


# zccache gives up when its daemon dies and leaves this in build.log; CI hit it
# on 2026-10-30 (run 41097086538, asan) and failed the pass for it.
ZCCACHE_REFUSAL = (
    "FAILED: [code=125] test/cmocka/CMakeFiles/test_core.dir/test_core.c.o \n"
    "zccache[err][D]: daemon unavailable at /b/zccache/daemon.sock (cannot start daemon at "
    "/b/zccache/daemon.sock: daemon recovery already failed on this cache root and is in its "
    "cool-down: daemon process 8981 exists but not accepting connections after retrying); "
    "refusing to run /usr/bin/cc uncached. Run 'zccache status'; set ZCCACHE_DISABLE=1 to "
    "compile without the daemon.\n"
)


# Tests whose checks depend on wall-clock timing and fail when four ASan
# processes share four cores (dispatch 41242258179, #3238). They hold the
# ctest RESOURCE_LOCK "timing", so the asan pool runs them alone after the
# others; in Tier 2, ctest only keeps them from overlapping each other.
TIMING_TESTS_D = ("test_async_delay_audio", "test_dock_proportions", "test_slideshow_mem_cap")
TIMING_TESTS_GRAPH = ("test_deinterlace_mode_load",)
TEST_GRAPH = ROOT / "test" / "cmocka" / "build" / "test-graph.json"


class TimingLockTest(unittest.TestCase):
    def test_timing_tests_hold_the_timing_lock(self):
        for name in TIMING_TESTS_D:
            text = (ROOT / "test" / "cmocka" / "tests.d" / f"{name}.cmake").read_text(encoding="utf-8")
            self.assertRegex(text, rf"set_tests_properties\(\s*{name}\s[^)]*RESOURCE_LOCK\s+timing", name)
        tests = json.loads(TEST_GRAPH.read_text(encoding="utf-8"))["tests"]
        for name in TIMING_TESTS_GRAPH:
            self.assertIn("timing", tests[name].get("resource_locks", []), name)

    def test_a_resource_lock_runs_alone_in_the_pool(self):
        payload = json.dumps(
            {"tests": [{"name": "t", "command": ["/b/t"], "properties": [{"name": "RESOURCE_LOCK", "value": ["timing"]}]}]}
        )
        self.assertTrue(load_module().ctest_tests(payload)[0].serial)


class SanitizeHarnessTest(unittest.TestCase):
    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.out = Path(self.tmp.name)

    def invoke(self, *argv: str) -> tuple[int, str, str]:
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err):
            code = self.module.main(["sanitize.py", *argv])
        return code, out.getvalue(), err.getvalue()

    def executable(self, name: str) -> Path:
        path = self.out / name
        path.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)
        return path

    def test_tool_flags_and_build_type(self):
        self.assertEqual(
            self.module.TOOL_FLAGS["asan"],
            ("-fsanitize=address,undefined -fno-omit-frame-pointer", "RelWithDebInfo"),
        )
        self.assertEqual(
            self.module.TOOL_FLAGS["tsan"],
            ("-fsanitize=thread -fno-omit-frame-pointer", "RelWithDebInfo"),
        )
        self.assertEqual(self.module.TOOL_FLAGS["valgrind"], ("-fno-omit-frame-pointer", "Debug"))

    def test_missing_or_unknown_tool_is_a_usage_error(self):
        for argv in ((), ("bogus",), ("ASAN",)):
            with self.subTest(argv=argv):
                code, out, err = self.invoke(*argv)
                self.assertEqual(code, 2, out + err)
                self.assertIn("asan|tsan|valgrind", err)

    def test_ctest_payload_becomes_the_test_list(self):
        tests = self.module.ctest_tests(CTEST_JSON)
        self.assertEqual([t.name for t in tests], ["test_core", "test_no_workdir", "test_utf8"])
        by_name = {t.name: t for t in tests}
        self.assertEqual(by_name["test_utf8"].workdir, "/b/test/cmocka")
        self.assertEqual(by_name["test_utf8"].command, ("/b/test/cmocka/test_utf8", "--verbose"))
        # No command: ctest cannot run it, so the harness never runs it.
        self.assertNotIn("test_no_command", by_name)
        # No WORKING_DIRECTORY property: the harness falls back to the build's
        # test/cmocka, the same way the shell did.
        self.assertEqual(by_name["test_no_workdir"].workdir, "")

    def test_a_wrapper_in_front_of_the_binary_is_dropped(self):
        binary = self.executable("test_core")
        wrapped = self.module.Test(name="test_core", workdir="", command=("xvfb-run", "-a", str(binary)))
        self.assertEqual(self.module.test_command(wrapped), [str(binary)])
        self.assertEqual(self.module.test_command(self.module.Test("test_none", "", ())), [])
        # The binary the test names never appears in the command line.
        self.assertEqual(self.module.test_command(self.module.Test("test_other", "", (str(binary),))), [])

    def test_a_non_executable_binary_is_not_run(self):
        missing = self.out / "test_missing"
        self.assertEqual(self.module.test_command(self.module.Test("test_missing", "", (str(missing),))), [])
        plain = self.out / "test_plain"
        plain.write_text("", encoding="utf-8")
        self.assertEqual(self.module.test_command(self.module.Test("test_plain", "", (str(plain),))), [])

    def test_only_the_asan_pass_skips_the_hardcoded_tests(self):
        # test_formatted_filename and test_utf8 hardcode
        # -fsanitize=address,undefined, so tsan and valgrind skip them (#159).
        for name in ("test_utf8", "test_formatted_filename"):
            self.assertFalse(self.module.is_skipped("asan", name), name)
            self.assertTrue(self.module.is_skipped("tsan", name), name)
            self.assertTrue(self.module.is_skipped("valgrind", name), name)
        for tool in ("asan", "tsan", "valgrind"):
            self.assertFalse(self.module.is_skipped(tool, "test_core"), tool)

    def test_per_test_sanitizer_logs(self):
        options = self.module.sanitizer_options(self.out / "tests", "test_core.")
        self.assertTrue(options["ASAN_OPTIONS"].startswith("detect_leaks=1:"), options)
        self.assertIn("halt_on_error=0", options["TSAN_OPTIONS"])
        for key, kind in (("ASAN_OPTIONS", "asan"), ("UBSAN_OPTIONS", "ubsan"), ("TSAN_OPTIONS", "tsan")):
            self.assertTrue(options[key].endswith(f"log_path={self.out / 'tests' / f'test_core.{kind}'}"), options[key])
        # The app's own run writes to <out>/asan, <out>/ubsan and <out>/tsan.
        app = self.module.sanitizer_options(self.out)
        self.assertTrue(app["ASAN_OPTIONS"].endswith(f"log_path={self.out / 'asan'}"), app)

    def test_configure_names_zccache_and_the_libobs_backend(self):
        command = self.module.configure_command(
            src=Path("/src"),
            build=Path("/b"),
            build_type="RelWithDebInfo",
            flags="-fsanitize=address,undefined",
            backend="C",
            version="30.0.0",
        )
        self.assertEqual(command[:2], ["cmake", "-S"])
        for flag in (
            "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
            "-DCMAKE_C_COMPILER_LAUNCHER=zccache",
            "-DCMAKE_CXX_COMPILER_LAUNCHER=zccache",
            "-DCMAKE_C_FLAGS=-fsanitize=address,undefined",
            "-DOBS_LIBOBS_BACKEND=C",
            "-DOBS_VERSION_OVERRIDE=30.0.0",
            "-DENABLE_UNIT_TESTS=ON",
            "-DENABLE_BROWSER=OFF",
            # The conversion ratchet's clang replay reads this tree (#3118).
            "-DCMAKE_EXPORT_COMPILE_COMMANDS=ON",
        ):
            self.assertIn(flag, command, flag)
        without = self.module.configure_command(
            src=Path("/src"),
            build=Path("/b"),
            build_type="Debug",
            flags="-fno-omit-frame-pointer",
            backend="HYBRID",
            version="",
        )
        self.assertFalse([a for a in without if a.startswith("-DOBS_VERSION_OVERRIDE")], without)

    def test_valgrind_builds_no_per_target_asan(self):
        """valgrind can't run an ASan binary, so its build turns off the cmocka
        tests' own ASan/UBSan (obs_test_sanitize, #159); asan and tsan already
        have -fsanitize= in the build flags, which turns it off for them (#3238)."""
        self.assertIn("-DOBS_TEST_SANITIZE_PER_TARGET=OFF", self.module.tool_cmake_args("valgrind"))
        self.assertEqual(self.module.tool_cmake_args("asan"), [])
        self.assertEqual(self.module.tool_cmake_args("tsan"), [])

    def test_valgrind_stretches_the_launch_window(self):
        self.assertEqual(self.module.launch_timing("asan", 45), (45, 60))
        self.assertEqual(self.module.launch_timing("tsan", 45), (45, 60))
        self.assertEqual(self.module.launch_timing("valgrind", 45), (180, 240))
        binary = "/b/rundir/RelWithDebInfo/bin/obs"
        obs = [binary, "--disable-shutdown-check", "--disable-updater", "--verbose"]
        self.assertEqual(self.module.launch_command("asan", binary, self.out), obs)
        valgrind = self.module.launch_command("valgrind", binary, self.out)
        self.assertEqual(valgrind[0], "valgrind")
        self.assertIn("--track-origins=yes", valgrind)
        # valgrind runs OBS: its log comes first, the app's arguments after.
        self.assertEqual(valgrind[valgrind.index(f"--log-file={self.out / 'app.vg.log'}") + 1 :], obs)

    def test_a_whole_run_writes_what_the_triage_reads(self):
        stubs = self.out / "stubs"
        stubs.mkdir()
        write_stub(stubs, "rustup", "exit 0\n")
        write_stub(stubs, "zccache", "exit 0\n")
        write_stub(stubs, "Xvfb", "exec sleep 30\n")
        # The asan pass runs each test under its own xvfb-run (#3238).
        write_stub(stubs, "xvfb-run", 'shift 6\nexec "$@"\n')
        write_stub(stubs, "dbus-run-session", "exit 0\n")
        # No OBS child of the session: the launch is a clean "exited".
        write_stub(stubs, "pgrep", "exit 1\n")
        write_stub(stubs, "cmake", CMAKE_STUB)
        write_stub(stubs, "test_core", "exit 0\n")
        ctest = CTEST_STUB.replace("@BINARY@", str(stubs / "test_core")).replace("@WORKDIR@", str(stubs))
        write_stub(stubs, "ctest", ctest)
        out = self.out / "out"
        env = dict(
            os.environ,
            SAN_OUT=str(out),
            OBS_VERSION_OVERRIDE="30.0.0",
            PATH=f"{stubs}{os.pathsep}{os.environ['PATH']}",
        )
        # prepare_toolchain installs the pinned zccache from the network; the
        # rest of the run is the stub toolchain above.
        printed = io.StringIO()
        with patch.dict(os.environ, env, clear=True), patch.object(self.module, "prepare_toolchain", return_value=True):
            with redirect_stdout(printed):
                code = self.module.main(["sanitize.py", "asan", "1"])
        self.assertEqual(code, 0, printed.getvalue())
        self.assertIn("== [asan] app launch (1s)", printed.getvalue())
        result = out / "asan"
        self.assertIn("test_core exit=0", (result / "tests" / "summary.txt").read_text(encoding="utf-8"))
        self.assertTrue((result / "tests" / "ctest.tsv").exists())
        self.assertEqual((result / "app-exit.txt").read_text(encoding="utf-8"), "app exit=0 (exited)\n")
        for name in ("rustup.log", "configure.log", "build.log", "app.log"):
            self.assertTrue((result / name).exists(), name)

    def test_the_shell_scripts_are_gone(self):
        for script in (SHELL_SCRIPT, DEPS_SHELL_SCRIPT):
            self.assertFalse(script.exists(), f"{script.name} still exists")

    def test_zccache_daemon_refusal_is_recognised(self):
        log = self.out / "build.log"
        log.write_text(ZCCACHE_REFUSAL, encoding="utf-8")
        self.assertTrue(self.module.zccache_refused_uncached(log))
        log.write_text(
            "FAILED: [code=1] libobs/CMakeFiles/libobs-objects.dir/foo.c.o\n"
            "foo.c:1:1: error: conversion from 'long long int' to 'int' may change value [-Werror=conversion]\n",
            encoding="utf-8",
        )
        self.assertFalse(self.module.zccache_refused_uncached(log))

    def layout(self):
        return self.module.Layout(
            root=self.out,
            out=self.out / "asan",
            build=self.out / "build-asan",
            inst=self.out / "build-asan" / "rundir" / "RelWithDebInfo",
            bin_dir=self.out / "bin",
        )

    def drive_build(self, failure: str, recover: bool):
        """build_obs with a stubbed run_logged: the build fails once with
        `failure`, then succeeds only when ZCCACHE_DISABLE=1 and `recover`."""
        layout = self.layout()
        layout.out.mkdir(parents=True, exist_ok=True)
        calls = []

        def fake_run_logged(command, log, cwd=None):
            calls.append((command, os.environ.get("ZCCACHE_DISABLE")))
            if command[:2] == ["cmake", "--build"]:
                if os.environ.get("ZCCACHE_DISABLE") == "1" and recover:
                    (layout.inst / "bin").mkdir(parents=True, exist_ok=True)
                    obs = layout.inst / "bin" / "obs"
                    obs.write_text("#!/bin/sh\n", encoding="utf-8")
                    obs.chmod(0o755)
                    (layout.inst / "lib").mkdir(parents=True, exist_ok=True)
                    (layout.inst / "lib" / "libobs.so.1").write_text("", encoding="utf-8")
                    return True
                log.write_text(failure, encoding="utf-8")
                return False
            return True

        with patch.object(self.module, "run_logged", fake_run_logged), patch.object(
            self.module, "git_describe", lambda: "30.0.0"
        ):
            libdir = self.module.build_obs("asan", "", "RelWithDebInfo", layout)
        return libdir, calls

    def test_asan_rebuilds_uncached_when_the_zccache_daemon_is_dead(self):
        self.addCleanup(os.environ.pop, "ZCCACHE_DISABLE", None)
        builds_before = len(os.environ)
        libdir, calls = self.drive_build(ZCCACHE_REFUSAL, recover=True)
        self.assertIsNotNone(libdir, "the dead-daemon build must be retried uncached")
        builds = [zccache for command, zccache in calls if command[:2] == ["cmake", "--build"]]
        self.assertEqual(builds, [None, "1"], "one rebuild, with ZCCACHE_DISABLE=1")
        self.assertEqual(len(os.environ), builds_before + 1, "nothing but ZCCACHE_DISABLE is exported")

    def test_a_real_compile_error_is_not_retried(self):
        self.addCleanup(os.environ.pop, "ZCCACHE_DISABLE", None)
        libdir, calls = self.drive_build(
            "FAILED: [code=1] libobs/CMakeFiles/libobs-objects.dir/foo.c.o\n"
            "foo.c:1:1: error: 'x' undeclared\n",
            recover=True,
        )
        self.assertIsNone(libdir)
        builds = [zccache for command, zccache in calls if command[:2] == ["cmake", "--build"]]
        self.assertEqual(builds, [None], "a source error must not be retried")


# A stub test: notes how many tests are running when it starts (the files in
# running/), stays a moment, prints a line and exits 0.
ISOLATION_STUB = """\
r="@RUNNING@"
touch "$r/@NAME@"
echo "@NAME@ $(ls "$r" | wc -l | tr -d ' ')" >> "@CONC@"
sleep 0.2
echo "output of @NAME@"
rm -f "$r/@NAME@"
exit 0
"""


class TestIsolationTest(unittest.TestCase):
    """What a pooled test shares (#3238, after #3251). On the one shared Xvfb
    display, tests side by side broke test_xshm_monitor_late in its setup
    ("cannot add RandR monitors", dispatch 41238464885): each asan test gets
    its own X server, as Tier 2 gives each test its own through xvfb-run.
    tsan and valgrind stay one at a time until they are measured in a pool."""

    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        (self.root / "running").mkdir()
        build = self.root / "build"
        self.layout = self.module.Layout(
            root=self.root, out=self.root / "asan", build=build, inst=build / "rundir", bin_dir=self.root / "bin"
        )

    def stub(self, name: str, serial: bool = False):
        body = (
            ISOLATION_STUB.replace("@RUNNING@", str(self.root / "running"))
            .replace("@CONC@", str(self.root / "concurrency.log"))
            .replace("@NAME@", name)
        )
        path = write_stub(self.root, name, body)
        return self.module.Test(name=name, workdir=str(self.root), command=(str(path),), serial=serial)

    def test_each_test_gets_its_own_x_server(self):
        fake = self.root / "fake-bin"
        fake.mkdir()
        record = self.root / "xvfb-run.log"
        # xvfb-run -n <number> -f <auth file> -s <screen> <command...>
        write_stub(fake, "xvfb-run", f'echo "$1 $2 $3 $4 ${{DISPLAY:-none}}" >> "{record}"\nshift 6\nexec "$@"\n')
        tests = [self.stub(f"test_p{i}") for i in range(4)] + [self.stub("test_serial", serial=True)]
        env = {"PATH": f"{fake}{os.pathsep}{os.environ['PATH']}", "DISPLAY": ":99"}
        with patch.dict(os.environ, env), redirect_stdout(io.StringIO()):
            summary = self.module.run_tests("asan", tests, self.layout, jobs=2, private_displays=True)
        self.assertEqual(summary, [f"test_p{i} exit=0" for i in range(4)] + ["test_serial exit=0"])
        self.assertEqual((self.layout.out / "tests" / "test_p0.log").read_text(encoding="utf-8"), "output of test_p0\n")
        rows = [line.split() for line in record.read_text(encoding="utf-8").splitlines()]
        self.assertEqual(len(rows), 5, "every test, serial ones too, runs under its own xvfb-run")
        self.assertTrue(all(flag == "-n" and auth == "-f" for flag, _, auth, _, _ in rows))
        self.assertEqual(len({number for _, number, _, _, _ in rows}), 5, "two tests got the same server number")
        self.assertEqual({display for *_, display in rows}, {"none"}, "a test inherited the shared DISPLAY")
        # Each server has its own auth file in the build tree, not in
        # xvfb-run's temporary directory, so its cleanup only removes an empty
        # directory: a file left there (xauth's lock files, most likely) made
        # the cleanup fail, and xvfb-run's exit 5 replaced the test's (tsan
        # pool run 41275822723: test_xshm_monitor_move, test_media_cover_art_monitor).
        files = [Path(path) for _, _, _, path, _ in rows]
        self.assertEqual(len(set(files)), 5, "two servers share an auth file")
        self.assertTrue(all(f.parent == self.layout.build / "xauth" for f in files), files)

    def test_private_displays_and_pool_size_per_tool(self):
        self.assertTrue(self.module.private_displays("asan"))
        self.assertFalse(self.module.private_displays("tsan"))
        self.assertFalse(self.module.private_displays("valgrind"))
        with patch.dict(os.environ, {}):
            os.environ.pop("SANITIZE_JOBS", None)
            self.assertEqual(self.module.unit_test_jobs("asan"), os.cpu_count() or 1)
            self.assertEqual(self.module.unit_test_jobs("tsan"), 1)
            self.assertEqual(self.module.unit_test_jobs("valgrind"), 1)
        with patch.dict(os.environ, {"SANITIZE_JOBS": "2"}):
            self.assertEqual(self.module.unit_test_jobs("valgrind"), 2)

    def test_run_unit_tests_uses_the_tool_settings(self):
        seen = {}

        def fake_run_tests(tool, tests, layout, runner=None, jobs=None, private_displays=False):
            seen.update(tool=tool, jobs=jobs, private_displays=private_displays)
            return []

        with patch.object(self.module, "run_tests", fake_run_tests), patch.object(
            self.module.subprocess, "run", return_value=self.module.subprocess.CompletedProcess([], 0, stdout='{"tests": []}')
        ):
            os.environ.pop("SANITIZE_JOBS", None)
            self.module.run_unit_tests("valgrind", self.layout)
        self.assertEqual(seen, {"tool": "valgrind", "jobs": 1, "private_displays": False})


class ParallelUnitTestsTest(unittest.TestCase):
    """Unit tests run concurrently, serial-marked ones alone (#3251)."""

    def setUp(self):
        self.module = load_module()

    def layout(self, root: Path):
        out = root / "out"
        return self.module.Layout(root=root, out=out, build=root / "build", inst=root / "inst", bin_dir=root / "bin")

    def stub_tests(self, root: Path, prefix: str, count: int, serial: bool = False):
        """`count` tests whose commands are real executables, as ctest lists them."""
        bin_dir = root / "stubs"
        bin_dir.mkdir(exist_ok=True)
        return [
            self.module.Test(
                name=f"{prefix}{i}",
                workdir="",
                command=(str(write_stub(bin_dir, f"{prefix}{i}", "exit 0\n")),),
                serial=serial,
            )
            for i in range(count)
        ]

    def test_independent_tests_overlap_and_keep_their_order(self):

        def runner(tool, test, command, layout):
            time.sleep(0.5)
            return 0

        with tempfile.TemporaryDirectory() as tmp:
            tests = self.stub_tests(Path(tmp), "test_", 4)
            start = time.monotonic()
            summary = self.module.run_tests("asan", tests, self.layout(Path(tmp)), runner=runner, jobs=4)
            elapsed = time.monotonic() - start
            durations = (Path(tmp) / "out" / "tests" / "durations.tsv").read_text(encoding="utf-8")
        self.assertLess(elapsed, 1.5, "four 0.5 s tests ran one at a time")
        self.assertEqual(summary, [f"test_{i} exit=0" for i in range(4)])
        self.assertEqual([line.split("\t")[0] for line in durations.splitlines()], [f"test_{i}" for i in range(4)])

    def test_serial_tests_never_overlap(self):
        running, peak = [0], [0]
        lock = threading.Lock()

        def runner(tool, test, command, layout):
            with lock:
                running[0] += 1
                peak[0] = max(peak[0], running[0])
            time.sleep(0.05)
            with lock:
                running[0] -= 1
            return 0

        with tempfile.TemporaryDirectory() as tmp:
            tests = self.stub_tests(Path(tmp), "test_s", 3, serial=True)
            self.module.run_tests("asan", tests, self.layout(Path(tmp)), runner=runner, jobs=4)
        self.assertEqual(peak[0], 1)

    def test_run_serial_and_resource_lock_mark_a_test_serial(self):
        payload = json.dumps({"tests": [
            {"name": "a", "command": ["/a"], "properties": [{"name": "RUN_SERIAL", "value": True}]},
            {"name": "b", "command": ["/b"], "properties": [{"name": "RESOURCE_LOCK", "value": ["x"]}]},
            {"name": "c", "command": ["/c"], "properties": []},
        ]})
        serial = {t.name: t.serial for t in self.module.ctest_tests(payload)}
        self.assertEqual(serial, {"a": True, "b": True, "c": False})


if __name__ == "__main__":
    unittest.main()
