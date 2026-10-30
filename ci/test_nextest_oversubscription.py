#!/usr/bin/env -S uv run --script
"""Fast Gate oversubscribes nextest, with native_build capped (#3071).

    uv run python -m unittest discover -s ci -p test_nextest_oversubscription.py

The Fast Gate test binaries are I/O and subprocess bound, so the 4-vCPU runner
idles some of its four slots at `--test-threads 4` (326 s of test time took
about 82 s of wall in merge-queue run 41042404269). `native_build::*` is the
exception: those tests drive cmake and a compiler, so they stay in one group of
two concurrent tests while the rest of the run oversubscribes.
"""

from __future__ import annotations

import importlib.util
import tomllib
import unittest
from pathlib import Path
from unittest import mock

CI = Path(__file__).resolve().parent
CONFIG = CI / "nextest.toml"

spec = importlib.util.spec_from_file_location("oversubscription_gate", CI / "local_gate.py")
assert spec is not None and spec.loader is not None
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)


class FastGateOversubscriptionTest(unittest.TestCase):
    def setUp(self) -> None:
        self.cfg = tomllib.loads(CONFIG.read_text(encoding="utf-8"))

    def test_native_build_group_is_capped_at_two(self):
        groups = self.cfg.get("test-groups", {})
        self.assertIn("native-build", groups, "the native_build test group is missing")
        self.assertEqual(groups["native-build"].get("max-threads"), 2)

    def test_both_profiles_put_native_build_in_the_group(self):
        for profile in ("default", "full"):
            overrides = self.cfg["profile"][profile].get("overrides", [])
            grouped = [o for o in overrides if o.get("test-group") == "native-build"]
            self.assertTrue(grouped, f"profile {profile} has no native-build group override")
            self.assertTrue(
                any("native_build" in o["filter"] for o in grouped),
                f"profile {profile}: the grouped override does not name native_build",
            )

    def test_default_threads_oversubscribe_the_ci_runner(self):
        self.assertEqual(gate.DEFAULT_TEST_THREADS, 6)
        with mock.patch.object(gate.os, "cpu_count", return_value=4):
            with mock.patch.dict(gate.os.environ, {}, clear=False):
                gate.os.environ.pop("OBS_GATE_TEST_THREADS", None)
                self.assertEqual(gate.test_threads(), 6)

    def test_host_override_still_wins(self):
        with mock.patch.dict(gate.os.environ, {"OBS_GATE_TEST_THREADS": "16"}, clear=False):
            self.assertEqual(gate.test_threads(), 16)


if __name__ == "__main__":
    unittest.main()
