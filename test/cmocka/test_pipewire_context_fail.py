#!/usr/bin/env -S uv run --script
"""Runs test_pipewire_context_fail with a private PipeWire daemon (obs-rust/obs-studio#962).

Port of `test_pipewire_context_fail.sh`. When pipewire is installed, starts a
private daemon for the control case: its own runtime dir and a minimal
configuration (native protocol and access control only: without the access
module clients never get permissions and hang; no D-Bus, no realtime or X11
modules, no session manager, no devices). Without one, the test still runs and
only the control case skips; a pipewire that is installed but does not start
fails.

usage: test_pipewire_context_fail.py <test binary>
"""

from __future__ import annotations

import os
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# The daemon's longest socket path, <dir>/run/pipewire-0-manager, must fit a
# Unix socket address (107 bytes); with a deep TMPDIR (a build tree) it does not
# and pipewire exits at once. Use /tmp then.
SOCKET_BUDGET = 107
SOCKET_SUFFIX = len("/run/pipewire-0-manager")
START_ATTEMPTS = 200
START_SLEEP = 0.1
# Seconds a stopped daemon gets to exit on SIGTERM before its group is killed.
STOP_GRACE = 5

DAEMON_CONFIG = """context.properties = {
    core.daemon = true
    core.name   = pipewire-0
    support.dbus = false
}
context.spa-libs = {
    support.* = support/libspa-support
}
context.modules = [
    { name = libpipewire-module-protocol-native }
    { name = libpipewire-module-access }
]
"""

# The shell's `unset`, in its order.
UNSET = (
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "HYPRLAND_INSTANCE_SIGNATURE",
    "DBUS_SESSION_BUS_ADDRESS",
    "DBUS_SYSTEM_BUS_ADDRESS",
    "PIPEWIRE_REMOTE",
    "PIPEWIRE_RUNTIME_DIR",
    "PIPEWIRE_CONFIG_DIR",
    "PIPEWIRE_CONFIG_NAME",
)

USAGE = "usage: test_pipewire_context_fail.py <test binary>"


def runtime_directory() -> Path:
    """`mktemp -d`, moved to /tmp when the daemon's socket path would not fit."""
    directory = tempfile.mkdtemp(
        prefix="obs-pw-context-test.", dir=os.environ.get("TMPDIR") or "/tmp"
    )
    if len(directory) + SOCKET_SUFFIX > SOCKET_BUDGET:
        shutil.rmtree(directory, ignore_errors=True)
        directory = tempfile.mkdtemp(prefix="obs-pw-context-test.", dir="/tmp")
    return Path(directory)


def socket_is_up(socket: Path) -> bool:
    """The shell's `[ -S "$XDG_RUNTIME_DIR/pipewire-0" ]`."""
    try:
        return stat.S_ISSOCK(socket.stat().st_mode)
    except OSError:
        return False


def start_daemon(directory: Path, runtime: Path) -> subprocess.Popen:
    """Start the private daemon with its own minimal configuration."""
    daemon_config = directory / "daemon-config"
    daemon_config.mkdir(parents=True, exist_ok=True)
    (daemon_config / "pipewire.conf").write_text(DAEMON_CONFIG)
    with (directory / "pipewire.log").open("w") as log:
        return subprocess.Popen(
            ["pipewire"],
            env={
                **os.environ,
                "PIPEWIRE_CONFIG_DIR": str(daemon_config),
                "PIPEWIRE_RUNTIME_DIR": str(runtime),
            },
            stdout=log,
            stderr=subprocess.STDOUT,
            # Its own process group: `pipewire` may be a wrapper (a `uv run`
            # stub, a Nix wrapper) whose child is the real daemon, and killing
            # only the wrapper orphans it (obs-rust/obs-studio#2984).
            start_new_session=True,
        )


def stop_daemon(daemon: subprocess.Popen) -> None:
    """Stop the daemon's whole process group, not just the direct child."""
    try:
        os.killpg(daemon.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        daemon.wait(timeout=STOP_GRACE)
    except subprocess.TimeoutExpired:
        pass
    # The wrapper may be gone while the group lives on, so always sweep it.
    try:
        os.killpg(daemon.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    daemon.wait()


def wait_for_socket(directory: Path, runtime: Path, daemon: subprocess.Popen) -> bool:
    """Wait for the daemon's socket; report a daemon that does not come up."""
    socket = runtime / "pipewire-0"
    for _ in range(START_ATTEMPTS):
        if socket_is_up(socket):
            return True
        # pipewire is installed, so a daemon that does not come up is a
        # failure: the control case would skip and ctest count it as passed.
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

    directory = runtime_directory()
    daemon: subprocess.Popen | None = None
    try:
        runtime = directory / "run"
        os.environ["XDG_RUNTIME_DIR"] = str(runtime)
        os.environ["XDG_CONFIG_HOME"] = str(directory / "config")
        os.environ["XDG_STATE_HOME"] = str(directory / "state")
        os.environ["HOME"] = str(directory / "home")
        for path in (runtime, directory / "config", directory / "state", directory / "home"):
            path.mkdir(parents=True, exist_ok=True)

        if shutil.which("pipewire"):
            daemon = start_daemon(directory, runtime)
            if not wait_for_socket(directory, runtime, daemon):
                return 1
            os.environ["PIPEWIRE_RUNTIME_DIR"] = str(runtime)
        else:
            print("pipewire not found; the control case will skip")

        return subprocess.run([test_bin]).returncode
    finally:
        if daemon is not None:
            stop_daemon(daemon)
        shutil.rmtree(directory, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
