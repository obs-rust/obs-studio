#!/usr/bin/env -S uv run --script
"""appimage.yaml's PR cache saves carry a pr-<N> component (#962).

    uv run python -m unittest ci/test_cache013_appimage.py

ci-lint's CACHE-013 flags a cache-writing step that can save in PR context whose
key has no delimited ``pr-<N>`` component: the cache janitor reclaims a closed
PR's entries by that component, so an unsuffixed PR save is never deleted.
``.github/workflows/appimage.yaml`` carried the finding as one ``[[exceptions]]``
block in ci.toml's baseline, for three ``astral-sh/setup-uv`` steps (no
``cache-suffix``) and three direct ``actions/cache/save`` keys (no ``pr-<N>``).
Each now carries the same ``pr-<N>`` expression and the exception block is gone.

ci-lint needs the network, so the rule is mirrored here over the workflow text
and ci.toml.

RED first: this file was committed with the unsuffixed steps and the exception
block still present, and failed on both tests.
"""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "appimage.yaml"

# The delimited pr-<N> component the janitor deletes a closed PR's entries by.
SUFFIX = "format('pr-{0}', github.event.pull_request.number)"

# The cache-writing steps in appimage.yaml, by their key/naming stem: the three
# setup-uv steps concatenate the suffix onto the cache key, and the three direct
# actions/cache/save keys end with it.
SETUP_UV_STEPS = 3
CACHE_KEYS = (
    "zccache-appimage-${{ github.run_id }}",
    "cargo-appimage-${{ hashFiles('Cargo.lock', 'rust-toolchain.toml') }}-${{ github.run_id }}",
    "testpkgs-${{ matrix.image }}-${{ hashFiles('build-aux/appimage/test-appimage.sh') }}",
)


class Cache013AppimageTest(unittest.TestCase):
    def test_every_setup_uv_step_is_suffixed(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        self.assertEqual(
            text.count("astral-sh/setup-uv@"), SETUP_UV_STEPS, "appimage.yaml setup-uv steps"
        )
        self.assertEqual(
            text.count("cache-suffix:"), SETUP_UV_STEPS, "each setup-uv step needs a cache-suffix"
        )
        # Three setup-uv suffixes, plus the same key on each cache's restore and
        # save step (three stems x two).
        self.assertEqual(text.count(SUFFIX), SETUP_UV_STEPS + 2 * len(CACHE_KEYS))

    def test_every_cache_key_carries_the_pr_n_component(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        for stem in CACHE_KEYS:
            # Restore and save use the same key, so each stem appears twice and
            # each occurrence must end with the pr-<N> component.
            self.assertEqual(text.count(stem), 2, stem)
            self.assertIn(f"{stem}-${{{{ github.event.pull_request.number && {SUFFIX} || '' }}}}", text, stem)

    def test_no_cache013_exception_left(self):
        data = tomllib.loads((ROOT / "ci.toml").read_text(encoding="utf-8"))
        stale = [
            entry
            for entry in data.get("exceptions", [])
            if entry.get("rule") == "CACHE-013"
            and entry.get("path") == ".github/workflows/appimage.yaml"
        ]
        self.assertEqual(
            stale,
            [],
            "the CACHE-013 exception for appimage.yaml must be deleted",
        )


if __name__ == "__main__":
    unittest.main()
