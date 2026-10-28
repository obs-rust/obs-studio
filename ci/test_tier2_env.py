#!/usr/bin/env -S uv run --script
"""Tier 2 Linux gives the tests the same environment in Docker and natively (#982).

    uv run python -m unittest ci/test_tier2_env.py

validate.sh used to inherit whatever the harness gave it: root with /root
and no locale in the container, the `runner` user with a lived-in HOME,
C.UTF-8 and the runner image's /usr/local/bin cmake on the native runner.
It now pins the locale, timezone and PATH order itself, runs ctest with a
fresh HOME and XDG dirs under the build root, and prints an environment
fingerprint so any remaining difference shows in the log.
"""

from __future__ import annotations

import re
import os
import subprocess
import unittest
from pathlib import Path

VALIDATE = Path(__file__).resolve().parent.parent / "rust" / "tools" / "linux-validate" / "validate.sh"


class PinnedEnvTest(unittest.TestCase):
    def setUp(self):
        self.text = VALIDATE.read_text(encoding="utf-8")

    def test_locale_and_timezone_are_pinned(self):
        self.assertRegex(self.text, r"export LANG=C\.UTF-8 LC_ALL=C\.UTF-8 TZ=UTC")

    def test_distro_tools_come_first(self):
        self.assertIn('export PATH="/usr/bin:/bin:$PATH"', self.text)

    def test_ctest_runs_with_a_fresh_home(self):
        self.assertIn('test_home="$BUILD/test-home"', self.text)
        for var in ("HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_CACHE_HOME"):
            self.assertRegex(self.text, rf'{var}="\$test_home')
        # Each mode's environment starts from test_env (mode ON adds
        # LD_LIBRARY_PATH for libobs-with-rust, #1065).
        self.assertIn('mode_env=("${test_env[@]}")', self.text)
        for line in re.findall(r"^.*xvfb-run -a ctest.*$", self.text, re.M):
            self.assertIn('"${mode_env[@]}"', line, line)

    def test_environment_fingerprint_is_printed(self):
        self.assertIn("== environment:", self.text)
        for probe in ("id -un", "nproc", "cmake --version", "cc --version"):
            self.assertIn(probe, self.text)

    def test_parallel_tests_do_not_inherit_an_oversized_software_renderer_pool(self):
        # Exercise the actual Bash environment array: an inherited renderer
        # pool must not multiply across all eight concurrent test processes.
        # The pairs array is the source of truth since the serial part of
        # #2773 needs the same NAME=VALUE list as --env arguments.
        block = re.search(r"test_env_pairs=\([\s\S]*?\)\ntest_env=\([^\n]*\)\n", self.text)[0]
        result = subprocess.run(["bash", "-c", 'test_home=/fixture-home\n' + block +
                                 '"${test_env[@]}" /usr/bin/env'],
                                env={**os.environ, "LP_NUM_THREADS": "128"}, capture_output=True, text=True, check=True)
        environment = dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)
        self.assertEqual(environment["LP_NUM_THREADS"], "1")
        self.assertEqual(environment["HOME"], "/fixture-home")


if __name__ == "__main__":
    unittest.main()
