#!/usr/bin/env -S uv run --script
"""docker-task clears a dead zccache daemon's state before the task (#3383).

zccache records its daemon as pid plus boot_id. Every container shares the
host's boot_id but starts a new PID namespace, so on a persistent cache volume
the next container finds the old pid taken by another process, decides the
daemon "exists but is not accepting connections", enters a 60 s cool-down and
fails every compile with exit 125. A daemon whose socket refuses a connection
cannot serve anyone, so docker-task removes its socket, lock, spawn-failed
marker and process record before running the command. A daemon that still
accepts (another container on the same volume) is left alone.

    uv run python -m unittest ci/test_docker_task_zccache.py
"""

from __future__ import annotations

import importlib.util
import os
import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
MODULE = ROOT / "ci" / "docker_task.py"


def load_module():
    spec = importlib.util.spec_from_file_location("docker_task_zccache", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


DT = load_module()

STATE = ("daemon-v1.15.5.lock", "daemon-v1.15.5.lock.spawn-failed", "daemon.running-process.json")


def make_state(root: Path) -> Path:
    version = root / "v1.15.5"
    version.mkdir(parents=True)
    for name in STATE:
        (version / name).write_text("403\n", encoding="utf-8")
    (version / "index.bin").write_text("keep\n", encoding="utf-8")
    return version


def run_script(cache: Path, marker: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["sh", "-c", DT.ZCCACHE_STALE_SCRIPT, "docker-task-zccache", "sh", "-c", f"echo ran > {marker}"],
        env={**os.environ, "ZCCACHE_CACHE_DIR": str(cache)},
        capture_output=True,
        text=True,
    )


class StaleDaemonTest(unittest.TestCase):
    def test_a_refusing_socket_is_cleared_and_the_command_runs(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        version = make_state(tmp / "cache")
        sock_path = version / "daemon-v1.15.5.sock"
        # A bound then closed listener leaves the socket file; connects are refused.
        dead = socket.socket(socket.AF_UNIX)
        dead.bind(str(sock_path))
        dead.close()

        result = run_script(tmp / "cache", tmp / "marker")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((tmp / "marker").read_text().strip(), "ran")
        for name in (*STATE, "daemon-v1.15.5.sock"):
            self.assertFalse((version / name).exists(), name)
        self.assertTrue((version / "index.bin").exists(), "the cache itself is kept")
        self.assertIn("stale zccache daemon", result.stderr)

    def test_a_missing_socket_with_a_stale_record_is_cleared(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        version = make_state(tmp / "cache")
        result = run_script(tmp / "cache", tmp / "marker")
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in STATE:
            self.assertFalse((version / name).exists(), name)

    def test_a_live_daemon_is_left_alone(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        version = make_state(tmp / "cache")
        live = socket.socket(socket.AF_UNIX)
        live.bind(str(version / "daemon-v1.15.5.sock"))
        live.listen(4)
        self.addCleanup(live.close)

        result = run_script(tmp / "cache", tmp / "marker")
        self.assertEqual(result.returncode, 0, result.stderr)
        for name in (*STATE, "daemon-v1.15.5.sock"):
            self.assertTrue((version / name).exists(), name)

    def test_the_command_status_is_kept(self):
        tmp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        result = subprocess.run(
            ["sh", "-c", DT.ZCCACHE_STALE_SCRIPT, "docker-task-zccache", "sh", "-c", "exit 7"],
            env={**os.environ, "ZCCACHE_CACHE_DIR": str(tmp / "none")},
        )
        self.assertEqual(result.returncode, 7)


class RunWrapsTest(unittest.TestCase):
    def command_for(self, envs: list[str]) -> list[str]:
        argv = ["--name", "t", "--repo", str(ROOT), "--image", "img"]
        for spec in envs:
            argv += ["--env", spec]
        argv += ["--", "/script", "arg"]
        with (
            patch.object(DT, "build_image", return_value="img:tag"),
            patch.object(DT, "release_container"),
            patch.object(DT, "check_budgets"),
            patch.object(DT.subprocess, "run") as docker,
        ):
            docker.return_value.returncode = 0
            DT.run(argv)
        return docker.call_args.args[0]

    def test_a_zccache_dir_wraps_the_command(self):
        command = self.command_for(["ZCCACHE_CACHE_DIR=/zccache"])
        start = command.index("img:tag") + 1
        self.assertEqual(command[start : start + 2], ["sh", "-c"])
        self.assertEqual(command[start + 2], DT.ZCCACHE_STALE_SCRIPT)
        self.assertEqual(command[-2:], ["/script", "arg"])

    def test_no_zccache_dir_leaves_the_command(self):
        command = self.command_for(["OTHER=1"])
        start = command.index("img:tag") + 1
        self.assertEqual(command[start:], ["/script", "arg"])


if __name__ == "__main__":
    unittest.main()
