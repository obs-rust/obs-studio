#!/usr/bin/env -S uv run --script
"""Packaged virtual-camera hardware qualification stays outside PR CI (#4749).

Parse the workflow's event/job blocks with the stdlib-only schedule-guard
helpers used by CI self-tests. This checks routing, not hardware behavior.
"""

import re
import unittest
from pathlib import Path

from test_workflow_schedule_guard import GUARD, condition, jobs

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/vcam-audio-hardware.yaml"
CANONICAL = "obs-rust/obs-studio"


class VcamAudioHardwareEventsTest(unittest.TestCase):
    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")
        self.on = re.search(r"^on:\n(.*?)(?=^\S)", self.text, re.M | re.S).group(1)
        self.events = set(re.findall(r"^  ([\w_]+):", self.on, re.M))
        self.jobs = jobs(self.text)

    def scheduled_jobs(self, event, repository):
        if event not in self.events:
            return set()
        selected = set()
        for name, job in self.jobs.items():
            guard = condition(job)
            # Fail closed on an expression this focused policy parser cannot
            # evaluate; never silently treat a new condition as passing.
            self.assertIn(guard, ("", GUARD))
            if not guard or event != "schedule" or repository == CANONICAL:
                selected.add(name)
        return selected

    def test_event_matrix(self):
        cases = (
            ("pull_request", CANONICAL, set()),  # even hardware harness/workflow edits
            ("merge_group", CANONICAL, set()),
            ("schedule", CANONICAL, {"windows", "macos"}),
            ("schedule", "contributor/obs-studio", set()),
            ("workflow_dispatch", CANONICAL, {"windows", "macos"}),
            ("workflow_dispatch", "contributor/obs-studio", {"windows", "macos"}),
        )
        for event, repository, expected in cases:
            with self.subTest(event=event, repository=repository):
                self.assertEqual(self.scheduled_jobs(event, repository), expected)

    def test_only_daily_and_manual_events(self):
        self.assertEqual(self.events, {"schedule", "workflow_dispatch"})
        self.assertEqual(re.findall(r"^    - cron: ['\"]([^'\"]+)['\"]$", self.on, re.M),
                         ["17 10 * * *"])
        self.assertEqual(set(self.jobs), {"windows", "macos"})

    def test_manual_build_run_is_optional_and_forwarded(self):
        dispatch = self.on.split("  workflow_dispatch:\n", 1)[1].split("\n  schedule:", 1)[0]
        self.assertRegex(dispatch, r"(?m)^      build_run:\n")
        self.assertIn("        type: string\n", dispatch)
        self.assertIn("        default: ''\n", dispatch)
        self.assertNotIn("required: true", dispatch)
        self.assertIn("  BUILD_RUN: ${{ inputs.build_run }}\n", self.text)
        for build_run in ("43309616180", ""):
            with self.subTest(build_run=build_run):
                self.assertEqual(self.scheduled_jobs("workflow_dispatch", CANONICAL),
                                 {"windows", "macos"})
        self.assertIn("$run = $env:BUILD_RUN\n", self.jobs["windows"])
        self.assertIn("if (-not $run) {", self.jobs["windows"])
        self.assertIn('run="$BUILD_RUN"\n', self.jobs["macos"])
        self.assertIn('if [ -z "$run" ]; then', self.jobs["macos"])
        for job in self.jobs.values():
            self.assertIn("--workflow scheduled.yaml --branch master", job)
            self.assertIn("--status success --limit 1 --json databaseId", job)
            self.assertIn("Testing $name from run $run", job)
            self.assertIn("GITHUB_STEP_SUMMARY", job)
            self.assertIn("gh run download", job)

    def test_runtime_checks_and_failure_evidence_remain(self):
        for name, device, recording in (
            ("windows", "CABLE Output", "cable-output.wav"),
            ("macos", "OBS Virtual Microphone", "obs-virtual-microphone.wav"),
        ):
            with self.subTest(job=name):
                job = self.jobs[name]
                self.assertEqual(condition(job), GUARD)
                self.assertRegex(job, r"(?m)^    timeout-minutes: 30$")
                self.assertNotRegex(job, r"(?m)^    (?:needs|continue-on-error):")
                steps = re.split(r"^      - ", job, flags=re.M)[1:]
                record = next(step for step in steps if f"record --device '{device}'" in step)
                self.assertNotRegex(record, r"(?m)^        (?:if|continue-on-error):")
                self.assertIn(recording, record)
                upload = next(step for step in steps if "uses: actions/upload-artifact@" in step)
                self.assertIn("        if: always()\n", upload)
                self.assertIn(recording, upload)
        self.assertIn("ci/vcam_audio_hw.py profile --config-root", self.jobs["windows"])
        self.assertIn("the Virtual Camera did not route track 1 to CABLE Input", self.jobs["windows"])
        self.assertIn("Core Audio does not list the OBS Virtual Microphone", self.jobs["macos"])
        self.assertIn("vmic_shm_write(shm, buf, 480, mach_absolute_time())", self.jobs["macos"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
