#!/usr/bin/env -S uv run --script
"""The private PipeWire-pulse harness is Python (obs-rust/obs-studio#962).

    uv run python -m unittest ci/test_pulse_monitor_device.py

Ported off `test/cmocka/test_pulse_monitor_device.sh`: the module skips (77)
when pipewire, WirePlumber, pipewire-pulse, pactl or dbus-run-session are not
installed, re-executes itself through a private `dbus-run-session`, makes a
runtime directory (moving it to `/tmp` when the daemon's longest socket path
would not fit a Unix socket address), starts pipewire, WirePlumber and
pipewire-pulse with a no-hardware config, waits for the sound server to answer
`pactl` (failing with the daemon logs when one that is installed does not come
up), runs the cmocka test binary and stops the daemons.

No real daemon, sound server or test binary runs here: the five tools are
stubs on PATH and the test binary records its environment and argv.

The shell's callers are the four ctest entries in `test/cmocka/tests.d/*.cmake`
and the `test_pipewire_wrapper.py` wrapper, so this suite drives the module
directly and pins that the wiring -- the shell gone, its GEN-005 exception
gone, the cmake callers and the test graph on `uv run --script` -- is complete.
"""

from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "test" / "cmocka" / "test_pulse_monitor_device.py"
SHELL_SCRIPT = ROOT / "test" / "cmocka" / "test_pulse_monitor_device.sh"
WRAPPER = ROOT / "test" / "cmocka" / "test_pipewire_wrapper.py"
GRAPH = ROOT / "test" / "cmocka" / "build" / "test-graph.json"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "test/cmocka/test_pulse_monitor_device.sh"'
CMAKE_CALLERS = tuple(
    ROOT / "test" / "cmocka" / "tests.d" / name
    for name in (
        "test_pulse_monitor_device.cmake",
        "test_pulse_stream_restart.cmake",
        "test_pulse_two_sources.cmake",
        "test_pulse_default_follow.cmake",
    )
)
GRAPH_COMMAND = [
    "uv",
    "run",
    "--script",
    "${source}/test/cmocka/test_pulse_monitor_device.py",
    "${target}",
]

# A tool that records its argv and environment, so one directory says what each
# stub saw.
RECORD = "import json, os, sys, time\n" + """record = {"argv": sys.argv[1:], "env": dict(os.environ)}
name = os.path.basename(sys.argv[0])
open(os.path.join(os.environ["RECORD"], name + ".json"), "w").write(json.dumps(record))
"""

# A sound-server daemon: it notes the stop signal before recording itself. It
# stays up until then, or dies at once to show what a failed start looks like.
# `DAEMON_BOOT_DELAY` ("name=seconds,...") slows one daemon's startup, as the
# lint gate's concurrency does (obs-rust/obs-studio#2972), and `DAEMON_DIE`
# names daemons that exit on their own right after recording themselves.
DAEMON = "import json, os, signal, sys, time\n" + """
name = os.path.basename(sys.argv[0])


def boot_delay(daemon):
    for entry in os.environ.get("DAEMON_BOOT_DELAY", "").split(","):
        key, _, seconds = entry.partition("=")
        if key == daemon:
            return float(seconds)
    return 0.0


time.sleep(boot_delay(name))


def stop(signum, frame):
    open(os.path.join(os.environ["RECORD"], name + ".terminated"), "w").write(str(signum))
    sys.exit(0)
signal.signal(signal.SIGTERM, stop)
if os.environ.get("DAEMON_MODE") == "die" or name in os.environ.get("DAEMON_DIE", "").split(","):
    print("boom: no socket", flush=True)
    sys.exit(1)
record = {"argv": sys.argv[1:], "env": dict(os.environ)}
open(os.path.join(os.environ["RECORD"], name + ".json"), "w").write(json.dumps(record))
if os.environ.get("DAEMON_MODE") == "hang" or name in os.environ.get("DAEMON_HANG", "").split(","):
    signal.signal(signal.SIGTERM, signal.SIG_IGN)
while True:
    time.sleep(0.05)
"""

# `pactl` answers only once pipewire-pulse has recorded itself, as the real one
# answers only once the private sound server listens.
PACTL = "import os, sys, time\n" + """
if os.environ.get("PACTL_MODE", "ok") != "ok":
    sys.exit(1)
deadline = time.time() + 10
while not os.path.exists(os.path.join(os.environ["RECORD"], "pipewire-pulse.json")):
    if time.time() > deadline:
        sys.exit(1)
    time.sleep(0.01)
"""

# `dbus-run-session` records the command it was given and exits: the harness
# must re-execute itself through it, not start the sound server here.
DBUS = RECORD

# The test binary starts only once the private graph is up (`pactl` answered),
# so it waits for every daemon to have recorded itself. A stub still booting
# when the binary runs is what the load-induced marker flake was (#2972), and
# the wait keeps that case deterministic.
TEST_BINARY = RECORD + """
deadline = time.time() + 10
for daemon in ("pipewire", "wireplumber", "pipewire-pulse"):
    while not os.path.exists(os.path.join(os.environ["RECORD"], daemon + ".json")):
        if time.time() > deadline:
            print("daemon did not start: " + daemon, flush=True)
            sys.exit(9)
        time.sleep(0.01)
try:
    record["config"] = open(
        os.path.join(os.environ["XDG_CONFIG_HOME"], "wireplumber", "wireplumber.conf.d", "90-no-hardware.conf")
    ).read()
except OSError:
    record["config"] = None
open(os.path.join(os.environ["RECORD"], "test-binary.json"), "w").write(json.dumps(record))
sys.exit(int(os.environ.get("TEST_EXIT", "0")))
"""


def load_module():
    """Import the module by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("test_pulse_monitor_device", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Harness:
    """A scratch tree with the five tool stubs and a recording test binary."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.bin = root / "bin"
        self.bin.mkdir(parents=True, exist_ok=True)
        self.record = root / "record"
        self.record.mkdir(parents=True, exist_ok=True)
        for name, body in (
            ("pipewire", DAEMON),
            ("wireplumber", DAEMON),
            ("pipewire-pulse", DAEMON),
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

    def recorded(self, name: str) -> dict:
        return json.loads((self.record / name).read_text(encoding="utf-8"))

    def env(self, **overrides: str) -> dict[str, str]:
        environment = {key: value for key, value in os.environ.items() if key not in ("PRIVATE_PIPEWIRE_STARTED", "VIRTUAL_ENV")}
        environment.update({"RECORD": str(self.record), "TMPDIR": str(self.root)})
        environment.update(overrides)
        return environment

    def run(self, *args: str, path: str | None = None, **overrides: str):
        environment = self.env(**overrides)
        environment["PATH"] = f"{self.bin}{os.pathsep}{environment['PATH']}" if path is None else path
        return subprocess.run(
            [sys.executable, str(MODULE), *args],
            capture_output=True,
            text=True,
            env=environment,
            timeout=60,
        )

    def private_dirs(self) -> list[Path]:
        return list(self.root.glob("obs-pw-test.*"))


class PulseMonitorDeviceTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.harness = Harness(Path(self.tmp.name))

    def test_a_missing_tool_skips_the_test(self) -> None:
        (self.harness.bin / "pactl").unlink()
        result = self.harness.run("test-pulse", path=str(self.harness.bin))
        self.assertEqual(result.returncode, 77, result.stdout + result.stderr)
        self.assertIn("pactl not found: skipping", result.stdout)
        self.assertFalse((self.harness.record / "test-binary.json").exists(), "the test binary must not run")

    def test_no_arguments_is_a_usage_error(self) -> None:
        result = self.harness.run(PRIVATE_PIPEWIRE_STARTED="1")
        self.assertEqual(result.returncode, 2)
        self.assertIn("usage: test_pulse_monitor_device.py <test binary>", result.stderr)

    def test_the_harness_runs_itself_inside_a_private_dbus_session(self) -> None:
        result = self.harness.run("test-pulse")
        self.assertEqual(result.returncode, 0, result.stderr)
        # dbus-run-session -- <interpreter> <this script> <test binary>
        argv = self.harness.recorded("dbus-run-session.json")["argv"]
        self.assertEqual(argv[0], "--")
        self.assertTrue(argv[1], "the harness re-executes through its interpreter")
        self.assertEqual(Path(argv[2]).name, "test_pulse_monitor_device.py")
        self.assertEqual(argv[3:], ["test-pulse"])
        env = self.harness.recorded("dbus-run-session.json")["env"]
        self.assertEqual(env["PRIVATE_PIPEWIRE_STARTED"], "1")
        # Nothing but the re-exec happened: the sound server is the child's.
        self.assertFalse((self.harness.record / "pipewire.json").exists())

    def test_the_sound_server_serves_the_test_binary(self) -> None:
        result = self.harness.run("test-pulse", "--alpha", "beta", PRIVATE_PIPEWIRE_STARTED="1", TEST_EXIT="3")
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        pipewire = self.harness.recorded("pipewire.json")
        runtime = pipewire["env"]["XDG_RUNTIME_DIR"]
        self.assertTrue(runtime.startswith(str(self.harness.root) + os.sep), runtime)
        self.assertEqual(pipewire["argv"], [])
        self.assertEqual(pipewire["env"]["PIPEWIRE_RUNTIME_DIR"], runtime)
        self.assertEqual(pipewire["env"]["PULSE_RUNTIME_PATH"], f"{runtime}/pulse")
        self.assertEqual(pipewire["env"]["XDG_CONFIG_HOME"], f"{runtime}/config")
        self.assertEqual(pipewire["env"]["XDG_STATE_HOME"], f"{runtime}/state")
        self.assertEqual(pipewire["env"]["HOME"], f"{runtime}/home")
        test = self.harness.recorded("test-binary.json")
        self.assertEqual(test["argv"], ["--alpha", "beta"])
        self.assertEqual(test["env"]["PULSE_SERVER"], f"unix:{runtime}/pulse/native")
        self.assertIn("monitor.bluez-midi = disabled", test["config"])
        self.assertIn("monitor.libcamera = disabled", test["config"])
        # Every daemon is stopped and the private directory removed afterwards.
        for name in ("pipewire", "wireplumber", "pipewire-pulse"):
            self.assertTrue((self.harness.record / f"{name}.terminated").exists(), name)
        self.assertEqual(self.harness.private_dirs(), [])

    def test_a_daemon_still_starting_when_the_test_runs_is_stopped_cleanly(self) -> None:
        """A slow daemon boot must not leave the stop unobservable (#2972).

        Under `./lint`'s concurrency, WirePlumber can still be installing its
        handlers when the harness stops it; a SIGTERM that lands before that is
        handled by the default action and writes no marker."""
        result = self.harness.run("test-pulse", PRIVATE_PIPEWIRE_STARTED="1", DAEMON_BOOT_DELAY="wireplumber=3")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        for name in ("pipewire", "wireplumber", "pipewire-pulse"):
            self.assertTrue((self.harness.record / f"{name}.terminated").exists(), name)

    def test_a_daemon_that_exits_on_its_own_fails_the_test_with_its_log(self) -> None:
        """A daemon gone before the test binary runs is a failure, not a pass."""
        result = self.harness.run("test-pulse", PRIVATE_PIPEWIRE_STARTED="1", DAEMON_DIE="wireplumber", TEST_EXIT="3")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("wireplumber exited on its own:", result.stdout)
        self.assertIn("boom: no socket", result.stdout)
        self.assertFalse((self.harness.record / "test-binary.json").exists(), "the test binary must not run")

    def test_a_daemon_that_ignores_sigterm_is_killed(self) -> None:
        """A hung daemon is killed after the stop deadline; it cannot hang the lane."""
        result = self.harness.run("test-pulse", PRIVATE_PIPEWIRE_STARTED="1", DAEMON_HANG="wireplumber", TEST_EXIT="3")
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        self.assertEqual(self.harness.private_dirs(), [])

    def test_a_sound_server_that_does_not_start_fails_the_test(self) -> None:
        result = self.harness.run("test-pulse", PRIVATE_PIPEWIRE_STARTED="1", DAEMON_MODE="die", PACTL_MODE="fail")
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("private pipewire-pulse did not start:", result.stdout)
        self.assertIn("boom: no socket", result.stdout)
        self.assertFalse((self.harness.record / "test-binary.json").exists(), "the test binary must not run")
        self.assertEqual(self.harness.private_dirs(), [])

    def test_a_deep_tmpdir_moves_the_private_dir_to_tmp(self) -> None:
        deep = self.harness.root / ("d" * 80)
        deep.mkdir(parents=True)
        result = self.harness.run("test-pulse", PRIVATE_PIPEWIRE_STARTED="1", TMPDIR=str(deep))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        runtime = self.harness.recorded("pipewire.json")["env"]["XDG_RUNTIME_DIR"]
        self.assertTrue(runtime.startswith("/tmp/obs-pw-test."), runtime)
        self.assertLessEqual(len(runtime) + len("/pipewire-0-manager"), 107)

    def test_wiring_is_complete(self) -> None:
        self.assertFalse(SHELL_SCRIPT.exists(), "test_pulse_monitor_device.sh should be gone")
        self.assertTrue(MODULE.is_file(), "test_pulse_monitor_device.py should exist")
        for caller in CMAKE_CALLERS:
            text = caller.read_text(encoding="utf-8")
            self.assertIn("uv run --script", text, caller.name)
            self.assertIn("test_pulse_monitor_device.py", text, caller.name)
            self.assertNotIn("test_pulse_monitor_device.sh", text, caller.name)
        graph = json.loads(GRAPH.read_text(encoding="utf-8"))
        self.assertEqual(graph["tests"]["test_pulse_monitor_samples"]["command"], GRAPH_COMMAND)
        wrapper = WRAPPER.read_text(encoding="utf-8")
        self.assertIn("test_pulse_monitor_device.py", wrapper)
        self.assertNotIn("test_pulse_monitor_device.sh", wrapper)
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(encoding="utf-8"),
            "the GEN-005 exception for test_pulse_monitor_device.sh should be removed",
        )

    def test_the_module_exposes_the_shells_contract(self) -> None:
        module = load_module()
        self.assertEqual(module.SOCKET_BUDGET, 107)
        self.assertEqual(module.SOCKET_SUFFIX, "/pipewire-0-manager")
        self.assertEqual(module.START_ATTEMPTS, 200)
        self.assertEqual(module.START_SLEEP, 0.1)
        self.assertEqual(
            module.TOOLS,
            ("pipewire", "wireplumber", "pipewire-pulse", "pactl", "dbus-run-session"),
        )


if __name__ == "__main__":
    unittest.main()
