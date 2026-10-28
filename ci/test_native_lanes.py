#!/usr/bin/env -S uv run --script
"""The native lanes, and the macOS/Windows jobs they attest (#443).

    uv run python -m unittest ci/test_native_lanes.py

A change to platform code is attested locally or run in CI. The two native
lanes run the jobs they attest on the host that attests them (Rust tests,
clippy and formatting, then Tier 2) and exit 75 everywhere else, which
`ci-lint local-gate` reads as "not applicable on this host" for an `optional`
lane: a Linux gate passes without attesting them, so CI still runs those jobs.
"""

from __future__ import annotations

import subprocess
import sys
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CI = ROOT / "ci"

import host_platform
import local_gate

LANES = ("macos-native", "windows-native")
# The jobs each lane attests, and the gates those jobs run.
JOBS = {
    "rust-tests-macos": ("macos-native", ["rust/all/fmt", "rust/aarch64-apple-darwin/clippy", "rust/aarch64-apple-darwin/test"]),
    "rust-tests-windows": ("windows-native", ["rust/all/fmt", "rust/x86_64-pc-windows-msvc/clippy", "rust/x86_64-pc-windows-msvc/test"]),
    "rust-tier2-macos": ("macos-native", ["cpp/macos-arm64/tier2"]),
    "rust-tier2-windows": ("windows-native", ["cpp/windows-x64/tier2"]),
}


def config() -> dict:
    return tomllib.loads((ROOT / "local-gate.toml").read_text(encoding="utf-8"))


class NativeLanesTest(unittest.TestCase):
    def test_lanes_read_the_host_through_the_facade(self):
        self.assertEqual(set(local_gate.NATIVE_HOSTS), set(LANES))
        self.assertEqual(local_gate.NATIVE_HOSTS["macos-native"], host_platform.is_macos)
        self.assertEqual(local_gate.NATIVE_HOSTS["windows-native"], host_platform.is_windows)
        for lane in LANES:
            with self.subTest(lane=lane):
                self.assertIn(lane, local_gate.LANES)

    def test_off_host_lane_says_not_applicable(self):
        for lane in LANES:
            if local_gate.native_host(lane):
                continue  # would run the real Tier 2 on this host
            with self.subTest(lane=lane):
                # The host check comes before any tool runs, so nothing is
                # built: the lane exits 75 (EX_TEMPFAIL).
                proc = subprocess.run(
                    [sys.executable, str(CI / "local_gate.py"), "--lane", lane],
                    cwd=ROOT, capture_output=True, text=True, timeout=120, check=False,
                )
                self.assertEqual(proc.returncode, 75, proc.stdout + proc.stderr)
                self.assertIn("not applicable", proc.stdout)

    def test_every_lane_is_optional_and_declares_its_command(self):
        lanes = config()["gate"]["lanes"]
        for lane in LANES:
            with self.subTest(lane=lane):
                self.assertTrue(lanes[lane].get("optional"), "a native lane must be optional")
                self.assertEqual(lanes[lane]["run"], ["uv", "run", "ci/local_gate.py", "--lane", lane])

    def test_trust_skips_the_native_jobs(self):
        trust = config()["gate"]["trust"]
        for job, (lane, _gates) in JOBS.items():
            ref = f"build-project.yaml:{job}"
            with self.subTest(job=job):
                self.assertIn(ref, trust["skip"])
                self.assertIn(lane, trust["covered-by"][ref])

    def test_verify_exempt_no_longer_lists_them(self):
        exempt = config()["gate"]["verify-exempt"]
        for job in JOBS:
            with self.subTest(job=job):
                self.assertNotIn(job, exempt, "an attested job must not also be verify-exempt")

    def test_attestations_map_the_jobs_to_their_gates(self):
        text = (ROOT / "ci-attestations.yml").read_text(encoding="utf-8")
        for job, (lane, gates) in JOBS.items():
            with self.subTest(job=job):
                self.assertTrue(f"  build-project.yaml:{job}: [{', '.join(gates)}]" in text, job)
        for lane in LANES:
            self.assertTrue(f"{{lane: {lane}}}" in text, lane)

    def test_workflow_consumes_the_skip_outputs(self):
        text = (ROOT / ".github" / "workflows" / "build-project.yaml").read_text(encoding="utf-8")
        for job in JOBS:
            with self.subTest(job=job):
                self.assertTrue(f"      skip_{job}: ${{{{ steps.gate.outputs.skip_{job} }}}}" in text, job)
                self.assertTrue(f"needs.ci-mode.outputs.skip_{job} != 'true'" in text, job)


if __name__ == "__main__":
    unittest.main()
