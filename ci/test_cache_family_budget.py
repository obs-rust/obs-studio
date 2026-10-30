#!/usr/bin/env -S uv run --script
"""The declared cache families fit in [cache].budget (#1801).

    uv run python -m unittest ci/test_cache_family_budget.py

The cache janitor keeps each family up to its declared size, so the sum of
those sizes is what the repository cache holds in its steady state. When the
sum is more than the budget, the janitor cannot keep every family, and GitHub
evicts the least recently used entries: on 2026-10-27 and 2026-10-30 those
were the Linux and Windows cook layers (#1801).

This is the CACHE-004 arithmetic of the pinned ci-lint
(zackees/ci.yml ci_lint/rules/cache_static.py, check_cache_004): a family with
`shapes` counts the sum of its shape maxima, any other family `max` times the
cardinality of `per`; a `lockfile` family counts twice (the lockfile-change
peak) unless every writer flow pre-prunes; [cache.pr].budget is added.
Sizes use binary units, as ci-lint's parse_size does. `ci-lint precheck`
reports the same rule but needs the network and fails on other rules too, so
this offline test is what gates it.
"""

from __future__ import annotations

import importlib.util
import json
import re
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI_TOML = ROOT / "ci.toml"
FIXTURES = ROOT / "ci" / "fixtures"

_SPEC = importlib.util.spec_from_file_location("ci_toml_checks", ROOT / "ci" / "test_ci_toml.py")
CI_TOML_CHECKS = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(CI_TOML_CHECKS)

# Phase A of #1801 (Decided 2026-10-30): the six app-build families leave the
# Actions cache. They held 5.71 GiB and hit 9 times in 42 restores, all on
# nightly, dispatch and release runs; deps, zccache-tier2 and tier2-apt serve
# every PR and queue run. The release-asset transport (#782) can bring them
# back outside the budget.
RETIRED_APP_FAMILIES = {
    "flatpak-builder": "flatpak-builder-",
    "windows-ccache-x64": "windows-ccache-",
    "windows-ccache-arm64": "windows-ccache-",
    "zccache-ubuntu": "zccache-ubuntu-",
    "xcode-x86_64": "macOS-xcode-",
    "xcode-arm64": "macOS-xcode-",
}

SIZE_RE = re.compile(r"^(\d+(?:\.\d+)?)\s*(B|KB|MB|GB)$", re.IGNORECASE)
SIZE_UNITS = {"B": 1, "KB": 1024, "MB": 1024**2, "GB": 1024**3}


def parse_size(text: str) -> int:
    match = SIZE_RE.match(text.strip())
    if match is None:
        raise ValueError(f"not a size: {text!r}")
    return int(float(match[1]) * SIZE_UNITS[match[2].upper()])


def cardinality(config: dict, per: str | None) -> int:
    platforms = config["platforms"]
    if per == "platform":
        return len(platforms)
    if per == "cross-platform":
        return max(len(platforms) - 1, 0)
    if per == "os":
        return len({platform["group"] for platform in platforms.values()})
    return 1


def footprint(config: dict, family: dict) -> int:
    if family.get("shapes"):
        return sum(parse_size(shape["max"]) for shape in family["shapes"].values())
    return parse_size(family["max"]) * cardinality(config, family.get("per"))


def worst_case(config: dict) -> tuple[int, dict[str, int]]:
    cache = config["cache"]
    sizes = {name: footprint(config, family) for name, family in cache["family"].items()}
    lockfile = sum(sizes[name] for name, family in cache["family"].items() if family.get("lockfile"))
    writers = [flow for flow in config.get("flows", {}).values() if flow.get("cache") == "write"]
    if writers and all(flow.get("pre-prune") for flow in writers):
        lockfile = 0
    pr_budget = parse_size(cache["pr"]["budget"]) if cache.get("pr", {}).get("budget") else 0
    return sum(sizes.values()) + lockfile + pr_budget, sizes


class CacheFamilyBudgetTest(unittest.TestCase):
    def test_declared_families_fit_in_the_budget(self):
        config = tomllib.loads(CI_TOML.read_text())
        budget = parse_size(config["cache"]["budget"])
        worst, sizes = worst_case(config)
        largest = ", ".join(
            f"{name} {size / 1024**3:.2f} GiB" for name, size in sorted(sizes.items(), key=lambda item: -item[1])[:6]
        )
        self.assertLessEqual(
            worst,
            budget,
            f"ci.toml cache families need {worst} B, budget is {budget} B; largest: {largest}",
        )

    def test_budget_is_the_repository_cap(self):
        config = tomllib.loads(CI_TOML.read_text())
        self.assertLessEqual(parse_size(config["cache"]["budget"]), 10 * 1024**3)

    def test_sanitize_shape_holds_the_measured_asan_archive(self):
        # A shape max below the live entry makes the janitor evict it, and the
        # PR asan job builds cold. Keep 25 % growth room, as the other
        # measured compiler caches do.
        config = tomllib.loads(CI_TOML.read_text())
        shape = config["cache"]["family"]["zccache"]["shapes"]["sanitize"]
        snapshot = json.loads((FIXTURES / "sanitize-compiler-cache.json").read_text())
        for entry in snapshot["entries"]:
            with self.subTest(key=entry["key"]):
                self.assertGreaterEqual(parse_size(shape["max"]), 1.25 * entry["size_in_bytes"])

    def test_app_build_families_are_retired_with_their_consumers(self):
        families = tomllib.loads(CI_TOML.read_text())["cache"]["family"]
        keys = CI_TOML_CHECKS.cache_keys()
        for name, prefix in RETIRED_APP_FAMILIES.items():
            with self.subTest(family=name):
                self.assertFalse(name in families, "a retired family must not hold declared capacity")
                self.assertEqual([path for path, key in keys if key.startswith(prefix)], [],
                                 "retire a family together with every save and restore of its keys")

    def test_arithmetic_matches_ci_lint(self):
        # ci-lint's own output for the master of 2026-10-30 (precheck --local):
        # deps "3 shapes [linux=1.4GB, macos=400MB, windows=650MB] = 2604243353 B",
        # registry "60MB x 3 (platform) = 188743680 B".
        config = {"platforms": {"a": {"group": "x"}, "b": {"group": "y"}, "c": {"group": "z"}}}
        deps = {"max": "900MB", "shapes": {"linux": {"max": "1.4GB"}, "macos": {"max": "400MB"}, "windows": {"max": "650MB"}}}
        self.assertEqual(footprint(config, deps), 2604243353)
        self.assertEqual(footprint(config, {"max": "60MB", "per": "platform"}), 188743680)
        self.assertEqual(footprint(config, {"max": "60MB"}), 62914560)


if __name__ == "__main__":
    unittest.main()
