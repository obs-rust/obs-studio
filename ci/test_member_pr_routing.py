#!/usr/bin/env -S uv run --script
"""Member PR routing stays base-owned, fail-closed and below queue priority."""

import io
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError
from urllib.parse import parse_qs, urlparse

import select_mq_runner as selector

ROOT = Path(__file__).resolve().parent.parent
LABELS = '["self-hosted","linux","x64","obs-mq"]'


class MemberPrRoutingTest(unittest.TestCase):
    def select(self, *, head="obs-rust/obs-studio", author="alice", state="active",
               error=False, capacity=12, reserve="", enabled="on", event="pull_request",
               peers=(), jobs=None, payload=None, busy=0, max_runs="", missing_peer_ids=False, mq_demand="6"):
        self.paths = []
        current = {"id": 20, "event": event, "status": "in_progress",
                   "created_at": "2026-11-19T12:00:00Z"}
        runs = [current, *peers]
        data = payload if payload is not None else {
            "repository": {"full_name": "obs-rust/obs-studio", "id": 1},
            "pull_request": {
                "user": {"login": author},
                "head": {"repo": {"full_name": head, "id": 1 if head == "obs-rust/obs-studio" else 2}},
                "base": {"repo": {"full_name": "obs-rust/obs-studio", "id": 1}},
            },
        }

        def respond(request, **kwargs):
            url = urlparse(request.full_url)
            self.paths.append(url.path)
            if "/public_members/" in url.path:
                raise HTTPError(url.geturl(), 404, "private", {}, None)
            if "/memberships/" in url.path:
                if error:
                    raise HTTPError(url.geturl(), 503, "unavailable", {}, None)
                result = {"state": state, "user": {"login": author}}
            elif url.path.endswith("/runners"):
                entries = [{"id": i + 1, "status": "online", "busy": i < busy,
                            "labels": [{"name": name} for name in json.loads(LABELS)]}
                           for i in range(capacity)]
                result = {"total_count": len(entries), "runners": entries}
            elif url.path.endswith("/runs"):
                query = parse_qs(url.query)
                entries = [r for r in runs if r["event"] == query["event"][0]
                           and r["status"] == query["status"][0]]
                if missing_peer_ids:
                    entries = [{key: value for key, value in run.items() if key != "id"}
                               if run["id"] != 20 else run for run in entries]
                result = {"total_count": len(entries), "workflow_runs": entries}
            else:
                run_id = int(url.path.split("/runs/")[1].split("/")[0])
                entries = (jobs or {}).get(run_id, [])
                result = {"total_count": len(entries), "jobs": entries}
            return io.BytesIO(json.dumps(result).encode())

        with tempfile.TemporaryDirectory() as directory:
            event_path = Path(directory) / "event.json"
            event_path.write_text(json.dumps(data))
            with patch.dict(os.environ, {
                "GITHUB_EVENT_NAME": event, "GITHUB_EVENT_PATH": str(event_path),
                "GITHUB_RUN_ID": "20", "GITHUB_TOKEN": "actions-token",
                "DELL_PR_ROUTING": enabled, "MQ_PR_RESERVE": reserve,
                "MQ_RUNNER_MAX_RUNS": max_runs,
                "MQ_PR_DEMAND": getattr(self, "pr_demand", "6"),
            }, clear=True), patch.object(selector, "urlopen", side_effect=respond), patch.object(selector.time, "sleep"):
                return selector.select_labels(event, LABELS, "probe-token", "obs-rust/obs-studio", demand=mq_demand)

    def test_manager_two_job_override_cannot_admit_thirteen_job_pr(self):
        self.pr_demand = "2"
        self.assertEqual(self.select(capacity=6, reserve="3", mq_demand="3"), "")

    def test_low_override_still_requires_full_pr_inventory_plus_reserve(self):
        self.pr_demand = "2"
        self.assertEqual(self.select(capacity=15, reserve="3", mq_demand="3"), "")
        self.assertEqual(self.select(capacity=16, reserve="3", mq_demand="3"), LABELS)

    def test_busy_queue_overcommit_never_relaxes_pr_idle_reserve(self):
        self.assertEqual(self.select(event="merge_group", capacity=6, busy=6), LABELS)
        self.assertEqual(self.select(capacity=12, busy=1, max_runs="100"), "")
        self.assertEqual(self.select(capacity=12, max_runs="bad"), LABELS)
        self.assertEqual(self.select(capacity=6, busy=1, reserve="0", max_runs="100"), "")

    def test_missing_ids_across_queue_and_pr_keep_both_weighted_reservations(self):
        peers = [{"id": index, "event": event, "status": "in_progress",
                  "created_at": "2026-11-19T11:59:00Z"}
                 for index, event in ((10, "merge_group"), (11, "pull_request"))]
        self.assertEqual(self.select(event="merge_group", capacity=6,
                                     peers=peers, max_runs="2", missing_peer_ids=True), "")
        self.assertFalse(any("/runs/-" in path for path in self.paths))

    def test_mixed_queue_at_run_cap_uses_hosted(self):
        peer = {"id": 10, "event": "merge_group", "status": "in_progress",
                "created_at": "2026-11-19T11:59:00Z"}
        jobs = {10: [{"name": "Build", "status": "in_progress", "labels": json.loads(LABELS)}]}
        self.assertEqual(self.select(event="merge_group", capacity=6, busy=6,
                                     peers=[peer], jobs=jobs, max_runs="1"), "")

    def test_member_with_idle_capacity_uses_dell(self):
        self.assertEqual(self.select(), LABELS)
        self.assertIn("/orgs/obs-rust/memberships/alice", self.paths)

    def test_default_pr_demand_preserves_reserve_for_full_inventory(self):
        self.pr_demand = ""
        self.assertEqual(self.select(capacity=18), "")
        self.assertEqual(self.select(capacity=19), LABELS)

    def test_invalid_pr_demand_uses_hosted(self):
        for demand in ("0", "-1", "bad"):
            self.pr_demand = demand
            self.assertEqual(self.select(), "")

    def test_fork_uses_hosted_without_api(self):
        self.assertEqual(self.select(head="fork/obs-studio"), "")
        self.assertEqual(self.paths, [])

    def test_non_member_and_pending_members_use_hosted(self):
        for state in ("pending", "inactive", None):
            with self.subTest(state=state):
                self.assertEqual(self.select(state=state), "")

    def test_membership_api_error_uses_hosted_without_retry(self):
        self.assertEqual(self.select(error=True), "")
        self.assertEqual(self.paths.count("/orgs/obs-rust/memberships/alice"), 1)
        self.assertFalse(any(path.endswith("/runners") for path in self.paths))

    def test_capacity_below_default_reserve_uses_hosted(self):
        self.assertEqual(self.select(capacity=11), "")

    def test_reserve_can_be_configured(self):
        self.assertEqual(self.select(capacity=8, reserve="2"), LABELS)
        self.assertEqual(self.select(capacity=7, reserve="2"), "")

    def test_invalid_reserve_uses_hosted(self):
        for reserve in ("-1", "oops"):
            self.assertEqual(self.select(reserve=reserve), "")

    def test_kill_switch_defaults_off(self):
        for enabled in ("", "off", "true", "ON"):
            self.assertEqual(self.select(enabled=enabled), "")
            self.assertEqual(self.paths, [])

    def test_pull_request_target_never_routes(self):
        self.assertEqual(self.select(event="pull_request_target"), "")
        self.assertEqual(self.paths, [])

    def test_untrusted_or_missing_payload_uses_hosted(self):
        for payload in ({}, {"pull_request": None}, {"repository": {"full_name": "fork/repo"}}):
            self.assertEqual(self.select(payload=payload), "")

    def test_merge_group_ignores_pr_kill_switch_and_reserve(self):
        self.assertEqual(self.select(event="merge_group", enabled="off", capacity=6, reserve="bad"), LABELS)

    def test_pr_reserves_undecided_merge_queue_even_when_newer(self):
        peer = {"id": 30, "event": "merge_group", "status": "queued",
                "created_at": "2026-11-19T12:01:00Z"}
        self.assertEqual(self.select(peers=[peer]), "")

    def test_existing_pr_dell_jobs_reduce_capacity_for_prs_and_queue(self):
        peer = {"id": 10, "event": "pull_request", "status": "in_progress",
                "created_at": "2026-11-19T11:59:00Z"}
        jobs = {10: [{"name": "Fast Gate ⚡", "status": "queued", "conclusion": None,
                      "labels": json.loads(LABELS)} for _ in range(7)]}
        self.assertEqual(self.select(event="pull_request", peers=[peer], jobs=jobs), "")
        self.assertEqual(self.select(event="merge_group", peers=[peer], jobs=jobs), LABELS)

    def test_mixed_demands_admit_only_one_queue_run_beside_large_pr(self):
        self.pr_demand = "13"
        pr = {"id": 10, "event": "pull_request", "status": "in_progress",
              "created_at": "2026-11-19T11:59:00Z"}
        mq = {"id": 15, "event": "merge_group", "status": "in_progress",
              "created_at": "2026-11-19T11:59:30Z"}
        # Idle census can lag a routing choice: charge the PR's full 13-slot
        # commitment, not one six-slot queue run, even before its jobs start.
        jobs = {10: [{"name": "Fast Gate ⚡", "status": "in_progress",
                      "conclusion": None, "labels": json.loads(LABELS)}]}
        self.assertEqual(self.select(event="merge_group", capacity=19, peers=[pr], jobs=jobs, max_runs="4"), LABELS)
        self.assertEqual(self.select(event="merge_group", capacity=19, peers=[pr, mq], jobs=jobs, max_runs="4"), "")


class MemberPrWorkflowTest(unittest.TestCase):
    def routed_pr_jobs(self):
        from tier2_matrix import matrix

        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        blocks = dict(re.findall(r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)", text, re.M | re.S))
        jobs = {}
        for name, block in blocks.items():
            if not re.search(r"runs-on: .*inputs.mq-heavy-labels", block):
                continue
            if name == "rust-linux-tier2":
                scheduling = matrix("pull_request")
                jobs[name] = len(scheduling["mode"]) * len(scheduling["part"])
            else:
                variants = re.search(r"^        (?:target|alias): \[([^\]]+)\]", block, re.M)
                jobs[name] = len(variants[1].split(",")) if variants else 1
        return jobs

    def test_default_pr_demand_covers_expanded_routed_jobs(self):
        self.assertEqual(selector.PR_RUNNER_DEMAND, sum(self.routed_pr_jobs().values()))

    def test_configured_pr_admission_demand_covers_actual_job_graph(self):
        count = sum(self.routed_pr_jobs().values())
        for configured in ("", "2", "3", "13", "20"):
            with self.subTest(configured=configured), patch.dict(os.environ, {
                "DELL_PR_ROUTING": "on", "MQ_PR_DEMAND": configured,
                "MQ_PR_RESERVE": "3",
            }, clear=True):
                capacity = selector.routing_capacity("pull_request", "1", "3", "")
                self.assertGreaterEqual(capacity["required"], count)
                self.assertEqual(capacity["reserve"], 3)
                self.assertGreater(capacity["required"] + capacity["reserve"], 6)

    def test_base_owned_selector_and_membership_helper(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        probe = ci.split("  select-mq-runner:\n", 1)[1].split("  cancel-superseded:\n", 1)[0]
        self.assertIn("ref: ${{ github.event.pull_request.base.sha || github.sha }}", probe)
        self.assertIn('run: uv run --no-project ci/run_base_runner_selector.py', probe)
        launcher = (ROOT / "ci/run_base_runner_selector.py").read_text()
        self.assertIn('["git", "show", f"{base}:ci/{script}"]', launcher)
        self.assertIn('("select_mq_runner.py", "pr_attestation.py")', launcher)
        self.assertIn("DELL_PR_ROUTING: ${{ vars.DELL_PR_ROUTING || 'off' }}", probe)
        self.assertIn("MQ_PR_RESERVE: ${{ vars.MQ_PR_RESERVE }}", probe)
        self.assertNotIn("if: github.event_name != 'pull_request'\n", probe)

    def test_dell_pr_jobs_do_not_receive_repository_secrets(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        blocks = dict(re.findall(r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:|\Z)", text, re.M | re.S))
        for name, block in blocks.items():
            if not re.search(r"runs-on: .*inputs.mq-heavy-labels", block):
                continue
            for line in block.splitlines():
                if "secrets." in line and "secrets.GITHUB_TOKEN" not in line:
                    with self.subTest(job=name, line=line):
                        self.assertIn("(github.event_name != 'pull_request' || inputs.mq-heavy-labels == '') && secrets.", line)

    def test_caller_strips_secrets_before_dell_pr_reusable_workflow(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        block = ci.split("  build-project:\n", 1)[1].split("  flatpak-runtime-reuse:\n", 1)[0]
        self.assertNotIn("secrets: inherit", block)
        build = (ROOT / ".github/workflows/build-project.yaml").read_text()
        names = set(re.findall(r"secrets\.([A-Z0-9_]+)", build))
        for name in names:
            self.assertIn(name + ": ${{ (github.event_name != 'pull_request' || needs.select-mq-runner.outputs.heavy-labels == '') && secrets." + name + " || '' }}", block)

    def test_fast_gate_pr_routing_keeps_queue_hosted(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        block = text.split("  rust-tests-linux:\n", 1)[1].split("\n  cross-compile:", 1)[0]
        runner = re.search(r"^    runs-on: (.*)$", block, re.M)[1]
        self.assertIn("github.event_name == 'pull_request' && inputs.mq-heavy-labels", runner)


class BaseRunnerLauncherTest(unittest.TestCase):
    def test_selector_and_membership_are_loaded_from_base_objects(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ci = root / "ci"
            ci.mkdir()

            def git(*args):
                return subprocess.run(["git", *args], cwd=root, check=True,
                                      capture_output=True, text=True).stdout.strip()

            git("init", "--quiet")
            (ci / "pr_attestation.py").write_text("identity = 'base-owned'\n")
            (ci / "select_mq_runner.py").write_text(
                "import os\nfrom pathlib import Path\nimport pr_attestation\n"
                "Path(os.environ['GITHUB_OUTPUT']).write_text(pr_attestation.identity)\n"
            )
            git("add", "ci")
            git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                "commit", "--quiet", "--no-gpg-sign", "-m", "trusted base")
            base = git("rev-parse", "HEAD")
            # A PR changes both files; executing either working-tree module fails.
            for path in ci.iterdir():
                path.write_text("raise RuntimeError('PR-controlled code executed')\n")
            output = root / "output"
            result = subprocess.run(
                ["uv", "run", "--no-project", str(ROOT / "ci/run_base_runner_selector.py")],
                cwd=root, env={**os.environ, "BASE": base, "RUNNER_TEMP": directory,
                               "GITHUB_OUTPUT": str(output)},
                capture_output=True, text=True, timeout=30,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(output.read_text(), "base-owned")

    def test_invalid_base_never_launches_selector(self):
        result = subprocess.run(
            ["uv", "run", "--no-project", str(ROOT / "ci/run_base_runner_selector.py")],
            env={**os.environ, "BASE": "HEAD:evil"},
            capture_output=True, text=True, timeout=30,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid immutable base SHA", result.stderr)


if __name__ == "__main__":
    unittest.main()
