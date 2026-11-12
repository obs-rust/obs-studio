#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["prooft @ git+https://github.com/forge-ci/prooft@683bcf0b3fcf77548054c85991f2b03fdaca77bd"]
# ///
"""Offline trust-contract tests: local git objects and authenticated API fixtures."""

from copy import deepcopy
from dataclasses import replace
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from prooft.attestation import Attestation
from prooft.git import Git
from prooft.keys import lane_key
from prooft.policy import Policy

import pr_attestation as subject

NOW = 1_794_520_000
FMT = "rust/all/fmt"
LINUX = "rust/x86_64-unknown-linux-gnu/test"
WINDOWS = "rust/x86_64-pc-windows-msvc/test"
POLICY = """version: 1
lanes:
  lint: {max-age-hours: 24}
  rust-test: {max-age-hours: 24}
  windows-native: {max-age-hours: 24}
gates:
  rust/all/fmt: {lane: lint}
  rust/x86_64-unknown-linux-gnu/test: {lane: rust-test}
  rust/x86_64-pc-windows-msvc/test: {lane: windows-native}
jobs:
  check-format.yaml:checks: [rust/all/fmt]
  build-project.yaml:rust-tests-linux: [rust/all/fmt, rust/x86_64-unknown-linux-gnu/test]
  build-project.yaml:rust-tests-windows: [rust/x86_64-pc-windows-msvc/test]
"""


class TrustContract(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="pr-attestation-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Local fixture")
        self.git("config", "user.email", "fixture@example.invalid")
        self.write("ci-attestations.yml", POLICY)
        self.write("ci/pr_attestation.py", Path(subject.__file__).read_text())
        self.write("ci/gh_graphql.py", "# base-owned fixture wrapper\n")
        self.write("src/value.txt", "base\n")
        self.git("add", ".")
        self.base_tree = self.git("write-tree")
        self.base = self.git("commit-tree", self.base_tree, input_text="base")
        self.write("src/value.txt", "head\n")
        self.git("add", "src/value.txt")
        self.tree = self.git("write-tree")
        self.head = self.commit([self.att(FMT)])
        self.facts = {
            "data": {"repository": {
                "nameWithOwner": "obs-rust/obs-studio",
                "pullRequest": {
                    "number": 17,
                    "baseRefOid": self.base,
                    "headRefOid": self.head,
                    "headRepository": {"nameWithOwner": "obs-rust/obs-studio"},
                    "author": {"login": "private-member", "__typename": "User"},
                },
                "object": {"oid": self.head, "signature": {
                    "isValid": True, "state": "VALID",
                    "signer": {"login": "signer-member", "__typename": "User"},
                }},
            }},
        }
        self.calls = []
        self.memberships = {}

    def git(self, *args, input_text=None):
        result = subprocess.run(
            ["git", "-C", str(self.root), *args], input=input_text,
            text=True, capture_output=True, check=True,
        )
        return result.stdout.strip()

    def write(self, path, content):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)

    def att(self, gate, **changes):
        lanes = {FMT: "lint", LINUX: "rust-test", WINDOWS: "windows-native"}
        att = Attestation.new(
            gate, tree=self.tree, parents=(self.base,), lane=lanes[gate],
            at=NOW - 30, host="linux-x64",
        )
        return replace(att, **changes).sealed()

    def commit(self, records=(), *, extra="", tree=None, parents=None):
        message = "fixture\n\n" + "\n".join(a.to_trailer() for a in records) + "\n" + extra
        parent_args = [x for parent in (parents or (self.base,)) for x in ("-p", parent)]
        return self.git("commit-tree", tree or self.tree, *parent_args, input_text=message)

    def graphql(self, query, variables):
        self.calls.append(("graphql", variables))
        return deepcopy(self.facts)

    def rest(self, path):
        self.calls.append(("rest", path))
        login = path.rsplit("/", 1)[1]
        answer = self.memberships.get(login, {"state": "active", "user": {"login": login}})
        if isinstance(answer, Exception):
            raise answer
        return deepcopy(answer)

    def evaluate(self, *, head=None, base=None, event="pull_request", now=NOW):
        if head:
            self.head = head
            repo = self.facts["data"]["repository"]
            repo["pullRequest"]["headRefOid"] = head
            repo["object"]["oid"] = head
        return subject.evaluate(
            self.root, base=base or self.base, head=self.head, number=17,
            event=event, graphql=self.graphql, rest=self.rest, now=now,
        )

    def assert_runs(self, result, reason=None):
        self.assertFalse(result["trusted"], result)
        self.assertTrue(result["jobs"], result)
        self.assertFalse(any(job["skip"] for job in result["jobs"].values()), result)
        self.assertTrue(all(job["reasons"] for job in result["jobs"].values()))
        if reason:
            self.assertIn(reason, json.dumps(result))

    def test_verified_private_member_partial_evidence_and_distinct_signer(self):
        result = self.evaluate()
        self.assertTrue(result["trusted"], result)
        self.assertTrue(result["jobs"]["check-format.yaml:checks"]["skip"])
        self.assertFalse(result["jobs"]["build-project.yaml:rust-tests-linux"]["skip"])
        self.assertFalse(result["jobs"]["build-project.yaml:rust-tests-windows"]["skip"])
        paths = [value for kind, value in self.calls if kind == "rest"]
        self.assertEqual(paths, ["orgs/obs-rust/memberships/private-member", "orgs/obs-rust/memberships/signer-member"])

    def test_fork_and_nonmember_replay_identical_trailers(self):
        self.facts["data"]["repository"]["pullRequest"]["headRepository"]["nameWithOwner"] = "outsider/fork"
        self.assert_runs(self.evaluate(), "same-repository")
        self.facts["data"]["repository"]["pullRequest"]["headRepository"]["nameWithOwner"] = "obs-rust/obs-studio"
        self.memberships["private-member"] = {"state": "pending"}
        self.assert_runs(self.evaluate(), "membership")

    def test_denied_membership_and_unverified_signer_membership(self):
        for user in ("private-member", "signer-member"):
            with self.subTest(user=user):
                self.memberships = {user: PermissionError("HTTP 403")}
                self.assert_runs(self.evaluate(), "membership")

    def test_unsigned_invalid_unknown_and_arbitrary_author_identities(self):
        original = deepcopy(self.facts)
        for signature in (None, {}, {"isValid": False}, {"isValid": True, "state": "VALID", "signer": None},
                          {"isValid": True, "state": "UNKNOWN", "signer": {"login": "member"}}):
            with self.subTest(signature=signature):
                self.facts = deepcopy(original)
                repo = self.facts["data"]["repository"]
                repo["object"]["signature"] = signature
                repo["object"]["author"] = {"user": {"login": "private-member"}}
                self.assert_runs(self.evaluate(), "signature")
        self.facts = deepcopy(original)
        self.facts["data"]["repository"]["pullRequest"]["author"]["__typename"] = "Bot"
        self.assert_runs(self.evaluate(), "identity")

    def test_exact_head_base_and_pr_number_required(self):
        for container, field in (("object", "oid"), ("pullRequest", "headRefOid"),
                                 ("pullRequest", "baseRefOid"), ("pullRequest", "number")):
            with self.subTest(field=field):
                target = self.facts["data"]["repository"][container]
                original = target[field]
                target[field] = "wrong"
                self.assert_runs(self.evaluate(), "identity")
                target[field] = original

    def test_api_errors_and_missing_facts_are_observable(self):
        for facts in ({}, {"errors": [{"message": "denied"}]}, {"data": {"repository": None}}):
            self.facts = facts
            self.assert_runs(self.evaluate(), "identity")

    def test_all_non_pr_events_ignore_valid_trailers(self):
        for event in ("merge_group", "push", "schedule", "release", "workflow_dispatch", "pull_request_target", ""):
            with self.subTest(event=event):
                self.assert_runs(self.evaluate(event=event), "event")
        self.assertEqual(self.calls, [])

    def test_invalid_records_reject_all_credit(self):
        cases = {
            "wrong-tree": self.att(FMT, tree=self.base_tree),
            "wrong-parents": self.att(FMT, parents=()),
            "stale": self.att(FMT, at=NOW - 86401),
            "future": self.att(FMT, at=NOW + 301),
            "invalid-time": self.att(FMT, at=0),
            "lane-mismatch": self.att(FMT, lane="cross"),
            "bad-stamp": replace(self.att(FMT), stamp="bad"),
            "undeclared": self.att(FMT, gate="rust/unknown/test"),
        }
        for name, record in cases.items():
            with self.subTest(name=name):
                self.assert_runs(self.evaluate(head=self.commit([record])), "evidence")

    def test_conflicting_duplicate_after_valid_record_is_rejected(self):
        self.assert_runs(self.evaluate(head=self.commit([self.att(FMT), self.att(FMT, at=NOW - 100)])), "duplicate")

    def test_malformed_trailers_even_beside_valid_evidence(self):
        for body in ("not json", "{}", '{"v":99}', self.att(FMT).to_json().replace(str(NOW - 30), '"bad"')):
            with self.subTest(body=body):
                self.assert_runs(self.evaluate(head=self.commit([self.att(FMT)], extra="Ci-Attestation: " + body)), "malformed")

    def test_missing_and_legacy_only_evidence_do_not_claim_target_gates(self):
        result = self.evaluate(head=self.commit(extra="Local-Gate: lint=pass cross=pass windows-native=pass"))
        self.assertFalse(any(job["skip"] for job in result["jobs"].values()))
        self.assertIn("missing", json.dumps(result))

    def test_compile_only_gate_cannot_claim_native_runtime(self):
        result = self.evaluate(head=self.commit([self.att(FMT), self.att(LINUX)]))
        self.assertTrue(result["jobs"]["build-project.yaml:rust-tests-linux"]["skip"])
        self.assertFalse(result["jobs"]["build-project.yaml:rust-tests-windows"]["skip"])
        self.assert_runs(self.evaluate(head=self.commit([self.att(WINDOWS, lane="cross")])), "lane-mismatch")

    def test_trust_surface_edits_and_renames_force_real_checks(self):
        paths = ("push", "ci/new.py", ".github/actions/a/action.yml", "local-gate.toml", "ci-attestations.yml",
                 "rust-toolchain.toml", "Cargo.toml", "rust/a/Cargo.toml", "Cargo.lock", ".cargo/config.toml")
        for path in paths:
            with self.subTest(path=path):
                blob = self.git("hash-object", "-w", "--stdin", input_text="untrusted\n")
                self.git("update-index", "--add", "--cacheinfo", "100644", blob, path)
                tree = self.git("write-tree")
                self.assert_runs(self.evaluate(head=self.commit(tree=tree)), "trust-surface")
                self.git("read-tree", self.tree)
        self.git("update-index", "--force-remove", "ci/pr_attestation.py")
        self.assert_runs(self.evaluate(head=self.commit(tree=self.git("write-tree"))), "trust-surface")

    def test_working_tree_policy_is_never_used(self):
        self.write("ci-attestations.yml", "version: 999\n")
        result = self.evaluate()
        self.assertTrue(result["trusted"], result)

    def test_unknown_base_policy_and_unavailable_objects_fail_closed(self):
        self.assert_runs(self.evaluate(base="0" * 40), "commit")
        self.assert_runs(self.evaluate(head="0" * 40), "commit")
        self.write("ci-attestations.yml", POLICY.replace("version: 1", "version: 99"))
        self.git("add", "ci-attestations.yml")
        bad_base = self.git("commit-tree", self.git("write-tree"), input_text="bad policy")
        self.assert_runs(self.evaluate(base=bad_base), "policy")

    def test_declared_input_binding_uses_base_patterns(self):
        policy_text = POLICY.replace("lint: {max-age-hours: 24}", 'lint: {max-age-hours: 24, inputs: ["src/**"]}')
        self.write("ci-attestations.yml", policy_text)
        self.git("add", "ci-attestations.yml")
        self.tree = self.git("write-tree")
        self.base = self.git("commit-tree", self.tree, input_text="input policy")
        self.facts["data"]["repository"]["pullRequest"]["baseRefOid"] = self.base
        key = lane_key(Git(self.root), self.base, "lint", ("src/**",))
        record = self.att(FMT, tree="1" * 40, parents=("2" * 40,), key=key)
        self.assertTrue(self.evaluate(head=self.commit([record]))["trusted"])
        self.assert_runs(self.evaluate(head=self.commit([replace(record, key="wrong").sealed()])), "evidence")

    def test_executable_must_match_immutable_base(self):
        self.write("ci/pr_attestation.py", "raise RuntimeError('head code')\n")
        self.git("add", "ci/pr_attestation.py")
        bad_base = self.git("commit-tree", self.git("write-tree"), input_text="other verifier")
        self.assert_runs(self.evaluate(base=bad_base), "verifier")

    def test_output_schema_and_github_outputs_are_stable(self):
        result = self.evaluate()
        self.assertEqual(result["schema_version"], 1)
        output = io.StringIO()
        subject.write_outputs(result, output)
        lines = dict(line.split("=", 1) for line in output.getvalue().splitlines())
        self.assertEqual(lines["trusted"], "true")
        self.assertEqual(lines["skip_checks"], "true")
        self.assertEqual(lines["skip_rust-tests-windows"], "false")
        self.assertIsInstance(json.loads(lines["reasons_rust-tests-windows"]), list)
        self.assertEqual(result, self.evaluate())

    def test_repository_policy_names_all_six_targets_with_explicit_freshness(self):
        policy = Policy.load(Path(__file__).resolve().parents[1] / "ci-attestations.yml")
        for target in ("x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu", "x86_64-pc-windows-msvc",
                       "aarch64-pc-windows-msvc", "x86_64-apple-darwin", "aarch64-apple-darwin"):
            for check in ("clippy", "dylint"):
                gate = policy.gates[f"rust/{target}/{check}"]
                self.assertEqual(policy.max_age_hours[gate.lane], 24)


if __name__ == "__main__":
    unittest.main()
