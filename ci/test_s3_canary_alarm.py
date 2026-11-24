#!/usr/bin/env -S uv run --script
"""A canary that cannot fail its own run is not a canary.

`s3-cache-canary.yaml` carried a job-level `continue-on-error: true`, so the
workflow run reported **success** even when the canary job failed. The script
already distinguishes the two cases that matter -- absent credentials exit 0
with a warning, a bucket that will not take a write exits 1 -- so the job-level
flag could only ever hide a real outage.

Same failure mode as the pool signal (forge-ci/gh-runner#90): instrumentation
that says `ok` while being useless. Pinned here so it cannot come back.
"""
from __future__ import annotations

from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parent.parent
CANARY = ROOT / '.github' / 'workflows' / 's3-cache-canary.yaml'
SCRIPT = ROOT / 'ci' / 's3_cache_canary.sh'


class S3CanaryAlarmTest(unittest.TestCase):
    def test_the_job_may_fail_its_own_run(self):
        # The job carries a comment explaining why the flag is absent, so scan
        # the block with comments stripped instead of matching raw text --
        # and without PyYAML, which ci/ tests may not import (#1196).
        text = CANARY.read_text()
        match = re.search(r'^  canary:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)', text, re.M | re.S)
        assert match is not None, 'no canary job in the workflow'
        canary = match[1]
        keys = [line for line in canary.splitlines()
                if line.strip() and not line.lstrip().startswith('#')]
        self.assertFalse([line for line in keys if re.match(r'\s*continue-on-error:', line)],
                         'a job-level continue-on-error makes the run green even when the canary fails')

    def test_missing_credentials_are_not_a_failure(self):
        # The one case that must stay quiet: a fork or a repo without the
        # secret should not go red every day.
        script = SCRIPT.read_text()
        self.assertIn('credentials are not configured', script)
        self.assertIn('exit 0', script)

    def test_a_failed_write_still_exits_non_zero(self):
        script = SCRIPT.read_text()
        self.assertIn('S3 cache write failed', script)
        self.assertIn('exit 1', script)


if __name__ == '__main__':
    unittest.main()
