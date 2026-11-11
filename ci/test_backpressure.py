#!/usr/bin/env -S uv run --script
"""Deterministic admission accounting; no network or runner mutations."""
import contextlib
import copy
import datetime as dt
import io
import json
import unittest
from unittest.mock import patch

from ci import backpressure as bp


HEAD = "a" * 40
CLOCK = dt.datetime(2026, 11, 11, 12, tzinfo=dt.timezone.utc)


def connection(nodes, **overrides):
    return {"nodes": nodes, "totalCount": len(nodes),
            "pageInfo": {"hasNextPage": False, "endCursor": None}, **overrides}


def check(conclusion="FAILURE", status="COMPLETED", head=HEAD):
    return {"__typename": "CheckRun", "status": status, "conclusion": conclusion,
            "checkSuite": {"commit": {"oid": head}}}


def pr(number=1, **overrides):
    return {"number": number, "isDraft": False, "autoMergeRequest": {"enabledAt": "now"},
            "mergeStateStatus": "BLOCKED", "headRefOid": HEAD, "headRefName": "fix/probe",
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

    def github(self, *args):
        self.calls.append(args)
        if args[0] == "pr":
            return copy.deepcopy(self.merged if "merged" in args else self.prs)
        if args[0] == "run":
            return copy.deepcopy(self.queued)
        if args[0] == "api":
            if self.bad == "runs":
                return [{"total_count": 2, "workflow_runs": self.runs}]
            return [{"total_count": len(self.runs), "workflow_runs": self.runs}]
        if args[0] == "graphql":
            query = args[-1]
            if "mergeQueue" in query:
                entries = connection([{"pullRequest": {"number": n}} for n in self.queue])
                if self.bad == "queue":
                    entries["pageInfo"]["hasNextPage"] = True
                return {"data": {"repository": {"mergeQueue": {"entries": entries}}}}
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
                                ([check(None)], "FAILURE"), ([{"__typename": "FutureCheck"}], "FAILURE"),
                                ([check(), check(None, "IN_PROGRESS")], "FAILURE"),
                                ([check(), {"__typename": "StatusContext", "state": "PENDING",
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
        self.merged = [{"number": n} for n in range(4)]
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

    def test_draft_and_unarmed_not_new_admission(self):
        self.prs = [pr(isDraft=True), pr(2, autoMergeRequest=None)]
        self.assertEqual(self.measure()["in_flight"], 0)


if __name__ == "__main__":
    unittest.main()
