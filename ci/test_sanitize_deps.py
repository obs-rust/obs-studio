#!/usr/bin/env -S uv run --script
"""The Sanitizers workflow installs the shared Tier 2 package list (#1395).

    uv run python -m unittest ci/test_sanitize_deps.py

rust/tools/sanitize/install_ubuntu_deps.py scraped the package lines out of
rust/tools/linux-validate/Dockerfile. #967 moved them to apt-packages.txt,
the scrape found nothing, and every nightly asan/tsan/valgrind job stopped
in "Install dependencies" with "no package list found". The rule lives in
rust/tools/sanitize/install_ubuntu_deps.py since the port off the shell script
(#962), and sanitizers.yaml runs it through `uv run --no-project`. This drives
the port against stub sudo/apt-get on PATH and checks it installs the whole
shared list.
"""

from __future__ import annotations

import importlib.util
import io
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent.parent
SANITIZE = ROOT / "rust" / "tools" / "sanitize"
MODULE = SANITIZE / "install_ubuntu_deps.py"
SHELL_SCRIPT = SANITIZE / "install-ubuntu-deps.sh"
PACKAGES = ROOT / "rust" / "tools" / "linux-validate" / "apt-packages.txt"


def load_module():
    """rust/tools/sanitize/install_ubuntu_deps.py by path: `uv run python -m
    unittest ci/test_sanitize_deps.py` does not put that directory on sys.path,
    and no file in this repo edits sys.path (ci/test_no_sys_path_edits.py)."""
    spec = importlib.util.spec_from_file_location("install_ubuntu_deps", MODULE)
    assert spec is not None and spec.loader is not None, f"cannot load {MODULE}"
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class InstallUbuntuDepsTest(unittest.TestCase):
    def setUp(self):
        self.module = load_module()
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def run_script(self) -> list[list[str]]:
        stubs = Path(self.tmp.name)
        log = stubs / "apt.log"
        (stubs / "sudo").write_text('#!/bin/sh\nexec "$@"\n', encoding="utf-8")
        (stubs / "apt-get").write_text(f'#!/bin/sh\necho "$*" >>"{log}"\n', encoding="utf-8")
        for stub in ("sudo", "apt-get"):
            (stubs / stub).chmod(0o755)
        env = dict(os.environ, PATH=f"{stubs}{os.pathsep}{os.environ['PATH']}")
        out, err = io.StringIO(), io.StringIO()
        with patch.dict(os.environ, env, clear=True), redirect_stdout(out), redirect_stderr(err):
            code = self.module.main(["install_ubuntu_deps.py"])
        self.assertEqual(code, 0, out.getvalue() + err.getvalue())
        return [line.split() for line in log.read_text(encoding="utf-8").splitlines()]

    def test_installs_the_shared_list_and_the_launch_tools(self):
        calls = self.run_script()
        self.assertEqual(calls[0], ["update"])
        install = calls[1]
        self.assertEqual(install[:3], ["install", "-y", "--no-install-recommends"])
        shared = [
            line.strip()
            for line in PACKAGES.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.startswith("#")
        ]
        self.assertGreater(len(shared), 50)
        for pkg in [*shared, "valgrind", "libgl1-mesa-dri", "dbus-x11"]:
            self.assertIn(pkg, install)
        self.assertFalse(any(p.startswith("#") for p in install), "comment text reached apt-get")

    def test_an_empty_package_list_stops_before_apt(self):
        empty = Path(self.tmp.name) / "apt-packages.txt"
        empty.write_text("# only comments\n", encoding="utf-8")
        out, err = io.StringIO(), io.StringIO()
        with redirect_stdout(out), redirect_stderr(err), self.assertRaises(SystemExit) as caught:
            self.module.packages(empty)
        self.assertEqual(caught.exception.code, 1, out.getvalue())
        self.assertIn("no packages found", err.getvalue())

    def test_the_shell_script_is_gone(self):
        self.assertFalse(SHELL_SCRIPT.exists(), "install-ubuntu-deps.sh still exists")


if __name__ == "__main__":
    unittest.main()
