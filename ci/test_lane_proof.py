#!/usr/bin/env -S uv run --script
"""Tests for ci/lane_proof.py, the lane-proof ledger (#699).

    uv run python -m unittest ci/test_lane_proof.py

A wrong skip merges untested code, so most cases here are about refusing:
fork runs, other workflows, push runs, stale or expired proofs, API errors,
changed proof surfaces and a head that widens its own exclusions.
"""

from __future__ import annotations

import importlib.util
import os
import shutil
import subprocess
import tempfile
import time
import unittest
import urllib.error
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("lane_proof", ROOT / "ci" / "lane_proof.py")
lane_proof = importlib.util.module_from_spec(spec)
spec.loader.exec_module(lane_proof)

REPO = "obs-rust/obs-studio"
NOW = 1_800_000_000.0


def run(event="merge_group", head=REPO, path=lane_proof.TRUSTED_WORKFLOW, age_h=1.0, run_id=7):
    created = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(NOW - age_h * 3600))
    return {"id": run_id, "event": event, "path": path, "created_at": created,
            "head_repository": {"full_name": head}, "head_branch": "b"}


class Trust(unittest.TestCase):
    def test_same_repo_pull_request_and_merge_queue(self):
        self.assertIsNone(lane_proof.trusted(run("pull_request"), REPO, NOW))
        self.assertIsNone(lane_proof.trusted(run("merge_group"), REPO, NOW))

    def test_refuses(self):
        cases = {
            "fork": run("pull_request", head="someone/obs-studio"),
            "push": run("push"),
            "dispatch": run("workflow_dispatch"),
            "other workflow": run(path=".github/workflows/evil.yaml"),
            "too old": run(age_h=lane_proof.MAX_AGE_HOURS + 1),
            "no time": {**run(), "created_at": ""},
        }
        for name, r in cases.items():
            with self.subTest(name):
                self.assertIsNotNone(lane_proof.trusted(r, REPO, NOW))


class Publication(unittest.TestCase):
    """Publication must survive optional skipped jobs without trusting a red lane."""

    def setUp(self):
        self.caller = (ROOT / ".github/workflows/ci.yml").read_text()
        self.build = (ROOT / ".github/workflows/build-project.yaml").read_text()
        self.publisher = self.caller.split("  tier2-lane-proof:\n", 1)[1].split("\n  cache-janitor:", 1)[0]
        self.condition = self.publisher.split("    if:", 1)[1].split("    runs-on:", 1)[0]

    def test_optional_skips_do_not_apply_implicit_success(self):
        self.assertIn("!cancelled()", self.condition)
        self.assertNotIn("success()", self.condition)

    def test_publisher_requires_gate_build_output_and_trusted_event(self):
        self.assertIn("needs: [build-project, ci-gate]", self.publisher)
        for guard in (
            "needs.ci-gate.result == 'success'",
            "needs.build-project.result == 'success'",
            "needs.build-project.outputs.tier2-linux-proof != ''",
            "github.event_name == 'merge_group'",
            "github.event_name == 'pull_request'",
            "github.event.pull_request.head.repo.full_name == github.repository",
        ):
            self.assertIn(guard, self.condition)

    def test_output_chain_still_comes_from_the_build_that_ran(self):
        self.assertIn("value: ${{ jobs.rust-linux-tier2-build.outputs.tier2-linux-proof }}", self.build)
        build = self.build.split("  rust-linux-tier2-build:\n", 1)[1].split("    steps:", 1)[0]
        self.assertIn("tier2-linux-proof: ${{ needs.ci-mode.outputs.tier2-linux_name }}", build)
        self.assertIn("needs.ci-mode.outputs.tier2-linux_hit != 'true'", build)
        self.assertIn("name: ${{ needs.build-project.outputs.tier2-linux-proof }}", self.publisher)

    def test_gate_logs_the_actual_reusable_workflow_outputs(self):
        gate = self.caller.split("  ci-gate:\n", 1)[1].split("  tier2-lane-proof:\n", 1)[0]
        self.assertIn("${{ toJSON(needs.build-project) }}", gate)
        self.assertIn('echo "$BUILD_PROJECT"', gate)


class Lookup(unittest.TestCase):
    def fetcher(self, artifacts, runs):
        def fetch(path, token):
            if "/actions/artifacts?" in path:
                return {"artifacts": artifacts}
            run_id = int(path.rsplit("/", 1)[1])
            if isinstance(runs[run_id], Exception):
                raise runs[run_id]
            return runs[run_id]
        return fetch

    def art(self, run_id, key="k", expired=False):
        return {"name": lane_proof.artifact_name("rust-test", key), "expired": expired,
                "workflow_run": {"id": run_id}}

    def test_trusted_proof_is_a_hit(self):
        fetch = self.fetcher([self.art(1)], {1: run(run_id=1)})
        hit, reason = lane_proof.lookup("rust-test", "k", REPO, "t", fetch=fetch, now=NOW)
        self.assertTrue(hit, reason)

    def test_untrusted_proofs_miss_with_reasons(self):
        fetch = self.fetcher([self.art(1), self.art(2)],
                             {1: run("pull_request", head="x/fork", run_id=1), 2: run("push", run_id=2)})
        hit, reason = lane_proof.lookup("rust-test", "k", REPO, "t", fetch=fetch, now=NOW)
        self.assertFalse(hit)
        self.assertIn("fork", reason)

    def test_expired_and_misnamed_artifacts_are_ignored(self):
        other = {**self.art(1), "name": lane_proof.artifact_name("rust-test", "other")}
        fetch = self.fetcher([self.art(1, expired=True), other], {1: run(run_id=1)})
        hit, _ = lane_proof.lookup("rust-test", "k", REPO, "t", fetch=fetch, now=NOW)
        self.assertFalse(hit)

    def test_api_errors_fail_closed(self):
        def broken(path, token):
            raise urllib.error.URLError("down")
        self.assertFalse(lane_proof.lookup("rust-test", "k", REPO, "t", fetch=broken, now=NOW)[0])
        fetch = self.fetcher([self.art(1)], {1: urllib.error.URLError("down")})
        self.assertFalse(lane_proof.lookup("rust-test", "k", REPO, "t", fetch=fetch, now=NOW)[0])

    def test_no_proof(self):
        fetch = self.fetcher([], {})
        self.assertEqual(lane_proof.lookup("rust-test", "k", REPO, "t", fetch=fetch, now=NOW),
                         (False, "no proof for this key"))


GATE = """[gate]
run = ["uv", "run", "ci/local_gate.py"]

[gate.lanes.rust-test]
run = ["uv", "run", "ci/local_gate.py", "--lane", "rust-test"]
tools = ["soldr"]
exclude = ["**"]
keep = ["rust/**"]
"""


@unittest.skipUnless(shutil.which("uv") and shutil.which("git"), "needs uv and git")
class Keys(unittest.TestCase):
    """End to end through the script, against a scratch repository."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name)
        self.git("init", "-q", "-b", "master")
        self.write("local-gate.toml", GATE)
        self.write("ci/local_gate.py", "")
        self.write("rust/lib.rs", "fn a() {}\n")
        self.write("docs/a.md", "a\n")
        self.base = self.commit("base")

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, *args):
        env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@e", "GIT_COMMITTER_NAME": "t",
               "GIT_COMMITTER_EMAIL": "t@e", "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
        return subprocess.run(["git", *args], cwd=self.repo, env=env, check=True, capture_output=True,
                              text=True).stdout.strip()

    def write(self, path, text):
        (self.repo / path).parent.mkdir(parents=True, exist_ok=True)
        (self.repo / path).write_text(text)

    def commit(self, msg):
        self.git("add", "-A")
        self.git("commit", "-q", "-m", msg)
        return self.git("rev-parse", "HEAD")

    def key(self, head="HEAD", base=None):
        env = {**os.environ, "LANE_PROOF_REPO": str(self.repo), "ImageOS": "test", "ImageVersion": "1"}
        out = subprocess.run(["uv", "run", str(ROOT / "ci" / "lane_proof.py"), "key", "--lane", "rust-test",
                              "--base", base or self.base, "--head", head],
                             env=env, check=True, capture_output=True, text=True).stdout
        return dict(line.split("=", 1) for line in out.splitlines() if "=" in line)

    def test_excluded_change_keeps_the_key(self):
        before = self.key()["key"]
        self.write("docs/a.md", "b\n")
        self.commit("docs")
        self.assertEqual(self.key()["key"], before)
        self.assertTrue(before)

    def test_kept_change_changes_the_key(self):
        before = self.key()["key"]
        self.write("rust/lib.rs", "fn b() {}\n")
        self.commit("rust")
        self.assertNotEqual(self.key()["key"], before)

    def test_surface_change_has_no_key(self):
        self.write("local-gate.toml", GATE.replace('keep = ["rust/**"]', 'keep = []'))
        self.commit("widen exclusions")
        self.assertEqual(self.key()["key"], "")
        self.assertIn("local-gate.toml", self.key()["reason"])

    def test_runner_install_policy_change_cannot_reuse_a_lane_proof(self):
        self.write("ci/nextest_tool.py", "changed runner installer policy\n")
        self.commit("runner policy")
        self.assertEqual(self.key()["key"], "")
        self.assertIn("ci/nextest_tool.py", self.key()["reason"])

    def test_declaration_is_read_from_the_base(self):
        # A head cannot widen its own exclusions: the base's keep list still
        # puts rust/ in the key even when the head's declaration drops it.
        self.write("rust/lib.rs", "fn c() {}\n")
        head = self.commit("rust change")
        k1 = self.key(head=head)["key"]
        self.assertNotEqual(k1, self.key(head=self.base)["key"])


if __name__ == "__main__":
    unittest.main()
