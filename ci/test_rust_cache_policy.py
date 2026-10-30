"""Every remote Rust consumer retains the stale-source safeguard (#1286)."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOWS = ("build-project.yaml", "release-rust.yaml")


class RustCachePolicyTest(unittest.TestCase):
    def test_bypassed_compiler_store_is_not_transferred(self):
        found = cook_free = 0
        for name in WORKFLOWS:
            text = (ROOT / ".github/workflows" / name).read_text()
            for step in re.split(r"(?m)^      - ", text):
                if "uses: zackees/setup-soldr@" not in step:
                    continue
                found += 1
                with self.subTest(workflow=name, callsite=found):
                    self.assertRegex(step, r"(?m)^          build-cache: false$",
                                     "Do not transfer an embedded store while its Rust consumer is bypassed")
                    self.assertNotRegex(step, r"(?m)^          cache: false$",
                                        "Preserve the other cache layers")
                    if "(#3027)" in step:
                        # Tier 2 Linux builds Rust only through Corrosion, in
                        # its own target dir: a cook there is never reused.
                        cook_free += 1
                        self.assertIn("prebuild-deps: none", step)
                        continue
                    self.assertIn("prebuild-deps-flags:", step)
                    self.assertNotIn("prebuild-deps: none", step)
        self.assertEqual(found, 12, "Audit new setup-soldr consumers explicitly")
        self.assertEqual(cook_free, 1, "Audit each consumer that skips cook explicitly")

    def test_all_consumers_inherit_the_rust_wrapper_bypass(self):
        for name in WORKFLOWS:
            with self.subTest(workflow=name):
                text = (ROOT / ".github/workflows" / name).read_text()
                # Workflow-level env reaches setup/cook and every native matrix
                # consumer, including the Linux job-container build.
                env = re.search(r"^env:\n((?:[ \t].*\n|\n)+)", text, re.M)
                self.assertIsNotNone(env, "Rust consumers need a shared safeguard")
                self.assertRegex(env[1], r"(?m)^  SOLDR_RUSTC_WRAPPER: none$")
                overrides = re.findall(r"(?m)^\s+SOLDR_RUSTC_WRAPPER:\s*(\S+)", text)
                self.assertTrue(overrides)
                self.assertEqual(set(overrides), {"none"}, "Do not re-enable an unsafe wrapper in a child job")

    def test_rust_bypass_does_not_disable_native_compiler_caches(self):
        for name in WORKFLOWS:
            with self.subTest(workflow=name):
                text = (ROOT / ".github/workflows" / name).read_text()
                self.assertNotRegex(text, r"(?m)^\s+ZCCACHE_DISABLE:")
                self.assertNotRegex(text, r"(?m)^\s+CCACHE_DISABLE:")


if __name__ == "__main__":
    unittest.main()
