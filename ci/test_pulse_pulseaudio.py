#!/usr/bin/env -S uv run --script
"""The PulseAudio end-to-end harness runs a private daemon from Python.

    PYTHONPATH=ci uv run --no-project python -m unittest test_pulse_pulseaudio

test/cmocka/test_pulse_pulseaudio.py gives the cmocka PulseAudio tests their
own daemon: a private runtime dir, HOME and D-Bus, and no hardware, session or
Bluetooth modules. These tests stub pulseaudio, pactl, dbus-run-session and the
test binary, so no sound server starts.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "test" / "cmocka" / "test_pulse_pulseaudio.py"
CMAKE = ROOT / "test" / "cmocka" / "tests.d" / "test_pulse_default_follow.cmake"

# Every stub records its argv and environment, so one file says what it saw.
RECORD = """import json, os, sys
record = {"argv": sys.argv[1:], "env": dict(os.environ)}
open(os.path.join(os.environ["RECORD"], os.path.basename(sys.argv[0]) + ".json"), "w").write(json.dumps(record))
"""

# The daemon takes its stop signal before it records itself, so a harness that
# stops it as soon as `pactl` answers cannot race its startup. It stays up
# until then, or dies at once to show what a failed start looks like.
DAEMON = "import json, os, signal, sys, time\n" + """
def stop(signum, frame):
    open(os.path.join(os.environ["RECORD"], "terminated"), "w").write(str(signum))
    sys.exit(0)
signal.signal(signal.SIGTERM, stop)
record = {"argv": sys.argv[1:], "env": dict(os.environ)}
open(os.path.join(os.environ["RECORD"], "pulseaudio.json"), "w").write(json.dumps(record))
if os.environ.get("DAEMON_MODE") == "die":
    print("boom: no socket", flush=True)
    sys.exit(1)
while True:
    time.sleep(0.05)
"""

# `pactl` answers only once the daemon has recorded itself, as the real one
# answers only once the daemon listens.
PACTL = "import json, os, sys, time\n" + """
if os.environ.get("PACTL_MODE", "ok") != "ok":
    sys.exit(1)
deadline = time.time() + 10
while not os.path.exists(os.path.join(os.environ["RECORD"], "pulseaudio.json")):
    if time.time() > deadline:
        sys.exit(1)
    time.sleep(0.01)
"""

DBUS = RECORD

TEST_BINARY = RECORD + """
try:
    record["config"] = open(os.path.join(os.environ["XDG_RUNTIME_DIR"], "test.pa")).read()
except OSError:
    record["config"] = None
open(os.path.join(os.environ["RECORD"], "test-binary.json"), "w").write(json.dumps(record))
sys.exit(int(os.environ.get("TEST_EXIT", "0")))
"""


class PulseAudioTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.record = self.root / "record"
        self.record.mkdir()
        for name, body in (
            ("pulseaudio", DAEMON),
            ("pactl", PACTL),
            ("dbus-run-session", DBUS),
            ("test-pulse", TEST_BINARY),
        ):
            self.stub(name, body)

    def stub(self, name: str, body: str) -> Path:
        path = self.bin / name
        path.write_text(f"#!{sys.executable}\n{body}")
        path.chmod(0o755)
        return path

    def run_script(self, *args: str, **env: str) -> subprocess.CompletedProcess:
        environment = {k: v for k, v in os.environ.items() if k not in ("PRIVATE_PULSEAUDIO_STARTED", "VIRTUAL_ENV")}
        environment.update(
            {"PATH": f"{self.bin}{os.pathsep}{environment['PATH']}", "RECORD": str(self.record), "TMPDIR": str(self.root)}
        )
        environment.update(env)
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args], capture_output=True, text=True, env=environment, timeout=60
        )

    def recorded(self, name: str) -> dict:
        return json.loads((self.record / name).read_text(encoding="utf-8"))

    def assert_private_dir_removed(self) -> None:
        self.assertEqual(list(self.root.glob("obs-pa-test.*")), [])

    def test_a_missing_tool_skips_the_test(self) -> None:
        (self.bin / "pactl").unlink()
        result = self.run_script("test-pulse", PATH=str(self.bin))
        self.assertEqual(result.returncode, 77)
        self.assertIn("pactl not found: skipping (needs a PulseAudio daemon, not pipewire-pulse)", result.stdout)
        self.assertFalse((self.record / "test-binary.json").exists())

    def test_the_harness_runs_itself_inside_a_private_dbus_session(self) -> None:
        result = self.run_script("test-pulse", OBS_TEST_PULSEAUDIO=str(self.bin / "pulseaudio"))
        self.assertEqual(result.returncode, 0, result.stderr)
        # dbus-run-session -- <interpreter> <this script> <test binary>
        argv = self.recorded("dbus-run-session.json")["argv"]
        self.assertEqual(argv[0], "--")
        self.assertTrue(argv[1], "the harness re-executes through its interpreter")
        self.assertEqual(Path(argv[2]).name, "test_pulse_pulseaudio.py")
        self.assertEqual(argv[3:], ["test-pulse"])
        env = self.recorded("dbus-run-session.json")["env"]
        self.assertEqual(env["PRIVATE_PULSEAUDIO_STARTED"], "1")
        self.assertEqual(env["OBS_TEST_PULSEAUDIO"], str(self.bin / "pulseaudio"))
        # Nothing but the re-exec happened: the daemon was left to the child.
        self.assertFalse((self.record / "pulseaudio.json").exists())

    def test_the_daemon_serves_the_test_binary(self) -> None:
        result = self.run_script("test-pulse", "--alpha", "beta", PRIVATE_PULSEAUDIO_STARTED="1", TEST_EXIT="3")
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        daemon = self.recorded("pulseaudio.json")
        runtime = daemon["env"]["XDG_RUNTIME_DIR"]
        self.assertTrue(runtime.startswith(str(self.root) + os.sep), runtime)
        self.assertEqual(
            daemon["argv"],
            [
                "-n",
                "-F",
                f"{runtime}/test.pa",
                "--daemonize=no",
                "--exit-idle-time=-1",
                "--use-pid-file=no",
                "--system=no",
                "--disallow-exit",
                "--log-target=stderr",
            ],
        )
        test = self.recorded("test-binary.json")
        self.assertEqual(test["argv"], ["--alpha", "beta"])
        self.assertEqual(test["env"]["PULSE_SERVER"], f"unix:{runtime}/pulse/native")
        self.assertEqual(test["env"]["PULSE_RUNTIME_PATH"], f"{runtime}/pulse")
        self.assertEqual(test["env"]["PULSE_STATE_PATH"], f"{runtime}/state")
        self.assertEqual(test["env"]["XDG_CONFIG_HOME"], f"{runtime}/config")
        self.assertEqual(test["env"]["XDG_STATE_HOME"], f"{runtime}/state")
        self.assertEqual(test["env"]["HOME"], f"{runtime}/home")
        self.assertEqual(test["config"], "load-module module-native-protocol-unix\n")
        # The private daemon is stopped and its directory removed afterwards.
        self.assertTrue((self.record / "terminated").exists())
        self.assert_private_dir_removed()

    def test_a_daemon_that_does_not_start_fails_the_test(self) -> None:
        result = self.run_script("test-pulse", PRIVATE_PULSEAUDIO_STARTED="1", DAEMON_MODE="die", PACTL_MODE="fail")
        self.assertEqual(result.returncode, 1)
        self.assertIn("private pulseaudio did not start:", result.stdout)
        self.assertIn("boom: no socket", result.stdout)
        self.assertFalse((self.record / "test-binary.json").exists())
        self.assert_private_dir_removed()

    def test_a_daemon_that_is_still_starting_is_waited_for(self) -> None:
        # `pactl` answering is the harness's signal that the daemon serves the
        # test binary, as the real one does only once it listens; a stub that
        # answers before the daemon has even started lets the harness stop it
        # mid-startup, and its record never appears (seen as a
        # FileNotFoundError in ./lint's parallel self-test).
        self.stub("pulseaudio", "import time\ntime.sleep(1.0)\n" + DAEMON)
        result = self.run_script("test-pulse", PRIVATE_PULSEAUDIO_STARTED="1")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue((self.record / "pulseaudio.json").exists(), "the daemon started up")
        self.assertTrue((self.record / "test-binary.json").exists())

    def test_a_deep_tmpdir_moves_the_private_dir_to_tmp(self) -> None:
        deep = self.root / ("d" * 80)
        deep.mkdir(parents=True)
        result = self.run_script("test-pulse", PRIVATE_PULSEAUDIO_STARTED="1", TMPDIR=str(deep))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        runtime = self.recorded("pulseaudio.json")["env"]["XDG_RUNTIME_DIR"]
        self.assertTrue(runtime.startswith("/tmp/obs-pa-test."), runtime)
        self.assertLessEqual(len(runtime) + 13, 107)

    def test_the_cmake_runs_the_python_script(self) -> None:
        text = CMAKE.read_text(encoding="utf-8")
        self.assertIn("test_pulse_pulseaudio.py", text)
        self.assertNotIn("test_pulse_pulseaudio.sh", text)


if __name__ == "__main__":
    unittest.main()
