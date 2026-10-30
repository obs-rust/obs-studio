#!/usr/bin/env -S uv run --script
"""The PipeWire context-failure harness is Python (obs-rust/obs-studio#962).

Ported off `test/cmocka/test_pipewire_context_fail.sh`: the module unsets the
display/DBus/PipeWire variables, makes a private runtime directory (moving it to
`/tmp` when the daemon's longest socket path would not fit a Unix socket
address), starts the minimal private PipeWire daemon when one is installed,
waits for its socket, and then runs the cmocka test binary, failing with the
daemon's log when a daemon that is installed does not come up.

No real daemon and no real test binary run here: `pipewire` is a stub on PATH
and the test binary is a stub that records the environment, the working
directory and the argv the module handed it.

The shell's only caller is
`test/cmocka/tests.d/test_pipewire_context_fail.cmake`, so this suite drives the
module directly and pins that the wiring -- the shell script gone, its GEN-005
exception gone, the cmake caller running the Python through `uv run` -- is
complete.
"""

from __future__ import annotations

import importlib.util
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "test" / "cmocka" / "test_pipewire_context_fail.py"
SHELL_SCRIPT = ROOT / "test" / "cmocka" / "test_pipewire_context_fail.sh"
CMAKE = ROOT / "test" / "cmocka" / "tests.d" / "test_pipewire_context_fail.cmake"
WRAPPER_TEST = ROOT / "test" / "cmocka" / "test_pipewire_wrapper.py"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "test/cmocka/test_pipewire_context_fail.sh"'

# A pipewire that binds the daemon socket and stays up. The module requires a
# socket (-S in the shell), so the stub makes a real one.
LIVE_PIPEWIRE = """#!/usr/bin/env -S uv run --script
import os
import socket
import time

# The interpreter's own pid, behind the `uv` wrapper: the leak check reads it.
with open(os.environ["CAPTURE_FILE"] + ".daemon-pid", "w") as pid_file:
    pid_file.write(str(os.getpid()))
path = os.path.join(os.environ["PIPEWIRE_RUNTIME_DIR"], "pipewire-0")
server = socket.socket(socket.AF_UNIX)
server.bind(path)
server.listen()
while True:
    time.sleep(0.05)
"""

# A pipewire that exits at once, as one that cannot open its socket does.
DYING_PIPEWIRE = """#!/bin/sh
echo "fake pipewire: cannot start" >&2
exit 1
"""

CAPTURE_BIN = """#!/bin/sh
{
  echo "cwd=$PWD"
  echo "argv=$*"
  env | sort
} > "$CAPTURE_FILE"
exit __STATUS__
"""

# What the no-pipewire control needs on PATH (the module itself needs nothing:
# it is already running, and the test binary is called by absolute path).
SHELL_TOOLS = ("sh", "env", "sort")
SOCKET_SUFFIX = "/run/pipewire-0-manager"


def capture_bin(status: int) -> str:
    """A test-binary stub that records its run and exits with `status`."""
    return CAPTURE_BIN.replace("__STATUS__", str(status))


def load_module():
    """Import the module by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("test_pipewire_context_fail", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def process_alive(pid: int) -> bool:
    """True while pid runs (a zombie awaiting its new parent counts as gone)."""
    try:
        state = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()[0]
    except (FileNotFoundError, IndexError):
        return False
    return state != "Z"


def write_stub(path: Path, body: str) -> None:
    path.write_text(body)
    path.chmod(0o755)


class Harness:
    """A scratch tree with a stub `pipewire`, a stub test binary and a capture file."""

    def __init__(self, work: Path, pipewire: str | None) -> None:
        self.work = work
        self.bin = work / "bin"
        self.bin.mkdir(parents=True, exist_ok=True)
        # True once shell_tools_only() has made PATH the stub directory alone.
        self.restricted = False
        if pipewire is not None:
            write_stub(self.bin / "pipewire", pipewire)
        self.capture_file = work / "capture.txt"
        self.test_bin = work / "test_pipewire_context_fail"
        self.write_test_bin(capture_bin(0))

    def write_test_bin(self, body: str) -> None:
        write_stub(self.test_bin, body)

    def shell_tools_only(self) -> None:
        """A PATH with the shell tools but no pipewire (the host PATH may have one)."""
        only = self.work / "bin-none"
        only.mkdir(parents=True, exist_ok=True)
        for tool in SHELL_TOOLS:
            found = which(tool)
            assert found is not None, tool
            link = only / tool
            if not link.exists():
                link.symlink_to(found)
        self.bin = only
        self.restricted = True

    def env(self, tmpdir: Path | None = None) -> dict[str, str]:
        env = dict(os.environ)
        # The stub directory wins over the real PATH, so the stub pipewire (and
        # no real one) is what the module finds; the host PATH stays behind it
        # so the stubs' own `sh`, `env` and `sort` still resolve (NixOS has no
        # /usr/bin coreutils, and `uv` runs the live stub). With no pipewire
        # stub at all, PATH is the tools directory alone: a host pipewire
        # (CI's Tier 2 container has one) must not be found either.
        env["PATH"] = str(self.bin) if self.restricted else f"{self.bin}:{os.environ.get('PATH', '')}"
        env["CAPTURE_FILE"] = str(self.capture_file)
        env.pop("PIPEWIRE_RUNTIME_DIR", None)
        env.pop("PIPEWIRE_CONFIG_DIR", None)
        if tmpdir is not None:
            env["TMPDIR"] = str(tmpdir)
        return env

    def run(self, *args: str, env: dict[str, str] | None = None):
        return subprocess.run(
            [sys.executable, str(MODULE), *args],
            env=self.env() if env is None else env,
            capture_output=True,
            text=True,
            timeout=120,
        )

    def run_pass(self, env: dict[str, str] | None = None):
        return self.run(str(self.test_bin), env=env)

    def capture(self) -> dict[str, str]:
        fields: dict[str, str] = {}
        for line in self.capture_file.read_text().splitlines():
            key, _, value = line.partition("=")
            fields[key] = value
        return fields


def which(tool: str) -> str | None:
    """shutil.which without importing shutil into the stub's own PATH handling."""
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        candidate = Path(directory) / tool
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


class PipewireContextFailTest(unittest.TestCase):
    def test_the_usage_error_names_the_argument(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            no_args = harness.run()
            self.assertEqual(no_args.returncode, 2)
            self.assertIn("usage: test_pipewire_context_fail.py <test binary>", no_args.stderr)

    def test_a_missing_pipewire_skips_the_control_case_and_passes_the_status_through(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), None)
            harness.shell_tools_only()
            result = harness.run_pass()
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("pipewire not found; the control case will skip", result.stdout)
            # PIPEWIRE_RUNTIME_DIR is exported only when the daemon started.
            self.assertNotIn("PIPEWIRE_RUNTIME_DIR", harness.capture())

    def test_the_test_binary_is_run_without_arguments_from_the_inherited_cwd(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), None)
            harness.shell_tools_only()
            harness.run_pass()
            capture = harness.capture()
            self.assertEqual(capture["argv"], "")
            self.assertEqual(capture["cwd"], str(Path.cwd()))

    def test_the_exported_environment_is_the_shells(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            self.assertEqual(harness.run_pass().returncode, 0)
            capture = harness.capture()
            runtime = Path(capture["XDG_RUNTIME_DIR"])
            self.assertEqual(runtime.name, "run")
            self.assertEqual(capture["XDG_CONFIG_HOME"], str(runtime.parent / "config"))
            self.assertEqual(capture["XDG_STATE_HOME"], str(runtime.parent / "state"))
            self.assertEqual(capture["HOME"], str(runtime.parent / "home"))
            # The daemon started, so the test binary sees its runtime directory.
            self.assertEqual(capture["PIPEWIRE_RUNTIME_DIR"], str(runtime))
            for name in (
                "DISPLAY",
                "WAYLAND_DISPLAY",
                "HYPRLAND_INSTANCE_SIGNATURE",
                "DBUS_SESSION_BUS_ADDRESS",
                "DBUS_SYSTEM_BUS_ADDRESS",
                "PIPEWIRE_REMOTE",
                # PIPEWIRE_CONFIG_DIR is the daemon's alone: the shell gave it
                # as a prefix on the `pipewire` command, not an export.
                "PIPEWIRE_CONFIG_DIR",
                "PIPEWIRE_CONFIG_NAME",
            ):
                self.assertNotIn(name, capture, name)

    def test_the_runtime_directory_is_removed_afterwards(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            self.assertEqual(harness.run_pass().returncode, 0)
            runtime = Path(harness.capture()["XDG_RUNTIME_DIR"])
            self.assertFalse(runtime.parent.exists(), "the runtime directory should be gone")

    def test_the_daemon_behind_a_wrapper_does_not_outlive_the_run(self):
        # The live stub runs under `uv run`, so the module's direct child is
        # uv and the daemon is its child. Killing only uv leaked the daemon on
        # every run (obs-rust/obs-studio#2984).
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            self.assertEqual(harness.run_pass().returncode, 0)
            pid = int(Path(str(harness.capture_file) + ".daemon-pid").read_text())
            deadline = time.monotonic() + 5
            while process_alive(pid) and time.monotonic() < deadline:
                time.sleep(0.05)
            self.assertFalse(process_alive(pid), f"stub pipewire {pid} outlived the run")

    def test_a_deep_tmpdir_falls_back_to_tmp(self):
        with tempfile.TemporaryDirectory() as work:
            deep = Path(work)
            while len(str(deep)) < 90:
                deep = deep / "a-build-tree-in-a-deeply-nested-worktree-directory"
            deep.mkdir(parents=True, exist_ok=True)
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            self.assertEqual(harness.run_pass(env=harness.env(tmpdir=deep)).returncode, 0)
            runtime = Path(harness.capture()["XDG_RUNTIME_DIR"])
            self.assertTrue(str(runtime).startswith("/tmp/"), runtime)
            self.assertLessEqual(len(str(runtime)) + len(SOCKET_SUFFIX), 107)

    def test_a_daemon_that_does_not_come_up_fails_with_its_log(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), DYING_PIPEWIRE)
            result = harness.run_pass()
            self.assertEqual(result.returncode, 1)
            self.assertIn("private pipewire did not start:", result.stdout)
            self.assertIn("fake pipewire: cannot start", result.stdout)
            self.assertFalse(harness.capture_file.exists(), "the test binary must not run")

    def test_the_test_status_is_propagated(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), None)
            harness.shell_tools_only()
            harness.write_test_bin(capture_bin(3))
            self.assertEqual(harness.run_pass().returncode, 3)

    def test_wiring_is_complete(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "test_pipewire_context_fail.sh should be gone")
        self.assertTrue(MODULE.is_file(), "test_pipewire_context_fail.py should exist")
        caller = CMAKE.read_text()
        self.assertIn("uv run --script", caller)
        self.assertNotIn("test_pipewire_context_fail.sh", caller)
        # The sibling wrapper test used to run this shell script as a control;
        # it must run the Python module instead, or it fails on a machine with
        # pipewire installed (CI has none, so PR CI does not catch it).
        wrapper_test = WRAPPER_TEST.read_text()
        self.assertNotIn("test_pipewire_context_fail.sh", wrapper_test)
        self.assertIn("test_pipewire_context_fail.py", wrapper_test)
        self.assertIn("uv run --script", wrapper_test)
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(),
            "the GEN-005 exception for test_pipewire_context_fail.sh should be removed",
        )

    def test_the_module_exposes_the_shells_contract(self):
        module = load_module()
        self.assertEqual(module.SOCKET_BUDGET, 107)
        self.assertEqual(module.SOCKET_SUFFIX, len(SOCKET_SUFFIX))
        self.assertEqual(module.START_ATTEMPTS, 200)
        self.assertEqual(module.START_SLEEP, 0.1)
        self.assertEqual(
            set(module.UNSET),
            {
                "DISPLAY",
                "WAYLAND_DISPLAY",
                "HYPRLAND_INSTANCE_SIGNATURE",
                "DBUS_SESSION_BUS_ADDRESS",
                "DBUS_SYSTEM_BUS_ADDRESS",
                "PIPEWIRE_REMOTE",
                "PIPEWIRE_RUNTIME_DIR",
                "PIPEWIRE_CONFIG_DIR",
                "PIPEWIRE_CONFIG_NAME",
            },
        )
        config = module.DAEMON_CONFIG
        self.assertIn("libpipewire-module-protocol-native", config)
        self.assertIn("libpipewire-module-access", config)
        self.assertIn("support.dbus = false", config)


if __name__ == "__main__":
    unittest.main()
