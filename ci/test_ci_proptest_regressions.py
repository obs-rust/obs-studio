#!/usr/bin/env -S uv run --script
"""A red proptest case must survive the runner that found it (#2993).

    uv run python -m unittest ci/test_ci_proptest_regressions.py

proptest saves a failing case to `rust/<crate>/tests/proptest-regressions/<test>.txt`
**on the runner**, and the test re-runs those seeds before generating new ones. The
files are checked in, so a case that reaches master is replayable forever; a case
found by the run that then fails dies with the runner unless the job uploads the
directory. #2988 is that hole: `render::gif_parity::api` failed on
`Rust Tests 🦀 (windows-2025-vs2026)`, the log names
`D:\\a\\...\\rust\\obs-graphics\\tests\\proptest-regressions\\gif_parity.txt`, and the file
no longer exists anywhere.

So every job whose steps run the Rust test lane must keep that directory when it
fails. The seed is also echoed into the job summary, because a reader of a red run
has the log in front of them and should not need to download an artefact to get it.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github" / "workflows" / "build-project.yaml"

# Steps that run the Rust test lane: the workspace/diff tests, the local gate's
# rust-test lane, and the per-crate invocations the Qt SDK check uses.
TEST_RUN = re.compile(
    r"soldr cargo test|local_gate\.py --lane rust-test|local_gate\.py --test-diff"
)
REGRESSIONS = "proptest-regressions"
ARTIFACT_ACTION = "actions/upload-artifact@"
# The three jobs that run the lane today. Pinned so a rename or a new lane is a
# test failure rather than a silently uncovered job (#436 selects them per OS).
EXPECTED_JOBS = {"rust-tests-linux", "rust-tests-macos", "rust-tests-windows"}


def jobs(text: str) -> dict[str, str]:
    """Split the `jobs:` mapping into `{job id: body}`."""
    found = {}
    for m in re.finditer(
        r"^  ([a-z0-9-]+):\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)", text, re.S | re.M
    ):
        found[m.group(1)] = m.group(2)
    return found


def steps(job: str) -> list[str]:
    """Split a job body into step blocks (`      - name: ...`)."""
    return [s for s in re.split(r"^      - ", job, flags=re.M)[1:] if s.strip()]


def field(step: str, name: str) -> str | None:
    """A step key: step keys sit at 8 spaces, `with:` children at 10."""
    m = re.search(rf"^ {{8,10}}{name}: (.*)$", step, re.M)
    return None if m is None else m.group(1).strip()


def runs(step: str) -> str:
    """The step's `run:` body, or "" when it is a `uses:` step."""
    m = re.search(r"^        run: [|>]?\n((?:          .*\n|\n)+)", step, re.M)
    if m:
        return m.group(1)
    m = re.search(r"^        run: (.*)$", step, re.M)
    return "" if m is None else m.group(1)


def test_jobs(text: str) -> set[str]:
    return {
        job_id
        for job_id, body in jobs(text).items()
        if any(TEST_RUN.search(runs(step)) for step in steps(body))
    }


def keepers(body: str) -> list[str]:
    """Steps that upload the regressions directory when the job fails."""
    found = []
    for step in steps(body):
        uses = field(step, "uses") or ""
        if not uses.startswith(ARTIFACT_ACTION):
            continue
        path = field(step, "path") or ""
        if REGRESSIONS not in path:
            continue
        found.append(step)
    return found


class ProptestRegressionsTest(unittest.TestCase):
    maxDiff = None

    def setUp(self):
        self.text = WORKFLOW.read_text(encoding="utf-8")

    def test_every_rust_test_job_keeps_its_regressions(self):
        """A failing lane uploads `rust/*/tests/proptest-regressions/` (#2993)."""
        missing = sorted(test_jobs(self.text) - {j for j, b in jobs(self.text).items() if keepers(b)})
        self.assertEqual(missing, [], "jobs that run the test lane but keep no proptest regressions")

    def test_the_known_lanes_are_covered(self):
        """The lane set is pinned, so a renamed or new job cannot slip out (#436)."""
        self.assertEqual(sorted(test_jobs(self.text)), sorted(EXPECTED_JOBS))

    def test_the_upload_is_pinned_and_bounded(self):
        """Every regressions step is a pinned action with a short retention."""
        for job_id, body in jobs(self.text).items():
            for step in keepers(body):
                uses = field(step, "uses")
                assert uses is not None
                digest = uses.split("@", 1)[1].split()[0]
                self.assertRegex(
                    digest, r"^[0-9a-f]{40}$", f"{job_id}: upload-artifact not pinned to a digest"
                )
                self.assertIsNone(
                    field(step, "continue-on-error"), f"{job_id}: upload may not fail the job"
                )
                self.assertEqual(field(step, "retention-days"), "7", f"{job_id}: retention")
                self.assertEqual(field(step, "if-no-files-found"), "ignore", f"{job_id}: empty upload")

    def test_the_step_runs_only_on_failure(self):
        """A green job uploads nothing: the step is `if: failure()`."""
        for job_id, body in jobs(self.text).items():
            for step in keepers(body):
                condition = field(step, "if") or ""
                self.assertIn("failure()", condition, f"{job_id}: step must be failure-only")

    def test_the_seed_reaches_the_job_summary(self):
        """The new seeds are echoed into `$GITHUB_STEP_SUMMARY` (#2993)."""
        for job_id, body in jobs(self.text).items():
            if not keepers(body):
                continue
            echo = [s for s in steps(body) if "GITHUB_STEP_SUMMARY" in runs(s)]
            self.assertTrue(echo, f"{job_id}: no step writes the new seeds to the job summary")
            for step in echo:
                self.assertIn("failure()", field(step, "if") or "", f"{job_id}: summary step")


if __name__ == "__main__":
    unittest.main()
