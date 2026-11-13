#!/usr/bin/env -S uv run --script
"""Tests for ci/gh_graphql.py, the one GraphQL entry point (through ghd).

    uv run python -m unittest ci/test_gh_graphql.py

A fake `ghd` (GHD_EXE) records its arguments and prints a canned body, so no
test touches the network or needs ghd installed.
"""

from __future__ import annotations

import json
import os
import runpy
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "ci" / "gh_graphql.py"

FAKE_GHD = """#!/bin/sh
printf '%s\\n' "$@" > "$GHD_ARGS_OUT"
printf '%s' "$GHD_BODY"
exit "${GHD_EXIT:-0}"
"""


class GhGraphql(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.ghd = self.tmp / "ghd"
        self.ghd.write_text(FAKE_GHD)
        self.ghd.chmod(0o755)
        self.args_out = self.tmp / "args"

    def run_script(self, *args, body='{"data":{"viewer":{"login":"octo"}}}', exit_code=0):
        env = dict(
            os.environ,
            GHD_EXE=str(self.ghd),
            GHD_ARGS_OUT=str(self.args_out),
            GHD_BODY=body,
            GHD_EXIT=str(exit_code),
        )
        return subprocess.run(
            [sys.executable, str(SCRIPT), *args], env=env, capture_output=True, text=True, check=False
        )

    def ghd_args(self):
        return self.args_out.read_text().splitlines()

    def test_passes_gh_arguments_to_ghd_api_graphql(self):
        out = self.run_script("-f", "query={viewer{login}}", "-F", "n=3")
        self.assertEqual(out.returncode, 0)
        self.assertEqual(self.ghd_args(), ["api", "graphql", "-f", "query={viewer{login}}", "-F", "n=3"])
        self.assertEqual(json.loads(out.stdout)["data"]["viewer"]["login"], "octo")

    def test_a_pasted_gh_api_graphql_prefix_is_tolerated(self):
        self.run_script("api", "graphql", "-f", "query=x")
        self.assertEqual(self.ghd_args(), ["api", "graphql", "-f", "query=x"])

    @unittest.skipUnless(shutil.which("jq"), "jq not installed")
    def test_jq_prints_like_gh(self):
        for flag in (["--jq", ".data.viewer.login"], ["-q", ".data.viewer.login"], ["--jq=.data.viewer.login"]):
            with self.subTest(flag=flag):
                out = self.run_script("-f", "query=x", *flag)
                self.assertEqual(out.stdout, "octo\n")
                self.assertNotIn("--jq", self.ghd_args())

    def test_ghd_failure_is_passed_through(self):
        out = self.run_script("-f", "query=x", "--jq", ".data", body='{"errors":[{"message":"no"}]}', exit_code=1)
        self.assertEqual(out.returncode, 1)
        self.assertIn('"errors"', out.stdout)

    def test_cold_fallback_uses_pinned_wheel_without_source_builds(self):
        wrapper = runpy.run_path(str(SCRIPT))
        with mock.patch.dict(os.environ, {}, clear=True), mock.patch(
            "shutil.which", side_effect=lambda tool: "/tools/uvx" if tool == "uvx" else None
        ):
            self.assertEqual(
                wrapper["ghd_command"](),
                ["/tools/uvx", "--no-build", "--from", "ghd==0.2.1", "ghd"],
            )


if __name__ == "__main__":
    unittest.main()
