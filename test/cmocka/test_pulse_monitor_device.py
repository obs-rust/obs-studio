#!/usr/bin/env -S uv run --script
"""Runs a PipeWire-pulse end-to-end test (test_pulse_monitor_device,
test_pulse_stream_restart) against a private PipeWire sound server: its own
runtime dir, config and D-Bus, no hardware devices. Exits 77 (skip) when
PipeWire, WirePlumber, pipewire-pulse, pactl or dbus-run-session are missing,
and fails when they are installed but the server does not start.

    uv run --script test_pulse_monitor_device.py <test binary> [args]
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# The daemon's longest socket path, <dir>/pipewire-0-manager, must fit a Unix
# socket address (107 bytes); with a deep TMPDIR (a build tree) it does not and
# pipewire exits at once. Use /tmp then.
SOCKET_SUFFIX = "/pipewire-0-manager"
SOCKET_BUDGET = 107
# The wait for the private sound server to answer `pactl info`: 200 * 0.1 s.
START_ATTEMPTS = 200
START_SLEEP = 0.1
# The sound server and its helpers, all required to run the test.
TOOLS = ("pipewire", "wireplumber", "pipewire-pulse", "pactl", "dbus-run-session")
# WirePlumber with every hardware monitor disabled: no ALSA, Bluetooth, V4L2 or
# libcamera device enters the private graph.
WIREPLUMBER_CONFIG = """wireplumber.profiles = {
  main = {
    monitor.alsa = disabled
    monitor.alsa-midi = disabled
    monitor.bluez = disabled
    monitor.bluez-midi = disabled
    monitor.v4l2 = disabled
    monitor.libcamera = disabled
  }
}
"""


def missing_tool() -> str | None:
    """The first of the sound server and its helpers that is not on PATH."""
    for tool in TOOLS:
        if shutil.which(tool) is None:
            return tool
    return None


def private_dir() -> Path:
    """A new directory whose longest PipeWire socket path fits in a Unix socket
    address: a deep TMPDIR (a build tree) does not, so use /tmp then."""
    base = os.environ.get("TMPDIR") or "/tmp"
    directory = Path(tempfile.mkdtemp(prefix="obs-pw-test.", dir=base))
    if len(str(directory)) + len(SOCKET_SUFFIX) > SOCKET_BUDGET:
        shutil.rmtree(directory, ignore_errors=True)
        directory = Path(tempfile.mkdtemp(prefix="obs-pw-test.", dir="/tmp"))
    return directory


def start(program: str, log: Path) -> subprocess.Popen:
    """Start a daemon with its output in `log`, as the shell did with `&`."""
    with log.open("w", encoding="utf-8") as handle:
        return subprocess.Popen([program], stdout=handle, stderr=subprocess.STDOUT)


def stop(process: subprocess.Popen) -> None:
    if process.poll() is None:
        process.terminate()
    process.wait()


def wait_for_server(pipewire, pipewire_pulse, logs: tuple[Path, ...]) -> bool:
    """Poll `pactl info` until the sound server answers; False when a daemon
    dies or it never comes up (the daemon logs are printed then)."""
    attempts = 0
    while subprocess.run(["pactl", "info"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0:
        attempts += 1
        if pipewire.poll() is not None or pipewire_pulse.poll() is not None or attempts > START_ATTEMPTS:
            print("private pipewire-pulse did not start:")
            for log in logs:
                print(log.read_text(errors="replace"), end="")
            return False
        time.sleep(START_SLEEP)
    return True


def run(argv: list[str]) -> int:
    directory = private_dir()
    processes: list[subprocess.Popen] = []
    try:
        os.environ.update(
            XDG_RUNTIME_DIR=str(directory),
            PIPEWIRE_RUNTIME_DIR=str(directory),
            PULSE_RUNTIME_PATH=str(directory / "pulse"),
            XDG_CONFIG_HOME=str(directory / "config"),
            XDG_STATE_HOME=str(directory / "state"),
            HOME=str(directory / "home"),
        )
        config = directory / "config" / "wireplumber" / "wireplumber.conf.d"
        config.mkdir(parents=True, exist_ok=True)
        (directory / "state").mkdir(parents=True, exist_ok=True)
        (directory / "home").mkdir(parents=True, exist_ok=True)
        (config / "90-no-hardware.conf").write_text(WIREPLUMBER_CONFIG, encoding="utf-8")

        pipewire_log = directory / "pipewire.log"
        wireplumber_log = directory / "wireplumber.log"
        pulse_log = directory / "pipewire-pulse.log"
        pipewire = start("pipewire", pipewire_log)
        processes.append(pipewire)
        time.sleep(1)
        processes.append(start("wireplumber", wireplumber_log))
        pipewire_pulse = start("pipewire-pulse", pulse_log)
        processes.append(pipewire_pulse)

        os.environ["PULSE_SERVER"] = f"unix:{directory / 'pulse' / 'native'}"
        # Everything is installed, so a sound server that does not come up is a
        # failure, not a reason to skip: ctest counts skipped tests as passed.
        if not wait_for_server(pipewire, pipewire_pulse, (pipewire_log, wireplumber_log, pulse_log)):
            return 1
        return subprocess.run(argv).returncode
    finally:
        # The shell stops pipewire-pulse, WirePlumber then pipewire and removes
        # the private directory.
        for process in reversed(processes):
            stop(process)
        shutil.rmtree(directory, ignore_errors=True)


def main(argv: list[str]) -> int:
    if not os.environ.get("PRIVATE_PIPEWIRE_STARTED"):
        tool = missing_tool()
        if tool is not None:
            print(f"{tool} not found: skipping")
            return 77
        # The private D-Bus session owns the sound server; this file runs again
        # inside it, with the ready flag set.
        env = dict(os.environ, PRIVATE_PIPEWIRE_STARTED="1")
        os.execvpe(
            "dbus-run-session",
            ["dbus-run-session", "--", sys.executable, str(Path(__file__).resolve()), *argv[1:]],
            env,
        )
    if len(argv) < 2:
        print(f"usage: {Path(argv[0]).name} <test binary> [args]", file=sys.stderr)
        return 2
    return run(argv[1:])


if __name__ == "__main__":
    sys.exit(main(sys.argv))
