#!/usr/bin/env -S uv run --script
"""Tier 2 Linux starts the slowest tests first (#931).

    uv run python -m unittest ci/test_tier2_schedule.py

Each OFF/ON ctest run took about 285 s at -j4 for 660 s of test work: every
CI container started without CTestCostData.txt, so ctest did not know that
test_deinterlace_2x_field_order (83 s) and the other real-time media tests
are slow and could start them last. validate.sh now carries the cost data
per mode in the ccache directory, which CI already restores.

The job count stays at 4: under -j8, test_encoder_join_keyframe (a real AV1
encode) and test_media_gif_last_frame failed their timing assertions while
passing 5/5 alone, so more jobs needs per-test PROCESSORS first.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VALIDATE = ROOT / "rust" / "tools" / "linux-validate" / "validate.sh"
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"


class CostDataTest(unittest.TestCase):
    def setUp(self):
        self.text = VALIDATE.read_text(encoding="utf-8")
        start = self.text.index('echo "== [$mode] ctest')
        self.around_ctest = self.text[start - 1500 : start + 1500]
        invocation = re.search(r'^\s*"\$\{known_flaky\[@\]\}" "\$\{mode_env\[@\]\}" xvfb-run [^\n]+ ctest --test-dir',
                               self.text, re.M)
        self.assertIsNotNone(invocation, "validate.sh must invoke CTest under the shared display")
        self.ctest = invocation.start()

    def test_cost_data_is_restored_before_ctest(self):
        restore = self.text.find("CTestCostData.txt")
        ctest = self.ctest
        self.assertNotEqual(restore, -1, "validate.sh never mentions CTestCostData.txt")
        self.assertLess(restore, ctest, "cost data must be in place before ctest runs")

    def test_cost_data_lives_per_mode_in_the_cost_dir(self):
        # OBS_TIER2_COST_DIR, defaulting to the zccache dir (#930).
        self.assertIn('cost_dir="${OBS_TIER2_COST_DIR:-${ZCCACHE_CACHE_DIR:-}}"', self.around_ctest)
        self.assertIn('cost_cache="$cost_dir/ctest-cost-$mode.txt"', self.around_ctest)

    def test_cost_data_is_saved_after_ctest(self):
        ctest = self.ctest
        self.assertRegex(self.text[ctest:], r'cp "\$cost_file" "\$cost_cache"',
                         "cost data must be written back after ctest")


class JobsTest(unittest.TestCase):
    def test_ci_runs_at_most_eight_test_binaries_at_once(self):
        """One runner, at most 8 test binaries at once. The timing tests that
        failed under -j8 (#938) were fixed to judge playback (#1001) and
        pass under that load without reserving processors."""
        helper = WORKFLOW.parents[2] / 'rust/tools/linux-validate/ci-runtime.py'
        m = re.search(r"OBS_CTEST_JOBS=(\d+)", helper.read_text(encoding="utf-8"))
        self.assertIsNotNone(m)
        self.assertLessEqual(int(m.group(1)), 8)


if __name__ == "__main__":
    unittest.main()
