#!/usr/bin/env -S uv run --script
"""The #633 video-init regression harness is Python (obs-rust/obs-studio#962).

Ported off `test/video-init-failure.sh`: the module starts a private `Xvfb`,
runs an ASan/LSan frontend build without an EGL vendor, dismisses the error
dialog, and then asserts the run failed at video initialization *and* leaked
nothing. No real X server or frontend runs here: `Xvfb`, `xdotool` and the app
are stubs on PATH, and the app stub records the environment, the working
directory and the argv the module handed it.

The shell script has no caller in the tree, so this suite drives the module
directly and pins that its wiring -- the shell script gone, its GEN-005
exception gone -- is complete.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "test" / "video_init_failure.py"
SHELL_SCRIPT = ROOT / "test" / "video-init-failure.sh"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "test/video-init-failure.sh"'

XVFB_STUB = """#!/bin/sh
echo 77
while :; do sleep 1; done
"""

XDOTOOL_STUB = """#!/bin/sh
case "$1" in
  search) echo 4242 ;;
  keydown) : ;;
esac
exit 0
"""

# A conforming run: the three log lines the module asserts, exit 0.
PASS_APP = """#!/bin/sh
{
  echo "cwd=$PWD"
  echo "argv=$*"
  env | sort
} > "${XDG_CONFIG_HOME%/*}/capture.txt"
echo "Failed to initialize video"
echo "eglInitialize failed"
echo "Number of memory leaks: 0"
exit __STATUS__
"""


def pass_app(status: int) -> str:
    """A conforming app stub that exits with `status`."""
    return PASS_APP.replace("__STATUS__", str(status))


def load_module():
    """Import the module by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("video_init_failure", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_stub(path: Path, body: str) -> None:
    path.write_text(body)
    path.chmod(0o755)


class Harness:
    """A scratch tree with stub `Xvfb`/`xdotool` and an app stub on PATH."""

    def __init__(self, work: Path) -> None:
        self.work = work
        self.bin = work / "bin"
        self.bin.mkdir(parents=True, exist_ok=True)
        write_stub(self.bin / "Xvfb", XVFB_STUB)
        write_stub(self.bin / "xdotool", XDOTOOL_STUB)
        self.out = work / "output"
        # The app lives in its own directory so the module's `cwd=app.parent`
        # and `dirname(app)/../lib` are both observable.
        self.app = work / "obs-bin" / "obs"
        self.app.parent.mkdir(parents=True, exist_ok=True)
        self.write_app(pass_app(0))

    def write_app(self, body: str) -> None:
        write_stub(self.app, body)

    def env(self) -> dict[str, str]:
        env = dict(os.environ)
        # The stub directory wins over the real PATH; the host PATH is kept
        # behind it so the stubs' own `env`/`sort` still resolve (NixOS has no
        # /usr/bin coreutils).
        env["PATH"] = f"{self.bin}:{os.environ.get('PATH', '')}"
        # The module appends to whatever LD_LIBRARY_PATH it inherits; drop it
        # so the expected value is the shell's bare `dirname(app)/../lib`.
        env.pop("LD_LIBRARY_PATH", None)
        return env

    def run(self, *args: str, env: dict[str, str] | None = None):
        return subprocess.run(
            [sys.executable, str(MODULE), *args],
            env=self.env() if env is None else env,
            capture_output=True,
            text=True,
        )

    def run_pass(self):
        return self.run(str(self.app), str(self.out))

    def capture(self) -> dict[str, str]:
        text = (self.out / "capture.txt").read_text()
        fields: dict[str, str] = {}
        for line in text.splitlines():
            key, _, value = line.partition("=")
            fields[key] = value
        return fields


class VideoInitFailureTest(unittest.TestCase):
    def test_both_arguments_are_required(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            no_args = harness.run()
            self.assertEqual(no_args.returncode, 1)
            self.assertIn("OBS executable required", no_args.stderr)
            one_arg = harness.run(str(harness.app))
            self.assertEqual(one_arg.returncode, 1)
            self.assertIn("output directory required", one_arg.stderr)

    def test_the_pass_run_prints_the_pass_line(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            result = harness.run_pass()
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn(
                "PASS: failed EGL initialization exits with zero OBS allocations",
                result.stdout,
            )

    def test_the_app_runs_from_its_own_directory_with_the_shell_flags(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            harness.run_pass()
            capture = harness.capture()
            self.assertEqual(capture["cwd"], str(harness.app.parent))
            self.assertEqual(
                capture["argv"],
                "--multi --disable-shutdown-check --disable-updater --verbose",
            )

    def test_the_exported_environment_is_unchanged(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            harness.run_pass()
            capture = harness.capture()
            out = harness.out
            self.assertEqual(capture["DISPLAY"], ":77")
            self.assertEqual(capture["QT_QPA_PLATFORM"], "xcb")
            self.assertEqual(capture["XDG_CONFIG_HOME"], str(out / "profile"))
            self.assertEqual(capture["XDG_RUNTIME_DIR"], str(out / "runtime"))
            self.assertEqual(
                capture["__EGL_VENDOR_LIBRARY_FILENAMES"],
                str(out / "no-egl-vendor.json"),
            )
            self.assertEqual(capture["LD_LIBRARY_PATH"], f"{harness.app.parent}/../lib")
            self.assertEqual(
                capture["ASAN_OPTIONS"], f"detect_leaks=1:log_path={out}/asan"
            )
            self.assertEqual(capture["LSAN_OPTIONS"], "report_objects=1:exitcode=0")

    def test_the_runtime_directory_is_private(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            harness.run_pass()
            self.assertTrue((harness.out / "profile").is_dir())
            runtime = harness.out / "runtime"
            self.assertTrue(runtime.is_dir())
            self.assertEqual(runtime.stat().st_mode & 0o777, 0o700)

    def test_exit_status_255_is_accepted_and_others_fail(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            harness.write_app(pass_app(255))
            self.assertEqual(harness.run_pass().returncode, 0)
            harness.write_app(pass_app(7))
            failed = harness.run_pass()
            self.assertEqual(failed.returncode, 1)
            self.assertIn("Failed to initialize video", failed.stdout)

    def test_a_missing_leak_count_fails_with_the_log(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            harness.write_app(
                pass_app(0).replace('echo "Number of memory leaks: 0"\n', "")
            )
            result = harness.run_pass()
            self.assertEqual(result.returncode, 1)
            self.assertIn("leaked an OBS allocation", result.stderr)
            self.assertIn("Failed to initialize video", result.stdout)

    def test_a_missing_video_failure_or_egl_line_fails(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            harness.write_app(
                pass_app(0).replace('echo "Failed to initialize video"\n', "")
            )
            self.assertEqual(harness.run_pass().returncode, 1)
            harness.write_app(pass_app(0).replace('echo "eglInitialize failed"\n', ""))
            self.assertEqual(harness.run_pass().returncode, 1)

    def test_an_lsan_hit_in_the_asan_logs_fails(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work))
            harness.out.mkdir(parents=True, exist_ok=True)
            (harness.out / "asan.999").write_text("OBSBasic::OBSBasic\n")
            result = harness.run_pass()
            self.assertEqual(result.returncode, 1)
            self.assertIn("LSan found a frontend initialization leak", result.stderr)

    def test_wiring_is_complete(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "video-init-failure.sh should be gone")
        self.assertTrue(MODULE.is_file(), "video_init_failure.py should exist")
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(),
            "the GEN-005 exception for video-init-failure.sh should be removed",
        )

    def test_the_module_exposes_the_shells_contract(self):
        module = load_module()
        self.assertEqual(
            module.APP_FLAGS,
            ["--multi", "--disable-shutdown-check", "--disable-updater", "--verbose"],
        )
        self.assertEqual(module.SEARCH_TIMEOUT, 90)
        self.assertTrue(module.EGL_FAILURE.search("Failed to get EGL display"))


if __name__ == "__main__":
    unittest.main()
