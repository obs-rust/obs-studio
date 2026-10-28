"""Scoped plugin caches need a compatible trusted writer (#1286)."""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class TouchedTargetCacheTest(unittest.TestCase):
    def test_master_warms_the_same_configuration_that_prs_restore(self):
        workflow = (ROOT / ".github/workflows/build-project.yaml").read_text()
        job = re.search(r"^  rust-tests-linux:\n(.*?)(?=^  [a-z0-9-]+:\n)", workflow, re.M | re.S).group(1)
        build = job.split("      - name: Build Touched Linux Targets", 1)[1].split("      # Skipped", 1)[0]
        self.assertIn("github.event_name == 'push'", build.split("        env:", 1)[0])
        self.assertIn("github.ref == 'refs/heads/master'", build.split("        env:", 1)[0])
        self.assertIn("name: Save Touched Target ccache", build)
        self.assertRegex(build, r"uses: (?:actions/cache/save@|\./\.github/actions/cache)")
        self.assertIn("key: targets-ccache-linux-${{ github.sha }}", build)
        self.assertIn("ccache --zero-stats", build)
        self.assertIn("ccache --show-stats", build)
        self.assertIn("ccache --max-size=250M", build)
        save = build.split("name: Save Touched Target ccache", 1)[1]
        self.assertIn("github.event_name == 'push' && github.ref == 'refs/heads/master'", save)


if __name__ == "__main__":
    unittest.main()
