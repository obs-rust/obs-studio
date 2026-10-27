#!/usr/bin/env -S uv run --script
"""The composite actions' setup-uv PR saves carry a pr-<N> cache suffix (#962).

    uv run python -m unittest ci/test_cache013_actions.py

ci-lint's CACHE-013 flags an ``astral-sh/setup-uv`` or ``actions/cache/save`` step
that can save a cache in PR context whose key has no delimited ``pr-<N>``
component: the cache janitor reclaims a closed PR's entries by that component,
so an unsuffixed PR save is never deleted. #2590 scoped the workflows; the
composite actions still carried the finding as ``[[exceptions]]`` in ci.toml's
baseline. Each setup-uv step now carries the same ``cache-suffix`` expression and
its exception block is gone; ``services-validator`` needed a second edit -- its
``Save Timestamp Cache`` step writes an ``actions/cache/save`` key, and the
exception is path-scoped, so both findings had to clear before the block left.

ci-lint needs the network, so the rule is mirrored here over the action text and
ci.toml.

RED first: this file was committed with the three unsuffixed setup-uv steps and
their exception blocks still present, and failed on both tests.
"""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ACTIONS = ROOT / ".github" / "actions"

# The delimited pr-<N> component the janitor deletes a closed PR's entries by.
SUFFIX = "format('pr-{0}', github.event.pull_request.number)"

TARGETS = (
    "compatibility-validator/action.yaml",
    "flatpak-manifest-validator/action.yaml",
    "package-obs/action.yaml",
    "services-validator/action.yaml",
)

# services-validator's second CACHE-013 finding: its ``Save Timestamp Cache``
# step writes an actions/cache/save key, which must carry the same pr-<N>
# component (the janitor keys on it, not on the step's own if-guard).
TIMESTAMP_SAVE = "services-validator/action.yaml"


class Cache013CompositeActionsTest(unittest.TestCase):
    def test_setup_uv_saves_are_suffixed(self):
        for rel in TARGETS:
            text = (ACTIONS / rel).read_text(encoding="utf-8")
            self.assertIn("cache-suffix:", text, rel)
            self.assertIn(SUFFIX, text, rel)

    def test_timestamp_save_key_is_suffixed(self):
        text = (ACTIONS / TIMESTAMP_SAVE).read_text(encoding="utf-8")
        after_save = text.split("Save Timestamp Cache", 1)[1]
        self.assertIn(
            SUFFIX,
            after_save,
            "the timestamp cache save key needs the pr-<N> component",
        )

    def test_no_cache013_exception_left(self):
        data = tomllib.loads((ROOT / "ci.toml").read_text(encoding="utf-8"))
        paths = {f".github/actions/{rel}" for rel in TARGETS}
        stale = [
            entry
            for entry in data.get("exceptions", [])
            if entry.get("rule") == "CACHE-013" and entry.get("path") in paths
        ]
        self.assertEqual(
            stale,
            [],
            "the CACHE-013 exceptions for the composite actions must be deleted",
        )


if __name__ == "__main__":
    unittest.main()
