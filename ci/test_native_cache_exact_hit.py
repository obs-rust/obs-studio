"""Native app caches must not repack an immutable exact-hit archive."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


class NativeCacheExactHitTest(unittest.TestCase):
    def test_existing_app_snapshots_skip_save_but_preserve_other_guards(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        for job, restore_id, prefix, existing_guard in (
            ("macos-build", "macos-compiler-cache", "${{ runner.os }}-xcode-",
             "github.ref == 'refs/heads/master'"),
            ("windows-build", "windows-compiler-cache", "windows-ccache-",
             "github.ref == 'refs/heads/master'"),
            ("flatpak-build", "flatpak-state-cache", "flatpak-builder-state-v1-x86_64-",
             "steps.ccache-stats.outcome == 'success'"),
        ):
            with self.subTest(job=job):
                block = re.search(rf"^  {job}:\n(.*?)(?=^  [a-z0-9-]+:|\Z)",
                                  workflow, re.M | re.S).group(1)
                steps = re.split(r"(?m)^      - ", block)
                restore = next(s for s in steps if "uses: actions/cache/restore@" in s
                               and f"key: {prefix}" in s)
                save = next(s for s in steps if "uses: actions/cache/save@" in s
                            and f"key: {prefix}" in s)
                self.assertIn(f"id: {restore_id}", restore)
                guard = re.search(r"^        if: (.+)$", save, re.M).group(1)
                self.assertIn(f"steps.{restore_id}.outputs.cache-hit != 'true'", guard,
                              "Fallback/miss may publish; an exact immutable key cannot")
                self.assertIn(existing_guard, guard, "Keep the existing save policy")
                self.assertEqual(re.search(r"^          key: (.+)$", restore, re.M).group(1),
                                 re.search(r"^          key: (.+)$", save, re.M).group(1))


if __name__ == "__main__":
    unittest.main()
