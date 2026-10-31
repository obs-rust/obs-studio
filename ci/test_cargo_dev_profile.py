#!/usr/bin/env -S uv run --script
"""Proptest's RNG dependencies stay optimized in the dev test profile (#3172).

    uv run python -m unittest ci/test_cargo_dev_profile.py

The debug test profile builds dependencies at opt-level 0. The parity proptests
spend most of their time in proptest's ChaCha RNG, so `proptest`, `rand_core`,
`rand_chacha` and `ppv-lite86` carry opt-level 3 overrides in the workspace
manifest. Dropping one silently gives the RNG its unoptimized inner loops back.
"""

from __future__ import annotations

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RNG_PACKAGES = ("proptest", "rand_core", "rand_chacha", "ppv-lite86")


class CargoDevProfileTest(unittest.TestCase):
    def setUp(self):
        manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
        self.dev_packages = (
            manifest.get("profile", {}).get("dev", {}).get("package", {})
        )

    def test_proptest_rng_dependencies_are_optimized(self):
        errors = [
            f"[profile.dev.package.{name}] opt-level = 3 is missing"
            for name in RNG_PACKAGES
            if self.dev_packages.get(name, {}).get("opt-level") != 3
        ]
        self.assertEqual(errors, [], "\n".join(errors))


if __name__ == "__main__":
    unittest.main()
