#!/usr/bin/env -S uv run --script
"""Runs test_pipewire_cursor_hdr against a private PipeWire daemon (obs-rust/obs-studio#962).

Port of `test_pipewire_cursor_hdr.sh`. When pipewire is installed, starts a
private daemon: its own runtime dir and config, no session manager, no devices,
and never the user's session bus. Exits 77 (skip) when pipewire is not
installed, and fails when it is installed but the daemon does not start.

usage: test_pipewire_cursor_hdr.py <test binary>
"""

from __future__ import annotations

import os
import shutil
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# The daemon's longest socket path, <dir>/pipewire-0-manager, must fit a Unix
# socket address (107 bytes); with a deep TMPDIR (a build tree) it does not and
# pipewire exits at once. Use /tmp then.
SOCKET_BUDGET = 107
SOCKET_SUFFIX = len("/pipewire-0-manager")
START_ATTEMPTS = 200
START_SLEEP = 0.1

# The shell's two `unset`s, in its order.
UNSET = ("DBUS_SESSION_BUS_ADDRESS", "PIPEWIRE_REMOTE")

USAGE = "usage: test_pipewire_cursor_hdr.py <test binary>"


def runtime_directory() -> Path:
    """`mktemp -d`, moved to /tmp when the daemon's socket path would not fit."""
    directory = tempfile.mkdtemp(
        prefix="obs-pw-cursor-test.", dir=os.environ.get("TMPDIR") or "/tmp"
    )
    if len(directory) + SOCKET_SUFFIX > SOCKET_BUDGET:
        shutil.rmtree(directory, ignore_errors=True)
        directory = tempfile.mkdtemp(prefix="obs-pw-cursor-test.", dir="/tmp")
    return Path(directory)


def socket_is_up(socket: Path) -> bool:
    """The shell's `[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]`."""
    try:
        return stat.S_ISSOCK(socket.stat().st_mode)
    except OSError:
        return False


def start_daemon(directory: Path) -> subprocess.Popen:
    """Start the private daemon with this directory as its runtime dir."""
    with (directory / "pipewire.log").open("w") as log:
        return subprocess.Popen(["pipewire"], stdout=log, stderr=subprocess.STDOUT)


def wait_for_socket(directory: Path, daemon: subprocess.Popen) -> bool:
    """Wait for the daemon's socket; report a daemon that does not come up."""
    socket = directory / "pipewire-0"
    for _ in range(START_ATTEMPTS):
        if socket_is_up(socket):
            return True
        # pipewire is installed, so a daemon that does not come up is a
        # failure, not a reason to skip: ctest counts skipped tests as passed.
        if daemon.poll() is not None:
            break
        time.sleep(START_SLEEP)

    print("private pipewire did not start:")
    print((directory / "pipewire.log").read_text(errors="replace"), end="")
    return False


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print(USAGE, file=sys.stderr)
        return 2
    test_bin = argv[0]

    for name in UNSET:
        os.environ.pop(name, None)

    if not shutil.which("pipewire"):
        print("pipewire not found: skipping")
        return 77

    directory = runtime_directory()
    daemon: subprocess.Popen | None = None
    try:
        os.environ["XDG_RUNTIME_DIR"] = str(directory)
        os.environ["PIPEWIRE_RUNTIME_DIR"] = str(directory)
        os.environ["XDG_CONFIG_HOME"] = str(directory / "config")
        os.environ["XDG_STATE_HOME"] = str(directory / "state")
        os.environ["HOME"] = str(directory / "home")
        for path in (directory / "config", directory / "state", directory / "home"):
            path.mkdir(parents=True, exist_ok=True)

        daemon = start_daemon(directory)
        if not wait_for_socket(directory, daemon):
            return 1
        return subprocess.run([test_bin]).returncode
    finally:
        if daemon is not None and daemon.poll() is None:
            daemon.kill()
            daemon.wait()
        shutil.rmtree(directory, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
