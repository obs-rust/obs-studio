"""Queue compiler objects must not share nightly-only LRU protection."""
from pathlib import Path
import json
import re
import tomllib
import unittest


ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / "ci" / "fixtures"
MIB = 1024 ** 2


def size(value):
    units = {"MB": MIB, "GB": 1024 ** 3}
    return int(float(value[:-2]) * units[value[-2:]])


class Tier2RetentionTest(unittest.TestCase):
    def setUp(self):
        self.families = tomllib.loads((ROOT / "ci.toml").read_text())["cache"]["family"]

    def owner(self, key):
        # Literal prefix classification is first-match at the pinned ci-lint.
        return next(name for name, family in self.families.items()
                    if family.get("prefix") and key.startswith(family["prefix"]))

    def test_queue_objects_have_independent_lru_protection(self):
        owner = self.owner("zccache-tier2-linux-both-seed")
        for key in ("zccache-ubuntu-seed", "zccache-appimage-seed", "zccache-sanitize-seed"):
            with self.subTest(key=key):
                self.assertNotEqual(owner, self.owner(key))
        family = self.families[owner]
        self.assertEqual(family["evict"], "lru")
        self.assertGreaterEqual(size(family["max"]), 475427223)

    def test_reallocation_does_not_expand_combined_allowance(self):
        owners = {self.owner(key) for key in ("zccache-tier2-linux-both-seed", "zccache-ubuntu-seed",
                                             "zccache-appimage-seed", "zccache-sanitize-seed")}
        footprint = 0
        for owner in owners:
            family = self.families[owner]
            footprint += (sum(size(shape["max"]) for shape in family["shapes"].values())
                          if family.get("shapes") else size(family["max"]))
        # Ubuntu gains capacity from measured over-sized native/dependency
        # allowances, rather than adding to the repository's overcommit.
        donors = ("windows-tier2-ccache", "macos-tier2-xcas", "libs-qt", "libs-ffmpeg")
        footprint += sum(size(self.families[name]["max"]) for name in donors)
        self.assertLessEqual(footprint, (1700 + 800 + 500 + 1024 + 500) * MIB)

    def test_tier2_keeps_both_libobs_modes(self):
        # The Ubuntu app build left the Actions cache (#1801 phase A); the
        # C-reference and hybrid coverage stays in Tier 2.
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        tier2 = re.search(r"^  rust-linux-tier2:\n(.*?)(?=^  [a-z0-9-]+:\n)", workflow, re.M | re.S)[1]
        self.assertRegex(tier2, r'mode: \["OFF", "ON"\]')

    def test_reduced_allowances_retain_measured_seeds_with_growth_room(self):
        snapshot = json.loads((FIXTURES / "cache-retention-families.json").read_text())
        for name in ("windows-tier2-ccache", "macos-tier2-xcas", "libs-qt", "libs-ffmpeg"):
            with self.subTest(family=name):
                measured = snapshot["families"][name]["listed_bytes"]
                self.assertGreater(measured, 0)
                self.assertGreaterEqual(size(self.families[name]["max"]), 2 * measured)

    def test_queue_compiler_caches_hold_their_measured_entry(self):
        # A max below the live entry makes the janitor evict the queue's warm
        # cache (ci-lint cache audit CACHE-004). Keep 25% growth room.
        snapshot = json.loads((FIXTURES / "queue-compiler-caches.json").read_text())
        for entry in snapshot["entries"]:
            name = self.owner(entry["key"])
            with self.subTest(family=name):
                self.assertGreaterEqual(size(self.families[name]["max"]), 1.25 * entry["size_in_bytes"])


if __name__ == "__main__":
    unittest.main()
