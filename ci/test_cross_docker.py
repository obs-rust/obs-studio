#!/usr/bin/env -S uv run --script
"""The cross-compile check runs in a container, so the host needs only Docker.

    uv run python -m unittest ci/test_cross_docker.py

cmake/cross/docker.py wraps the same check `uv run --script ci/docker_task.py`
runs for the CI lanes, with the caches and volume budgets the cross builds need.
These tests stub `uv` and check the argument vector it is handed, so no
container is started.
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
SCRIPT = ROOT / "cmake" / "cross" / "docker.py"
DOCS = ROOT / "docs" / "ci-tooling.md"

# The uv stub records argv as JSON, so a container command stays one argument.
UV_STUB = ("import json, os, sys\n"
           "open(os.environ['CALLS'], 'w').write(json.dumps(sys.argv[1:]))\n")


class CrossDockerTest(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        stub = self.bin / "uv"
        stub.write_text(f"#!{sys.executable}\n" + UV_STUB)
        stub.chmod(0o755)
        self.calls = self.root / "calls.log"

    def run_script(self, *args: str, **env: str) -> subprocess.CompletedProcess:
        environment = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
        environment.update(PATH=f"{self.bin}{os.pathsep}{environment['PATH']}", CALLS=str(self.calls), **env)
        return subprocess.run([sys.executable, str(SCRIPT), *args], capture_output=True, text=True, env=environment)

    def argv(self) -> list[str]:
        return json.loads(self.calls.read_text(encoding="utf-8"))

    def test_every_alias_is_built_in_one_container(self) -> None:
        result = self.run_script("mac-arm64", "win-x64")
        self.assertEqual(result.returncode, 0, result.stderr)
        argv = self.argv()
        self.assertEqual(argv[:4], ["run", "--script", str(ROOT / "ci" / "docker_task.py"), "run"])
        self.assertIn("obs-cross", argv)
        self.assertIn("--source", argv)
        self.assertEqual(argv[argv.index("--source") + 1], "overlay")
        budgets = [argv[i + 1] for i, arg in enumerate(argv) if arg == "--budget"]
        self.assertEqual(budgets, ["build=6G", "soldr=12G", "deps=2G"])
        self.assertEqual(argv[-3:], ["cross", "mac-arm64", "win-x64"])
        # cmd is one argument: every alias is configured and built in turn.
        command = argv[argv.index("-c") + 1]
        self.assertIn('for alias in "$@"', command)
        self.assertIn('cmake -S cmake/cross -B "/build/$alias"', command)
        self.assertIn("-- -k 0", command)

    def test_the_pinned_toolchain_and_soldr_versions_are_forwarded(self) -> None:
        self.assertEqual(self.run_script("win-x64").returncode, 0)
        argv = self.argv()
        toolchain = self.environment(argv, "RUST_TOOLCHAIN")
        soldr = self.environment(argv, "SOLDR_VERSION")
        self.assertRegex(toolchain, r"^\d+\.\d+\.\d+$")
        self.assertRegex(soldr, r"^\d+\.\d+\.\d+$")

    def environment(self, argv: list[str], name: str) -> str:
        value = next(value for value in argv if value.startswith(name + "="))
        return value.split("=", 1)[1]

    def test_clean_and_wait_reach_the_task_runner(self) -> None:
        self.assertEqual(self.run_script("--clean", "--wait", "win-x64").returncode, 0)
        argv = self.argv()
        self.assertIn("--clean", argv)
        self.assertIn("--wait", argv)
        self.assertLess(argv.index("--clean"), argv.index("--wait"))

    def test_extra_cmake_arguments_are_forwarded_to_the_container(self) -> None:
        extra = "-DOBS_UNITY_BUILD_BATCH_SIZE=0"
        self.assertEqual(self.run_script("win-x64", OBS_CROSS_CMAKE_ARGS=extra).returncode, 0)
        self.assertEqual(self.environment(self.argv(), "OBS_CROSS_CMAKE_ARGS"), extra)
        # The container's shell expands it: the command only names the variable.
        self.assertIn("${OBS_CROSS_CMAKE_ARGS:-}", self.argv()[self.argv().index("-c") + 1])

    def test_usage_is_printed_without_an_alias_or_for_an_option(self) -> None:
        for args in ((), ("--bogus", "win-x64")):
            with self.subTest(args=args):
                result = self.run_script(*args)
                self.assertEqual(result.returncode, 2)
                self.assertIn("usage:", result.stderr)
                self.assertFalse(self.calls.exists())

    def test_the_docs_name_the_python_entry_point(self) -> None:
        self.assertIn("cmake/cross/docker.py", DOCS.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
