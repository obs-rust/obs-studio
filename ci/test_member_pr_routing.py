#!/usr/bin/env -S uv run --script
"""Member PR routing stays base-owned, fail-closed and below queue priority."""

import io
import json
import os
from pathlib import Path
import re
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
               peers=(), jobs=None, payload=None):
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
                entries = [{"id": i + 1, "status": "online", "busy": False,
                            "labels": [{"name": name} for name in json.loads(LABELS)]}
                           for i in range(capacity)]
                result = {"total_count": len(entries), "runners": entries}
            elif url.path.endswith("/runs"):
                query = parse_qs(url.query)
                entries = [r for r in runs if r["event"] == query["event"][0]
                           and r["status"] == query["status"][0]]
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
            }, clear=True), patch.object(selector, "urlopen", side_effect=respond), patch.object(selector.time, "sleep"):
                return selector.select_labels(event, LABELS, "probe-token", "obs-rust/obs-studio", demand="6")

    def test_member_with_idle_capacity_uses_dell(self):
        self.assertEqual(self.select(), LABELS)
        self.assertIn("/orgs/obs-rust/memberships/alice", self.paths)

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
        for event in ("pull_request", "merge_group"):
            self.assertEqual(self.select(event=event, peers=[peer], jobs=jobs), "")


class MemberPrWorkflowTest(unittest.TestCase):
    def test_base_owned_selector_and_membership_helper(self):
        ci = (ROOT / ".github/workflows/ci.yml").read_text()
        probe = ci.split("  select-mq-runner:\n", 1)[1].split("  cancel-superseded:\n", 1)[0]
        self.assertIn("ref: ${{ github.event.pull_request.base.sha || github.sha }}", probe)
        self.assertIn('git show "$BASE:ci/$script"', probe)
        self.assertIn("select_mq_runner.py pr_attestation.py", probe)
        self.assertIn('uv run --no-project "$trusted/select_mq_runner.py"', probe)
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
                        self.assertIn("github.event_name != 'pull_request' && secrets.", line)

    def test_fast_gate_pr_routing_keeps_queue_hosted(self):
        text = (ROOT / ".github/workflows/build-project.yaml").read_text()
        block = text.split("  rust-tests-linux:\n", 1)[1].split("\n  cross-compile:", 1)[0]
        runner = re.search(r"^    runs-on: (.*)$", block, re.M)[1]
        self.assertIn("github.event_name == 'pull_request' && inputs.mq-heavy-labels", runner)


if __name__ == "__main__":
    unittest.main()
