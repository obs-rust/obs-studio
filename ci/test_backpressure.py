#!/usr/bin/env -S uv run --script
"""Deterministic admission accounting; no network or runner mutations."""
import contextlib
import copy
import datetime as dt
import io
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch

def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(f"{name}.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


bp = load("backpressure")
ai_rebase = load("ai_rebase")


HEAD = "a" * 40
CLOCK = dt.datetime(2026, 11, 11, 12, tzinfo=dt.timezone.utc)


def connection(nodes, **overrides):
    return {"nodes": nodes, "totalCount": len(nodes),
            "pageInfo": {"hasNextPage": False, "endCursor": None}, **overrides}


def check(conclusion="FAILURE", status="COMPLETED", head=HEAD):
    return {"id": f"check-{conclusion}-{status}-{head}", "__typename": "CheckRun", "status": status, "conclusion": conclusion,
            "checkSuite": {"commit": {"oid": head}}}


def pr(number=1, **overrides):
    return {"number": number, "isDraft": False, "autoMergeRequest": {"enabledAt": "now"},
            "mergeStateStatus": "BLOCKED", "headRefOid": HEAD, "headRefName": "fix/probe",
            "createdAt": "2026-11-11T10:00:00Z",
            **overrides}


class AdmissionTests(unittest.TestCase):
    def setUp(self):
        self.prs = [pr()]
        self.queue = []
        self.contexts = [check()]
        self.commit = HEAD
        self.rollup = "FAILURE"
        self.runs = []
        self.queued = []
        self.merged = []
        self.bad = None
        self.calls = []
        self.global_runs = []
        self.suites = []
        self.final_head = HEAD

    def github(self, *args):
        self.calls.append(args)
        if args[0] == "pr":
            return copy.deepcopy(self.merged if "merged" in args else self.prs)
        if args[0] == "run":
            return copy.deepcopy(self.queued)
        if args[0] == "api":
            if "check-suites" in args[1]:
                return [{"total_count": len(self.suites), "check_suites": self.suites}]
            if "created=" in args[1]:
                return [{"total_count": len(self.global_runs) + (self.bad == "inventory"),
                         "workflow_runs": self.global_runs}]
            if self.bad == "runs":
                return [{"total_count": 2, "workflow_runs": self.runs}]
            return [{"total_count": len(self.runs), "workflow_runs": self.runs}]
        if args[0] == "graphql":
            query = args[-1]
            depth = 0
            for char in query:
                depth += (char == "{") - (char == "}")
                self.assertGreaterEqual(depth, 0, query)
            self.assertEqual(depth, 0, query)
            if "mergeQueue" in query:
                entries = connection([{"pullRequest": {"number": n}} for n in self.queue])
                if self.bad == "queue":
                    entries["pageInfo"]["hasNextPage"] = True
                return {"data": {"repository": {"mergeQueue": {"entries": entries}}}}
            if "statusCheckRollup" not in query:
                return {"data": {"repository": {"pullRequest": {"headRefOid": self.final_head}}}}
            contexts = connection(self.contexts)
            if self.bad == "checks":
                contexts["totalCount"] += 1
            return {"data": {"repository": {"pullRequest": {
                "headRefOid": self.commit, "commits": {"nodes": [{"commit": {
                    "oid": self.commit, "statusCheckRollup": {
                        "state": self.rollup, "contexts": contexts}}}]}}}}}
        raise AssertionError(args)

    def measure(self):
        with patch.object(bp, "gh_json", side_effect=self.github), patch.object(bp, "now", return_value=CLOCK):
            return bp.measure()

    def test_parked_red_releases_one_probe(self):
        self.prs.append(pr(2))
        v = self.measure()
        self.assertEqual((v["state"], v["in_flight"], v["parked_red"], v["limit"], v["headroom"]),
                         ("open", 0, 2, 1, 1))

    def test_green_and_unknown_conserved(self):
        for contexts, state in [([check("SUCCESS")], "SUCCESS"), ([], "FAILURE"),
                                ([check(None)], "FAILURE"), ([{"id": "future-1", "__typename": "FutureCheck"}], "FAILURE"),
                                ([check(), check(None, "IN_PROGRESS")], "FAILURE"),
                                ([check(), {"id": "status-1", "__typename": "StatusContext", "state": "PENDING",
                                            "commit": {"oid": HEAD}}], "FAILURE")]:
            with self.subTest(contexts=contexts):
                self.contexts, self.rollup = contexts, state
                self.assertEqual(self.measure()["in_flight"], 1)

    def test_red_active_retry_counts_even_before_new_check(self):
        for status in ["queued", "in_progress", "waiting", "requested", "pending", "unknown"]:
            with self.subTest(status=status):
                self.runs = [{"id": 9, "status": status, "head_sha": HEAD}]
                self.assertEqual(self.measure()["in_flight"], 1)

    def test_queue_wins_and_deduplicates_even_null_auto_merge(self):
        self.prs = [pr(), pr(2, autoMergeRequest=None), pr(3, isDraft=True)]
        self.queue = [1, 2, 3]
        v = self.measure()
        self.assertEqual((v["in_flight"], v["parked_red"]), (3, 0))

    def test_queue_entry_added_during_check_read_wins(self):
        original = self.github
        queue_reads = 0
        def github(*args):
            nonlocal queue_reads
            if args[0] == "graphql" and "mergeQueue" in args[-1]:
                queue_reads += 1
                if queue_reads == 2:
                    self.queue = [1]
            return original(*args)
        with patch.object(bp, "gh_json", side_effect=github), patch.object(bp, "now", return_value=CLOCK):
            v = bp.measure()
        self.assertEqual((v["in_flight"], v["parked_red"]), (1, 0))

    def test_failed_old_head_not_parked(self):
        self.contexts = [check(head="b" * 40)]
        self.assertEqual(self.measure()["in_flight"], 1)
        self.contexts = [check()]
        self.commit = "b" * 40
        self.assertEqual(self.measure()["in_flight"], 1)

    def test_starvation_independent_of_parked_red(self):
        self.queued = [{"databaseId": 7, "createdAt": "2026-11-11T11:40:00Z", "event": "push"}]
        v = self.measure()
        self.assertEqual((v["state"], v["headroom"]), ("closed", 0))
        self.assertTrue(any("runner" in r for r in v["reasons"]))

    def test_throughput_limit_unchanged(self):
        self.merged = [{"number": n} for n in range(1, 5)]
        self.rollup = "SUCCESS"
        self.assertEqual(self.measure()["headroom"], 3)

    def test_failed_and_incomplete_reads_close_main(self):
        for bad in ["queue", "checks", "runs", "error", "malformed", "open_cap", "queued_cap", "merged_cap"]:
            with self.subTest(bad=bad):
                self.bad = bad
                def github(*args):
                    if bad == "error":
                        raise bp.subprocess.CalledProcessError(1, "gh", stderr="unavailable")
                    if bad == "malformed":
                        return None
                    if bad == "open_cap" and args[0] == "pr" and "open" in args:
                        return [pr(n) for n in range(300)]
                    if bad == "merged_cap" and args[0] == "pr" and "merged" in args:
                        return [{"number": n} for n in range(200)]
                    if bad == "queued_cap" and args[0] == "run":
                        return [{"databaseId": n, "createdAt": "2026-11-11T12:00:00Z"} for n in range(200)]
                    return self.github(*args)
                with patch.object(bp, "gh_json", side_effect=github), patch.object(bp, "now", return_value=CLOCK), \
                        patch("sys.argv", ["backpressure"]), contextlib.redirect_stdout(io.StringIO()) as out:
                    self.assertEqual(bp.main(), bp.EXIT_CLOSED)
                    self.assertEqual(json.loads(out.getvalue())["headroom"], 0)

    def test_all_queue_pages_and_pending_status_on_later_check_page(self):
        calls = []
        def github(*args):
            query = args[-1]
            calls.append(query)
            later = 'after:"next"' in query
            if "mergeQueue" in query:
                value = connection([{"pullRequest": {"number": 2 if later else 1}}],
                                   totalCount=2, pageInfo={"hasNextPage": not later, "endCursor": "next"})
                return {"data": {"repository": {"mergeQueue": {"entries": value}}}}
            value = connection([{"id": "status-1", "__typename": "StatusContext", "state": "PENDING",
                                 "commit": {"oid": HEAD}}] if later else [check()],
                               totalCount=2, pageInfo={"hasNextPage": not later, "endCursor": "next"})
            return {"data": {"repository": {"pullRequest": {"headRefOid": HEAD,
                "commits": {"nodes": [{"commit": {"oid": HEAD,
                    "statusCheckRollup": {"state": "FAILURE", "contexts": value}}}]}}}}}
        with patch.object(bp, "gh_json", side_effect=github):
            self.assertEqual(bp.queue_entries(bp.REPO), {1, 2})
            self.assertFalse(bp.terminal_red(bp.REPO, pr()))
        self.assertEqual(len(calls), 4)

    def test_paginated_runs_include_hidden_retry(self):
        pages = [{"total_count": 2, "workflow_runs": [{"id": 1, "status": "completed"}]},
                 {"total_count": 2, "workflow_runs": [{"id": 2, "status": "queued"}]}]
        with patch.object(bp, "gh_json", return_value=pages):
            self.assertFalse(bp.inactive_runs(bp.REPO, pr()))
            pages[1]["workflow_runs"][0]["status"] = "completed"
            self.assertTrue(bp.inactive_runs(bp.REPO, pr()))
            pages[1]["total_count"] = 3
            with self.assertRaises(ValueError):
                bp.inactive_runs(bp.REPO, pr())

    def test_no_github_mutations_and_explained_text_output(self):
        with patch.object(bp, "gh_json", side_effect=self.github), patch.object(bp, "now", return_value=CLOCK), \
                patch("sys.argv", ["backpressure", "--check"]), contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(bp.main(), 0)
        self.assertIn("counted=0 parked_red=1", out.getvalue())
        self.assertFalse(any("mutation" in " ".join(call) for call in self.calls))
        self.assertFalse(any("--disable-auto" in call or "merge" in call or "rerun" in call for call in self.calls))

    def test_synthetic_ref_and_unassociated_active_work_conserved(self):
        for associated in [[], [{"number": 1}]]:
            self.global_runs = [{"id": 88, "status": "in_progress", "event": "merge_group",
                                 "head_branch": "gh-readonly-queue/master/pr-1-xyz", "head_sha": "b" * 40,
                                 "pull_requests": associated}]
            self.assertEqual(self.measure()["in_flight"], 1)
        self.global_runs[0]["pull_requests"] = [{"number": 99}]
        self.assertEqual(self.measure()["in_flight"], 1)  # another group member may be ours
        self.global_runs[0]["event"] = "pull_request"
        self.assertEqual(self.measure()["parked_red"], 1)

    def test_run_inventory_is_not_partitioned_by_status(self):
        self.global_runs = [{"id": 80, "status": "queued", "event": "merge_group"}]
        self.assertEqual(self.measure()["in_flight"], 1)
        inventories = [c[1] for c in self.calls if c[0] == "api" and "created=" in c[1]]
        self.assertEqual(len(inventories), 1)
        self.assertNotIn("status=", inventories[0])
        self.assertIn("2026-11-11T10", inventories[0])

    def test_earliest_candidate_boundary_and_pre_pr_retry(self):
        self.prs.append(pr(2, createdAt="2026-11-10T09:00:00Z"))
        self.measure()
        inventory = next(c[1] for c in self.calls if c[0] == "api" and "created=" in c[1])
        self.assertIn("2026-11-10T09", inventory)
        self.global_runs = [{"id": 70, "status": "requested", "event": "merge_group",
                             "created_at": "2026-11-10T09:00:00Z"}]
        self.assertEqual(self.measure()["in_flight"], 2)
        self.global_runs = []
        self.runs = [{"id": 71, "status": "queued", "head_sha": HEAD,
                      "created_at": "2026-11-09T09:00:00Z"}]
        self.assertEqual(self.measure()["in_flight"], 2)

    def test_truncated_inventory_closes_even_with_spare_capacity(self):
        self.bad = "inventory"
        self.merged = [{"number": n} for n in range(1, 5)]
        with patch.object(bp, "gh_json", side_effect=self.github), patch.object(bp, "now", return_value=CLOCK), \
                patch("sys.argv", ["backpressure"]), contextlib.redirect_stdout(io.StringIO()) as out:
            self.assertEqual(bp.main(), bp.EXIT_CLOSED)
        self.assertEqual(json.loads(out.getvalue())["headroom"], 0)

    def test_malformed_run_association_closes_even_with_spare_capacity(self):
        self.merged = [{"number": n} for n in range(1, 5)]
        for number in [None, "1", False, 0]:
            self.global_runs = [{"id": 80, "status": "in_progress", "event": "pull_request",
                                 "pull_requests": [{"number": number}]}]
            with self.subTest(number=number), patch.object(bp, "gh_json", side_effect=self.github), \
                    patch.object(bp, "now", return_value=CLOCK), patch("sys.argv", ["backpressure"]), \
                    contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(bp.main(), bp.EXIT_CLOSED)
                self.assertEqual(json.loads(out.getvalue())["headroom"], 0)

    def test_head_advancing_during_run_read_conserved(self):
        self.final_head = "b" * 40
        self.assertEqual(self.measure()["in_flight"], 1)

    def test_unpublished_active_suite_conserved(self):
        for status in ["requested", "queued", "in_progress", None]:
            self.suites = [{"id": 8, "status": status, "head_sha": HEAD}]
            self.assertEqual(self.measure()["in_flight"], 1)
        self.suites[0]["status"] = "completed"
        self.assertEqual(self.measure()["parked_red"], 1)

    def test_duplicate_check_contexts_rejected(self):
        self.contexts = [check(), check()]
        with self.assertRaises(ValueError):
            self.measure()

    def test_open_accounting_does_not_override_caller_holds_or_reviews(self):
        self.assertEqual(self.measure()["state"], "open")
        for blocked, reason in [({"isDraft": True}, "draft"),
                                ({"body": "Do not enqueue: investigating DNS"}, "hold"),
                                ({"reviewDecision": "CHANGES_REQUESTED"}, "changes requested")]:
            with self.subTest(blocked=blocked):
                allowed, why = ai_rebase.should_enqueue(blocked)
                self.assertFalse(allowed)
                self.assertIn(reason, why)

    def test_draft_and_unarmed_not_new_admission(self):
        self.prs = [pr(isDraft=True), pr(2, autoMergeRequest=None)]
        self.assertEqual(self.measure()["in_flight"], 0)


if __name__ == "__main__":
    unittest.main()
