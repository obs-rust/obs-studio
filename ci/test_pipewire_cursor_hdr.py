#!/usr/bin/env -S uv run --script
"""The private-PipeWire-dir harness is Python (obs-rust/obs-studio#962).

Ported off `test/cmocka/test_pipewire_cursor_hdr.sh`: the module unsets the
session bus and `PIPEWIRE_REMOTE`, skips (77) when pipewire is not installed,
makes a private runtime directory (moving it to `/tmp` when the daemon's
longest socket path would not fit a Unix socket address), starts a pipewire
daemon with that directory as its runtime dir, waits for its socket (failing
with the daemon's log when one that is installed does not come up), then runs
the cmocka test binary and propagates its status.

No real daemon and no real test binary run here: `pipewire` is a stub on PATH
and the test binary is a stub that records the environment, the working
directory and the argv the module handed it.

The shell's callers are the `test_pipewire_camera_*` / `test_pipewire_screencast_*`
ctest entries (all through `test/cmocka/tests.d/*.cmake`) plus the wrapper
`test_pipewire_wrapper.sh`, so this suite drives the module directly and pins
that the wiring -- the shell script gone, its GEN-005 exception gone, the cmake
callers running the Python through `uv run --script` -- is complete.
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
MODULE = ROOT / "test" / "cmocka" / "test_pipewire_cursor_hdr.py"
SHELL_SCRIPT = ROOT / "test" / "cmocka" / "test_pipewire_cursor_hdr.sh"
CMAKE = ROOT / "test" / "cmocka" / "tests.d" / "test_pipewire_cursor_hdr.cmake"
CI_TOML = ROOT / "ci.toml"
EXCEPTION = 'path = "test/cmocka/test_pipewire_cursor_hdr.sh"'

# A pipewire that binds the daemon socket and stays up. The module requires a
# socket (-S in the shell), so the stub makes a real one.
LIVE_PIPEWIRE = """#!/usr/bin/env -S uv run --script
import os
import socket
import time

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
SOCKET_SUFFIX = "/pipewire-0-manager"


def capture_bin(status: int) -> str:
    """A test-binary stub that records its run and exits with `status`."""
    return CAPTURE_BIN.replace("__STATUS__", str(status))


def load_module():
    """Import the module by path (it is a script, not an installed module)."""
    spec = importlib.util.spec_from_file_location("test_pipewire_cursor_hdr", MODULE)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write_stub(path: Path, body: str) -> None:
    path.write_text(body)
    path.chmod(0o755)


def which(tool: str) -> str | None:
    """shutil.which without importing shutil into the stub's own PATH handling."""
    for directory in os.environ.get("PATH", "").split(os.pathsep):
        candidate = Path(directory) / tool
        if candidate.is_file() and os.access(candidate, os.X_OK):
            return str(candidate)
    return None


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
        self.test_bin = work / "test_pipewire_cursor_hdr"
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
        env.pop("PIPEWIRE_REMOTE", None)
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


class PipewireCursorHdrTest(unittest.TestCase):
    def test_the_usage_error_names_the_argument(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            no_args = harness.run()
            self.assertEqual(no_args.returncode, 2)
            self.assertIn("usage: test_pipewire_cursor_hdr.py <test binary>", no_args.stderr)

    def test_a_missing_pipewire_skips_and_the_test_binary_does_not_run(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), None)
            harness.shell_tools_only()
            result = harness.run_pass()
            self.assertEqual(result.returncode, 77, result.stdout + result.stderr)
            self.assertIn("pipewire not found: skipping", result.stdout)
            self.assertFalse(harness.capture_file.exists(), "the test binary must not run")

    def test_the_test_binary_is_run_without_arguments_from_the_inherited_cwd(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            self.assertEqual(harness.run_pass().returncode, 0)
            capture = harness.capture()
            self.assertEqual(capture["argv"], "")
            self.assertEqual(capture["cwd"], str(Path.cwd()))

    def test_the_exported_environment_is_the_shells(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            self.assertEqual(harness.run_pass().returncode, 0)
            capture = harness.capture()
            runtime = Path(capture["XDG_RUNTIME_DIR"])
            # The shell gives the daemon the runtime directory itself, not a
            # `run/` child (that is the context-fail harness).
            self.assertEqual(capture["PIPEWIRE_RUNTIME_DIR"], str(runtime))
            self.assertEqual(capture["XDG_CONFIG_HOME"], str(runtime / "config"))
            self.assertEqual(capture["XDG_STATE_HOME"], str(runtime / "state"))
            self.assertEqual(capture["HOME"], str(runtime / "home"))
            for name in ("DBUS_SESSION_BUS_ADDRESS", "PIPEWIRE_REMOTE"):
                self.assertNotIn(name, capture, name)

    def test_the_runtime_directory_is_removed_afterwards(self):
        with tempfile.TemporaryDirectory() as work:
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            self.assertEqual(harness.run_pass().returncode, 0)
            runtime = Path(harness.capture()["XDG_RUNTIME_DIR"])
            self.assertFalse(runtime.exists(), "the runtime directory should be gone")

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
            harness = Harness(Path(work), LIVE_PIPEWIRE)
            harness.write_test_bin(capture_bin(3))
            self.assertEqual(harness.run_pass().returncode, 3)

    def test_wiring_is_complete(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "test_pipewire_cursor_hdr.sh should be gone")
        self.assertTrue(MODULE.is_file(), "test_pipewire_cursor_hdr.py should exist")
        caller = CMAKE.read_text()
        self.assertIn("uv run --script", caller)
        self.assertNotIn("test_pipewire_cursor_hdr.sh", caller)
        self.assertNotIn(
            EXCEPTION,
            CI_TOML.read_text(),
            "the GEN-005 exception for test_pipewire_cursor_hdr.sh should be removed",
        )

    def test_the_module_exposes_the_shells_contract(self):
        module = load_module()
        self.assertEqual(module.SOCKET_BUDGET, 107)
        self.assertEqual(module.SOCKET_SUFFIX, len(SOCKET_SUFFIX))
        self.assertEqual(module.START_ATTEMPTS, 200)
        self.assertEqual(module.START_SLEEP, 0.1)
        self.assertEqual(set(module.UNSET), {"DBUS_SESSION_BUS_ADDRESS", "PIPEWIRE_REMOTE"})


if __name__ == "__main__":
    unittest.main()
