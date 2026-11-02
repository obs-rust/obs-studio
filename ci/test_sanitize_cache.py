#!/usr/bin/env -S uv run --script
"""PR asan jobs find a clean master archive (#3501).

    uv run --no-project ci/test_sanitize_cache.py

A PR restores only what master saved. The nightly restored a stale archive,
compiled most of the tree again and saved old and new entries together: 962 MB
became 1.79 GB, the janitor deleted it, and every PR built cold (about 24 min
against about 4). tsan and valgrind shared the family budget with asan, so
their archives pushed it out and were deleted unused.
"""

from __future__ import annotations

import datetime as dt
import importlib.util
import json
import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "ci" / "fixtures"
SANITIZERS = (ROOT / ".github/workflows/sanitizers.yaml").read_text(encoding="utf-8")
PUSH = (ROOT / ".github/workflows/push.yaml").read_text(encoding="utf-8")
SIZE_UNITS = {"MB": 1024**2, "GB": 1024**3}


def size(value: str) -> int:
    return int(float(value[:-2]) * SIZE_UNITS[value[-2:]])


def step(text: str, name: str) -> str:
    match = re.search(rf"^ *- name: {name}.*?(?=^ *- |\Z)", text, re.M | re.S)
    assert match, f"no step {name}"
    return match[0]


def job(text: str, name: str) -> str:
    match = re.search(rf"^  {name}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.M | re.S)
    assert match, f"no job {name}"
    return match[1]


def load_state():
    spec = importlib.util.spec_from_file_location("asan_cache_state", ROOT / "ci" / "asan_cache_state.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class FamilyTest(unittest.TestCase):
    def setUp(self):
        self.families = tomllib.loads((ROOT / "ci.toml").read_text(encoding="utf-8"))["cache"]["family"]

    def owner(self, key: str) -> str:
        # Literal prefix classification is first-match at the pinned ci-lint.
        return next(name for name, family in self.families.items()
                    if family.get("prefix") and key.startswith(family["prefix"]))

    def test_asan_archive_has_its_own_budget(self):
        asan = self.owner("zccache-sanitize-asan-ON-seed")
        for key in ("zccache-sanitize-tsan-ON-seed", "zccache-sanitize-valgrind-ON-seed", "zccache-appimage-seed"):
            with self.subTest(key=key):
                self.assertNotEqual(asan, self.owner(key))

    def test_family_holds_the_measured_archive_with_growth_room(self):
        # The limit that deleted the archive was the family's, not a shape's.
        family = self.families[self.owner("zccache-sanitize-asan-ON-seed")]
        snapshot = json.loads((FIXTURES / "sanitize-compiler-cache.json").read_text(encoding="utf-8"))
        for entry in snapshot["entries"]:
            with self.subTest(key=entry["key"]):
                self.assertGreaterEqual(size(family["max"]), 1.25 * entry["size_in_bytes"])


class SanitizersWorkflowTest(unittest.TestCase):
    def test_master_builds_fresh(self):
        restore = step(SANITIZERS, "Restore zccache")
        self.assertIn("github.ref != 'refs/heads/master'", restore)

    def test_only_asan_keeps_an_archive(self):
        self.assertIn("matrix.tool == 'asan'", step(SANITIZERS, "Restore zccache"))
        self.assertIn("matrix.tool == 'asan'", step(SANITIZERS, "Save zccache"))

    def test_build_only_saves_without_running_the_checks(self):
        for name in ("Replay the conversion ratchet", "Triage", "Upload logs"):
            with self.subTest(step=name):
                self.assertIn("!inputs.build_only", step(SANITIZERS, name))
        self.assertIn("SANITIZE_BUILD_ONLY", step(SANITIZERS, "Build, test and launch"))

    def test_a_push_refresh_and_the_nightly_do_not_cancel_each_other(self):
        block = re.search(r"^concurrency:\n((?:  .*\n)+)", SANITIZERS, re.M)[1]
        self.assertIn("github.event_name", block.split("cancel-in-progress")[0])
        self.assertRegex(block, r"cancel-in-progress: \$\{\{ github\.event_name != 'push' \}\}")


class PushWorkflowTest(unittest.TestCase):
    def test_refresh_runs_only_when_the_state_job_asks(self):
        refresh = job(PUSH, "asan-cache-refresh")
        self.assertIn("needs: asan-cache-state", refresh)
        self.assertIn("needs.asan-cache-state.outputs.refresh == 'true'", refresh)
        self.assertIn("uses: ./.github/workflows/sanitizers.yaml", refresh)
        self.assertIn("tools: asan", refresh)
        self.assertIn("build_only: true", refresh)

    def test_state_job_is_master_in_this_repository_only(self):
        state = job(PUSH, "asan-cache-state")
        self.assertIn("github.ref == 'refs/heads/master'", state)
        self.assertIn("github.repository == 'obs-rust/obs-studio'", state)
        self.assertIn("uv run --no-project ci/asan_cache_state.py", state)
        self.assertIn("actions: read", state)


class StateTest(unittest.TestCase):
    NOW = dt.datetime(2026, 11, 2, 16, 0, tzinfo=dt.timezone.utc)

    def entry(self, created: str, ref: str = "refs/heads/master") -> dict:
        return {"key": "zccache-sanitize-asan-ON-abc", "ref": ref, "created_at": created}

    def test_no_archive_asks_for_a_build(self):
        self.assertTrue(load_state().needs_refresh([], self.NOW))

    def test_a_fresh_archive_does_not(self):
        self.assertFalse(load_state().needs_refresh([self.entry("2026-11-02T15:28:45.583172Z")], self.NOW))

    def test_an_old_archive_does(self):
        self.assertTrue(load_state().needs_refresh([self.entry("2026-11-02T09:59:59Z")], self.NOW))

    def test_the_newest_entry_decides(self):
        entries = [self.entry("2026-11-01T09:00:00Z"), self.entry("2026-11-02T15:00:00Z")]
        self.assertFalse(load_state().needs_refresh(entries, self.NOW))

    def test_an_entry_of_another_ref_does_not_count(self):
        entries = [self.entry("2026-11-02T15:00:00Z", "refs/pull/1/merge")]
        self.assertTrue(load_state().needs_refresh(entries, self.NOW))

    def test_a_failed_query_builds_nothing(self):
        state = load_state()
        self.assertFalse(state.decide(lambda: (_ for _ in ()).throw(OSError("no network")), self.NOW))


if __name__ == "__main__":
    unittest.main()
