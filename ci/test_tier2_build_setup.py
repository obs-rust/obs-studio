"""Tier 2 Linux build setup: a bounded zccache and no unused cook (#3026, #3027)."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"


def build_job():
    text = WORKFLOW.read_text()
    return re.search(r"^  rust-linux-tier2-build:\n(.*?)(?=^  [a-z0-9-]+:\n)", text, re.M | re.S)[1]


def step(job, name):
    return re.search(rf"- name: {re.escape(name)}.*?(?=\n      - |\Z)", job, re.S)[0]


class Tier2BuildSetupTest(unittest.TestCase):
    def test_bound_reads_the_container_workspace(self):
        # The job runs in a container: ${{ github.workspace }} in a run
        # script is the host path, absent there, so the bound never ran and
        # the restored archive grew to 2.1 GB for a 1.3 GB build (#3026).
        bound = step(build_job(), "Bound zccache 🧹")
        run = bound.split("run: |", 1)[1]
        self.assertNotIn("github.workspace", run)
        self.assertIn('"$GITHUB_WORKSPACE/.tier2-zccache"', run)
        self.assertIn("du -sm", run)

    def test_bound_reports_size_on_every_run(self):
        bound = step(build_job(), "Bound zccache 🧹")
        self.assertNotIn("if: github.event_name", bound.split("run: |", 1)[0])
        self.assertIn("refs/heads/master", bound.split("run: |", 1)[1])

    def test_no_cook_for_corrosion_build(self):
        # Corrosion builds in its own target dir; the cooked workspace
        # target/ was loaded (12 s) and never used (#3027).
        soldr = step(build_job(), "Set Up soldr 🦀")
        self.assertRegex(soldr, r"\n\s+prebuild-deps: none\n")
        self.assertNotIn("prebuild-deps-flags", soldr)
        self.assertRegex(soldr, r"\n\s+cargo-registry-cache: true\n")


if __name__ == "__main__":
    unittest.main()
