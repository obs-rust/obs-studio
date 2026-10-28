#!/usr/bin/env -S uv run --script
"""The Linux Qt fast-path job pins its prebuilt libs by tag and sha256 (#992).

    uv run python -m unittest ci/test_qt_fastpath.py
"""

from __future__ import annotations

import re
import itertools
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = (ROOT / ".github/workflows/build-project.yaml").read_text()


class QtFastPathTest(unittest.TestCase):
    def job(self) -> str:
        m = re.search(r"\n  linux-qt-frontend:\n(.*?)(?=\n  [a-z0-9-]+:\n|\Z)", WORKFLOW, re.S)
        self.assertIsNotNone(m, "linux-qt-frontend job missing")
        return m.group(1)

    def test_pins(self):
        job = self.job()
        for tag, asset in [("qt6-6.11.1", "qt6-6.11.1-linux-x86_64-glibc2.28-release.tar.zst"),
                           ("ffmpeg-8.1.2", "ffmpeg-8.1.2-linux-x86_64-glibc2.28-release.tar.zst")]:
            self.assertIn(tag, job)
            self.assertIn(asset, job)
        self.assertEqual(len(set(re.findall(r"\b[0-9a-f]{64}\b", job))), 2)

    def test_caches_by_sha_and_skips_out_of_scope(self):
        job = self.job()
        self.assertRegex(job, r"uses: (?:actions/cache/restore@|\./\.github/actions/cache)")
        self.assertRegex(job, r"uses: (?:actions/cache/save@|\./\.github/actions/cache)")
        for key in ("libs-qt-${{ env.QT_SHA256 }}", "libs-ffmpeg-${{ env.FFMPEG_SHA256 }}"):
            self.assertEqual(job.count(f"key: {key}"), 2, "restore and save must use the same dependency identity")
        self.assertRegex(job, r"key: .*(QT_SHA256|\b[0-9a-f]{64}\b)")
        self.assertIn("needs.check-event.outputs.qt", job)

    def test_no_distro_qt(self):
        self.assertNotRegex(self.job(), r"qt6-(base|svg)-dev")

    def test_qt_starts_after_scope_without_waiting_for_fast_gate(self):
        self.assertRegex(self.job(), r"(?m)^    needs: \[check-event\]$")
        self.assertNotIn("needs.rust-tests-linux", self.job())

    def test_scope_failure_cancellation_and_out_of_scope_skip_qt(self):
        condition = re.search(r"(?m)^    if: \$\{\{ (.*?) \}\}$", self.job()).group(1)
        # Evaluate only this job's small boolean condition; unexpected syntax
        # fails the test rather than executing arbitrary workflow expressions.
        for status, scoped, cancelled in itertools.product(
            ("success", "failure", "cancelled", "skipped"), (True, False), (True, False)
        ):
            with self.subTest(status=status, scoped=scoped, cancelled=cancelled):
                expression = condition.replace("needs.check-event.result == 'success'", str(status == "success"))
                expression = expression.replace("fromJSON(needs.check-event.outputs.qt)", str(scoped))
                expression = expression.replace("cancelled()", str(cancelled))
                expression = expression.replace("&&", " and ").replace("!", "not ").strip()
                self.assertRegex(expression, r"^(?:True|False|and|not|\s|\(|\))+$")
                actual = eval(expression, {"__builtins__": {}})
                self.assertEqual(actual, status == "success" and scoped and not cancelled)

    def test_required_gate_still_rejects_either_build_failure(self):
        gate = (ROOT / ".github/workflows/ci.yml").read_text().split("  ci-gate:", 1)[1]
        self.assertIn("build-project", gate.split("steps:", 1)[0])
        self.assertIn("contains(needs.*.result, 'failure')", gate)
        self.assertIn("contains(needs.*.result, 'cancelled')", gate)
        self.assertNotIn("continue-on-error", self.job())


if __name__ == "__main__":
    unittest.main()
