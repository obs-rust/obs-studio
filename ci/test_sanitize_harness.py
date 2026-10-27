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


if __name__ == "__main__":
    unittest.main()
