#!/usr/bin/env -S uv run --script
"""Runs a PulseAudio end-to-end test against a private PulseAudio daemon (not
pipewire-pulse): its own runtime dir, HOME and D-Bus, no hardware, session
or Bluetooth modules, so there is no audio output; the test loads null sinks
itself. test_pulse_monitor_device.py is the same for PipeWire.
The daemon is "pulseaudio" from PATH, or $OBS_TEST_PULSEAUDIO (e.g. a script
that runs an unpacked build). Exits 77 (skip) when it, pactl or
dbus-run-session are missing, and fails when they are installed but the
daemon does not start.

    uv run --script test_pulse_pulseaudio.py <test binary> [args]
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# The daemon's socket path, <dir>/pulse/native, must fit a Unix socket address.
SOCKET_OVERHEAD = len("/pulse/native")
SOCKET_LIMIT = 107
# Only the native protocol. Moving and rescuing streams is done by the core
# (PulseAudio 15 and later), not by a module.
CONFIG = "load-module module-native-protocol-unix\n"
DAEMON_ARGS = (
    "--daemonize=no",
    "--exit-idle-time=-1",
    "--use-pid-file=no",
    "--system=no",
    "--disallow-exit",
    "--log-target=stderr",
)


def missing_tool(pulseaudio: str) -> str | None:
    """The first of the daemon and its helpers that is not on PATH."""
    for tool in (pulseaudio, "pactl", "dbus-run-session"):
        if shutil.which(tool) is None:
            return tool
    return None


def private_dir() -> Path:
    """A new 0700 directory with a PulseAudio socket path that fits in a
    Unix socket address: a deep TMPDIR (a build tree) does not, so use /tmp."""
    base = os.environ.get("TMPDIR") or "/tmp"
    directory = Path(tempfile.mkdtemp(prefix="obs-pa-test.", dir=base))
    if len(str(directory)) + SOCKET_OVERHEAD > SOCKET_LIMIT:
        shutil.rmtree(directory, ignore_errors=True)
        directory = Path(tempfile.mkdtemp(prefix="obs-pa-test.", dir="/tmp"))
    directory.chmod(0o700)
    return directory


def stop(daemon: subprocess.Popen) -> None:
    if daemon.poll() is None:
        daemon.terminate()
    daemon.wait()


def wait_for_daemon(daemon: subprocess.Popen, log: Path) -> bool:
    """Poll pactl until the daemon answers; False when it dies or never comes."""
    attempts = 0
    while subprocess.run(["pactl", "info"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0:
        attempts += 1
        if daemon.poll() is not None or attempts > 200:
            print("private pulseaudio did not start:")
            print(log.read_text(errors="replace"), end="")
            return False
        time.sleep(0.1)
    return True


def run(argv: list[str], pulseaudio: str) -> int:
    directory = private_dir()
    try:
        os.environ.update(
            XDG_RUNTIME_DIR=str(directory),
            PULSE_RUNTIME_PATH=str(directory / "pulse"),
            PULSE_STATE_PATH=str(directory / "state"),
            XDG_CONFIG_HOME=str(directory / "config"),
            XDG_STATE_HOME=str(directory / "state"),
            HOME=str(directory / "home"),
        )
        for name in ("config", "state", "home"):
            (directory / name).mkdir(parents=True, exist_ok=True)

        config = directory / "test.pa"
        config.write_text(CONFIG, encoding="utf-8")
        log = directory / "pulseaudio.log"
        with log.open("w", encoding="utf-8") as handle:
            daemon = subprocess.Popen(
                [pulseaudio, "-n", "-F", str(config), *DAEMON_ARGS],
                stdout=handle,
                stderr=subprocess.STDOUT,
            )
        try:
            os.environ["PULSE_SERVER"] = f"unix:{directory / 'pulse' / 'native'}"
            # Everything is installed, so a daemon that does not come up is a
            # failure, not a reason to skip: ctest counts skipped tests as passed.
            if not wait_for_daemon(daemon, log):
                return 1
            return subprocess.run(argv).returncode
        finally:
            stop(daemon)
    finally:
        shutil.rmtree(directory, ignore_errors=True)


def main(argv: list[str]) -> int:
    pulseaudio = os.environ.get("OBS_TEST_PULSEAUDIO") or "pulseaudio"
    if not os.environ.get("PRIVATE_PULSEAUDIO_STARTED"):
        tool = missing_tool(pulseaudio)
        if tool is not None:
            print(f"{tool} not found: skipping (needs a PulseAudio daemon, not pipewire-pulse)")
            return 77
        # The private D-Bus session owns the daemon; this file runs again in it.
        env = dict(os.environ, PRIVATE_PULSEAUDIO_STARTED="1", OBS_TEST_PULSEAUDIO=pulseaudio)
        os.execvpe(
            "dbus-run-session",
            ["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()), *argv[1:]],
            env,
        )
    if len(argv) < 2:
        print(f"usage: {Path(argv[0]).name} <test binary> [args]", file=sys.stderr)
        return 2
    return run(argv[1:], pulseaudio)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
