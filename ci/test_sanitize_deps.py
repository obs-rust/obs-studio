#!/usr/bin/env -S uv run --script
"""The Sanitizers workflow installs the shared Tier 2 package list (#1395).

    uv run python -m unittest ci/test_sanitize_deps.py

rust/tools/sanitize/install-ubuntu-deps.sh scraped the package lines out of
rust/tools/linux-validate/Dockerfile. #967 moved them to apt-packages.txt,
the scrape found nothing, and every nightly asan/tsan/valgrind job stopped
in "Install dependencies" with "no package list found". This runs the script
against stub sudo/apt-get and checks it installs the whole shared list.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "rust" / "tools" / "sanitize" / "install-ubuntu-deps.sh"
PACKAGES = ROOT / "rust" / "tools" / "linux-validate" / "apt-packages.txt"


@unittest.skipUnless(shutil.which("bash"), "needs bash")
class InstallUbuntuDepsTest(unittest.TestCase):
    def run_script(self) -> list[list[str]]:
        with tempfile.TemporaryDirectory() as tmp:
            stubs = Path(tmp)
            log = stubs / "apt.log"
            (stubs / "sudo").write_text('#!/bin/sh\nexec "$@"\n', encoding="utf-8")
            (stubs / "apt-get").write_text(f'#!/bin/sh\necho "$*" >>"{log}"\n', encoding="utf-8")
            for stub in ("sudo", "apt-get"):
                (stubs / stub).chmod(0o755)
            env = dict(os.environ, PATH=f"{stubs}{os.pathsep}{os.environ['PATH']}")
            subprocess.run(["bash", str(SCRIPT)], check=True, env=env, capture_output=True, text=True)
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


if __name__ == "__main__":
    unittest.main()
