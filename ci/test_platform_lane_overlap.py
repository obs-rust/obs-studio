"""Verify the macOS/Windows Rust lanes start with Fast Gate (#2985).

Fast Gate is a full lint plus Rust-test lane on Linux, measured at 1.8-8.6 min
on a merge-queue run and 10.4-11.2 min on the PR run that #2593 recorded. The
macOS and Windows Rust lanes used to `needs: rust-tests-linux`, so they were
created only after it finished even though they consume none of its output:
the queue run 40993230941 spent 3.6 min of its 14.4 min platform-lane critical
path waiting for the gate, and every merge paid it.

This is the same decision `rust-linux-tier2-build` already carries
("compilation consumes no Fast Gate output and can overlap its runner",
ci/test_tier2_compile_overlap.py): a lane that only needs the diff scope, the
runner and the toolchain is not made to wait for an unrelated lint lane.

Merge protection does not move: a failed Fast Gate still fails the run, because
the CI Gate step lists every job of the run attempt (ci/gate_jobs.py) and the
caller's required check sees the failure. What changes is only *when* the
platform runners start.
"""

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"
CALLER = ROOT / ".github/workflows/ci.yml"

PLATFORM_LANES = (
    "rust-tests-macos",
    "rust-tests-windows",
    "rust-tier2-macos",
    "rust-tier2-windows",
)
# Kept serial: it runs on the same cheap Linux pool as Fast Gate, so starting it
# early saves no wall time and would spend a runner on a diff the gate rejects.
STILL_GATED = ("rust-cross-lint",)


def job(name):
    match = re.search(rf"^  {re.escape(name)}:\n(.*?)(?=^  [a-z0-9-]+:\n|\Z)",
                      WORKFLOW.read_text(), re.M | re.S)
    if match is None:
        raise AssertionError(f"no job {name!r} in {WORKFLOW}")
    return match.group(1)


def dependencies(name):
    match = re.search(r"^    needs: \[([^]]+)\]", job(name), re.M)
    return [value.strip() for value in match.group(1).split(",")] if match else []


def header(name):
    """The job's key block: everything before its steps."""
    return job(name).split("    steps:", 1)[0]


def ancestors(name):
    direct = dependencies(name)
    return set(direct).union(*(ancestors(parent) for parent in direct))


class PlatformLaneOverlapTest(unittest.TestCase):
    def test_platform_lanes_have_no_fast_gate_ancestor(self):
        for name in PLATFORM_LANES:
            with self.subTest(lane=name):
                self.assertNotIn("rust-tests-linux", ancestors(name))
                self.assertIn("check-event", dependencies(name))
                self.assertIn("ci-mode", dependencies(name))

    def test_platform_lanes_do_not_reference_the_gate_result(self):
        # A `needs.<job>.result` left behind would be the same serialization,
        # and it would be a dangling reference once the need is gone.
        for name in PLATFORM_LANES:
            with self.subTest(lane=name):
                self.assertNotIn("needs.rust-tests-linux", header(name))

    def test_platform_lanes_keep_their_scope_and_skip_guards(self):
        # Overlapping the gate must not turn into running on every event: the
        # diff scope (#436) and the ci-mode skip flags still decide.
        for name in PLATFORM_LANES:
            with self.subTest(lane=name):
                head = header(name)
                self.assertIn("always()", head)
                self.assertIn("!cancelled()", head)
                self.assertIn("needs.check-event.outputs.", head)
                self.assertIn("needs.ci-mode.outputs.skip_", head)

    def test_cheap_same_pool_lane_still_waits_for_the_gate(self):
        for name in STILL_GATED:
            with self.subTest(lane=name):
                self.assertIn("rust-tests-linux", dependencies(name))
                self.assertIn("needs.rust-tests-linux.result != 'failure'", header(name))

    def test_a_failed_fast_gate_still_fails_the_run(self):
        # The platform lanes no longer gate on the Fast Gate result, so the
        # run-level check is the only thing keeping a red gate out of a merge.
        caller_gate = CALLER.read_text().split("  ci-gate:", 1)[1]
        self.assertIn("uv run --no-project ci/gate_jobs.py", caller_gate,
                      "CI Gate must fail on any unsuccessful job of the run")
        self.assertIn("contains(needs.*.result, 'failure')", caller_gate)
        self.assertIn("build-project", caller_gate.split("steps:", 1)[0])


if __name__ == "__main__":
    unittest.main()
