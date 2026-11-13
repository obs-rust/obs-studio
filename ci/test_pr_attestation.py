#!/usr/bin/env -S uv run --script
# /// script
# dependencies = []
# ///
"""Offline trust-contract tests: local git objects and injected API responses.

Plain discovery requires only the standard library. To exercise the real
pinned prooft adapter as well, after its dependency has been cached:
UV_OFFLINE=true uv run --no-project ci/test_pr_attestation.py --integration
GitHub identity/membership remains injected; no network or signing keys.
"""

from copy import deepcopy
from dataclasses import dataclass, replace
import hashlib
import re
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import pr_attestation as subject

INTEGRATION = "--integration" in sys.argv
if INTEGRATION:
    sys.argv.remove("--integration")


@dataclass(frozen=True)
class RecordFixture:
    """Prooft v1 input fixture and injected validation state, never a verifier."""

    gate: str
    tree: str
    parents: tuple
    lane: str
    key: str = ""
    via: str = "run"
    at: int = 0
    host: str = "linux-x64"
    secs: int | None = None
    stamp: str = ""
    state: str = "valid"

    def body(self):
        return {
            "v": 1,
            "gate": self.gate,
            "tree": self.tree,
            "parents": list(self.parents),
            "lane": self.lane,
            "key": self.key,
            "via": self.via,
            "secs": self.secs,
            "host": self.host,
            "at": self.at,
        }

    def sealed(self):
        body = json.dumps(self.body(), sort_keys=True, separators=(",", ":"))
        payload = f"{self.tree}\n{','.join(self.parents)}\n{body}"
        return replace(self, stamp=hashlib.sha256(payload.encode()).hexdigest()[:32])

    def to_json(self):
        return json.dumps(
            {**self.body(), "stamp": self.stamp}, sort_keys=True, separators=(",", ":")
        )

    def to_trailer(self):
        return "Ci-Attestation: " + self.to_json()


NOW = 1_794_520_000
FMT = "rust/all/fmt"
LINUX = "rust/x86_64-unknown-linux-gnu/test"
WINDOWS = "rust/x86_64-pc-windows-msvc/test"
TIER2 = "cpp/windows-x64/tier2"
POLICY = """version: 1
lanes:
  lint: {max-age-hours: 24}
  rust-test: {max-age-hours: 24}
  windows-native: {max-age-hours: 24}
gates:
  rust/all/fmt: {lane: lint}
  rust/x86_64-unknown-linux-gnu/test: {lane: rust-test}
  rust/x86_64-pc-windows-msvc/test: {lane: windows-native}
  cpp/windows-x64/tier2: {lane: windows-native}
jobs:
  check-format.yaml:checks: [rust/all/fmt]
  build-project.yaml:rust-tests-linux: [rust/all/fmt, rust/x86_64-unknown-linux-gnu/test]
  build-project.yaml:rust-tests-windows: [rust/x86_64-pc-windows-msvc/test]
  build-project.yaml:rust-tier2-windows: [cpp/windows-x64/tier2]
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
        self.proof_reports = {}
        self.head = self.commit([self.att(FMT)])
        self.facts = {
            "data": {
                "repository": {
                    "nameWithOwner": "obs-rust/obs-studio",
                    "pullRequest": {
                        "number": 17,
                        "baseRefOid": self.base,
                        "headRefOid": self.head,
                        "headRepository": {"nameWithOwner": "obs-rust/obs-studio"},
                        "author": {"login": "private-member", "__typename": "User"},
                    },
                    "object": {
                        "oid": self.head,
                        "signature": {
                            "isValid": True,
                            "state": "VALID",
                            "signer": {"login": "signer-member", "__typename": "User"},
                        },
                    },
                }
            },
        }
        self.calls = []
        self.memberships = {}

    def git(self, *args, input_text=None):
        result = subprocess.run(
            ["git", "-C", str(self.root), *args],
            input=input_text,
            text=True,
            capture_output=True,
            check=True,
        )
        return result.stdout.strip()

    def write(self, path, content):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(content)

    def att(self, gate_name, **changes):
        lanes = {
            FMT: "lint",
            LINUX: "rust-test",
            WINDOWS: "windows-native",
            TIER2: "windows-native",
        }
        record = RecordFixture(
            gate_name,
            tree=self.tree,
            parents=(self.base,),
            lane=lanes[gate_name],
            at=NOW - 30,
        )
        return replace(record, **changes).sealed()

    def commit(self, records=(), *, extra="", tree=None, parents=None):
        message = (
            "fixture\n\n" + "\n".join(a.to_trailer() for a in records) + "\n" + extra
        )
        parent_args = [
            x for parent in (parents or (self.base,)) for x in ("-p", parent)
        ]
        sha = self.git(
            "commit-tree", tree or self.tree, *parent_args, input_text=message
        )
        seen = {record.gate for record in records}
        self.proof_reports[sha] = {
            "attestations": [
                {"gate": record.gate, "via": record.via} for record in records
            ],
            "malformed": [],
            "keys": {"lint": "fixture-of-prooft-declared-input-key"},
            "statuses": [
                {"gate": record.gate, "state": record.state} for record in records
            ]
            + [
                {"gate": gate, "state": "missing"}
                for gate in (FMT, LINUX, WINDOWS, TIER2)
                if gate not in seen
            ],
        }
        return sha

    def proofs(self, request):
        # Pinned prooft API response fixtures. No package download or network
        # is needed by the plain ci self-test runner.
        if INTEGRATION:
            return subject.prooft_api(request)
        if request["operation"] == "evidence":
            return deepcopy(self.proof_reports[request["head"]])
        if not request["text"].startswith("version: 1\n"):
            raise ValueError("policy: unsupported version")
        return {
            "gates": {
                FMT: {"lane": "lint"},
                LINUX: {"lane": "rust-test"},
                WINDOWS: {"lane": "windows-native"},
                TIER2: {"lane": "windows-native"},
            },
            "jobs": {
                "check-format.yaml:checks": [FMT],
                "build-project.yaml:rust-tests-linux": [FMT, LINUX],
                "build-project.yaml:rust-tests-windows": [WINDOWS],
                "build-project.yaml:rust-tier2-windows": [TIER2],
            },
            "lanes": ["lint", "rust-test", "windows-native"],
            "max_age_hours": {"lint": 24, "rust-test": 24, "windows-native": 24},
        }

    def graphql(self, query, variables):
        self.calls.append(("graphql", variables))
        return deepcopy(self.facts)

    def rest(self, path):
        self.calls.append(("rest", path))
        login = path.rsplit("/", 1)[1]
        answer = self.memberships.get(
            login, {"state": "active", "user": {"login": login}}
        )
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
            self.root,
            base=base or self.base,
            head=self.head,
            number=17,
            event=event,
            graphql=self.graphql,
            rest=self.rest,
            now=now,
            proofs=self.proofs,
        )

    def assert_runs(self, result, reason=None):
        self.assertFalse(result["trusted"], result)
        self.assertTrue(result["jobs"], result)
        self.assertFalse(any(job["skip"] for job in result["jobs"].values()), result)
        self.assertTrue(all(job["reasons"] for job in result["jobs"].values()))
        if reason:
            self.assertIn(reason, json.dumps(result))

    def complete_client(self, missing=()):
        records = [RecordFixture(gate, self.tree, (self.base,), "lint", at=NOW - 30).sealed()
                   for gate in subject.CLIENT_GATES if gate not in missing]
        return self.commit(records)

    def planned(self, files=("rust/obs-util/src/value.rs",), **kwargs):
        result = self.evaluate(**kwargs)
        return subject.admission(result, files, event=kwargs.get("event", "pull_request"))

    def test_complete_verified_member_allocates_only_admission_jobs(self):
        plan = self.planned(head=self.complete_client())
        self.assertTrue(plan["fast_path"], plan)
        self.assertEqual(plan["queue_policy"], "heavy-v1")
        self.assertTrue(all(job["skip"] for job in plan["jobs"].values()), plan)
        self.assertTrue(plan["jobs"]["build-project.yaml:rust-windows-link"]["deferred"])

    def test_each_missing_client_gate_runs_real_affected_work_and_all_heavy_work(self):
        for gate in subject.CLIENT_GATES:
            with self.subTest(gate=gate):
                plan = self.planned(head=self.complete_client(missing={gate}))
                self.assertFalse(plan["fast_path"])
                for name in subject.DEFERRED_JOBS:
                    self.assertFalse(plan["jobs"][name]["skip"], (gate, plan))
                self.assertTrue(any(not j["skip"] and not j.get("deferred")
                                    for n, j in plan["jobs"].items() if n not in subject.DEFERRED_JOBS))

    def test_event_to_execution_outsider_and_error_matrix(self):
        head = self.complete_client()
        self.evaluate(head=head)
        original = deepcopy(self.facts)
        cases = ("fork", "nonmember", "private-denied", "unsigned", "api-error")
        for case in cases:
            with self.subTest(case=case):
                self.facts = deepcopy(original)
                self.memberships = {}
                repo = self.facts["data"]["repository"]
                if case == "fork":
                    repo["pullRequest"]["headRepository"]["nameWithOwner"] = "outsider/obs-studio"
                elif case == "nonmember":
                    self.memberships["private-member"] = {"state": "pending"}
                elif case == "private-denied":
                    self.memberships["private-member"] = RuntimeError("403")
                elif case == "unsigned":
                    repo["object"]["signature"] = None
                else:
                    self.facts = {"errors": [{"message": "denied"}]}
                plan = self.planned()
                self.assertFalse(plan["fast_path"], plan)
                for name, state in plan["jobs"].items():
                    if name not in ("check-format.yaml:swift-format", "ci.yml:validate-data"):
                        self.assertFalse(state["skip"], (case, name, plan))

    def test_selected_swift_data_and_docs_require_relevant_client_gates(self):
        self.evaluate(head=self.complete_client())
        for path, gate in (("plugins/mac/foo.swift", "swift/all/fmt"),
                           ("plugins/rtmp-services/data/services.json", "data/all/services-schema"),
                           ("plugins/win-capture/data/compatibility.json", "data/all/compatibility")):
            with self.subTest(path=path):
                plan = self.planned(files=(path,))
                self.assertFalse(plan["fast_path"], plan)
                self.assertFalse(plan["jobs"]["check-format.yaml:swift-format" if path.endswith(".swift")
                                               else "ci.yml:validate-data"]["skip"])
                result = self.evaluate()
                result["gates"][gate] = "valid"
                self.assertTrue(subject.admission(result, (path,), event="pull_request")["fast_path"])
        self.assertTrue(self.planned(files=("docs/guide.md",))["fast_path"])

    def test_queue_and_malformed_scope_never_take_fast_path(self):
        self.evaluate(head=self.complete_client())
        for event in ("merge_group", "push", "workflow_dispatch"):
            plan = self.planned(event=event)
            self.assertFalse(plan["fast_path"])
            self.assertFalse(any(j["skip"] for j in plan["jobs"].values()))
        plan = subject.admission(self.evaluate(), None, event="pull_request")
        self.assertFalse(any(j["skip"] for j in plan["jobs"].values()))

    def test_legacy_local_gate_is_diagnostic_only(self):
        plan = self.planned(head=self.commit([self.att(FMT), self.att(LINUX)],
                                            extra="Local-Gate: legacy-tree-stamp"))
        self.assertFalse(plan["fast_path"])
        self.assertFalse(plan["jobs"]["build-project.yaml:rust-tests-linux"]["skip"])

    def test_workflow_consumes_only_base_owned_unified_decision(self):
        root = Path(__file__).resolve().parents[1]
        build = (root / ".github/workflows/build-project.yaml").read_text()
        mode = build.split("  ci-mode:\n", 1)[1].split("  rust-tests-linux:", 1)[0]
        self.assertIn('git show "$BASE:ci/pr_attestation.py"', mode)
        self.assertIn('uv run --no-project', mode)
        self.assertNotIn("ATTEST_MODE", mode)
        self.assertNotIn("local-gate verify", mode)
        self.assertNotIn("ci/lane_proof.py check", mode)
        self.assertIn("steps.admission.outputs.skip_rust-tests-linux", mode)
        self.assertIn("swift: ${{ steps.swift.outputs.swift }}", build)

    def test_verified_private_member_partial_evidence_and_distinct_signer(self):
        result = self.evaluate()
        self.assertTrue(result["trusted"], result)
        self.assertTrue(result["jobs"]["check-format.yaml:checks"]["skip"])
        self.assertFalse(result["jobs"]["build-project.yaml:rust-tests-linux"]["skip"])
        self.assertFalse(
            result["jobs"]["build-project.yaml:rust-tests-windows"]["skip"]
        )
        paths = [value for kind, value in self.calls if kind == "rest"]
        self.assertEqual(
            paths,
            [
                "orgs/obs-rust/memberships/private-member",
                "orgs/obs-rust/memberships/signer-member",
            ],
        )

    def test_fork_and_nonmember_replay_identical_trailers(self):
        self.facts["data"]["repository"]["pullRequest"]["headRepository"][
            "nameWithOwner"
        ] = "outsider/fork"
        self.assert_runs(self.evaluate(), "same-repository")
        self.facts["data"]["repository"]["pullRequest"]["headRepository"][
            "nameWithOwner"
        ] = "obs-rust/obs-studio"
        self.memberships["private-member"] = {"state": "pending"}
        self.assert_runs(self.evaluate(), "membership")

    def test_denied_membership_and_unverified_signer_membership(self):
        for user in ("private-member", "signer-member"):
            with self.subTest(user=user):
                self.memberships = {user: PermissionError("HTTP 403")}
                self.assert_runs(self.evaluate(), "membership")

    def test_pinned_prooft_adapter_is_isolated_and_has_no_floating_version(self):
        answer = subprocess.CompletedProcess([], 0, stdout='{"fixture":true}')
        with patch.object(subject.subprocess, "run", return_value=answer) as invoke:
            self.assertEqual(
                subject.prooft_api({"operation": "policy", "text": POLICY}),
                {"fixture": True},
            )
        args = invoke.call_args.args[0]
        self.assertEqual(
            args[:5], ["uv", "run", "--no-project", "--with", subject.PROOFT_SOURCE]
        )
        self.assertEqual(
            " ".join(args[:8]),
            f"uv run --no-project --with {subject.PROOFT_SOURCE} python -I -c",
        )
        self.assertIn(
            "@d08d58bebcd8592e4526b9bde9ca4717966a8786", subject.PROOFT_SOURCE
        )
        self.assertNotEqual(Path(invoke.call_args.kwargs["cwd"]), self.root)

    def test_prooft_errors_timeouts_and_unknown_results_fail_closed(self):
        for error in (
            OSError("unavailable"),
            subprocess.TimeoutExpired("prooft", 120),
            ValueError("bad JSON"),
        ):
            with (
                self.subTest(error=error),
                patch.object(self, "proofs", side_effect=error),
            ):
                self.assert_runs(self.evaluate(), "evidence")
        for result in ({}, {"gates": None}, {"attestations": []}):
            with (
                self.subTest(result=result),
                patch.object(self, "proofs", return_value=result),
            ):
                self.assert_runs(self.evaluate())

    def test_unsigned_invalid_unknown_and_arbitrary_author_identities(self):
        original = deepcopy(self.facts)
        for signature in (
            None,
            {},
            {"isValid": False},
            {"isValid": True, "state": "VALID", "signer": None},
            {"isValid": True, "state": "UNKNOWN", "signer": {"login": "member"}},
        ):
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
        for container, field in (
            ("object", "oid"),
            ("pullRequest", "headRefOid"),
            ("pullRequest", "baseRefOid"),
            ("pullRequest", "number"),
        ):
            with self.subTest(field=field):
                target = self.facts["data"]["repository"][container]
                original = target[field]
                target[field] = "wrong"
                self.assert_runs(self.evaluate(), "identity")
                target[field] = original

    def test_api_errors_and_missing_facts_are_observable(self):
        for facts in (
            {},
            {"errors": [{"message": "denied"}]},
            {"data": {"repository": None}},
        ):
            self.facts = facts
            self.assert_runs(self.evaluate(), "identity")

    def test_raising_api_lookup_falls_back_without_privilege_changes(self):
        for error in (
            PermissionError("denied"),
            subprocess.TimeoutExpired("lookup", 1),
            ValueError("invalid JSON"),
        ):
            with (
                self.subTest(error=error),
                patch.object(self, "graphql", side_effect=error),
            ):
                self.assert_runs(self.evaluate(), "identity")

    def test_membership_must_belong_to_queried_identity(self):
        self.memberships["signer-member"] = {
            "state": "active",
            "user": {"login": "other-member"},
        }
        self.assert_runs(self.evaluate(), "membership")

    def test_bot_signer_and_nonmember_collaborator_are_not_identity_proof(self):
        repo = self.facts["data"]["repository"]
        repo["object"]["signature"]["signer"]["__typename"] = "Bot"
        self.assert_runs(self.evaluate(), "identity")
        repo["object"]["signature"]["signer"]["__typename"] = "User"
        repo["pullRequest"]["authorAssociation"] = "COLLABORATOR"
        self.memberships["private-member"] = PermissionError("HTTP 404")
        self.assert_runs(self.evaluate(), "membership")

    def test_all_non_pr_events_ignore_valid_trailers(self):
        for event in (
            "merge_group",
            "push",
            "schedule",
            "release",
            "workflow_dispatch",
            "pull_request_target",
            "",
        ):
            with self.subTest(event=event):
                self.assert_runs(self.evaluate(event=event), "event")
        self.assertEqual(self.calls, [])

    def test_invalid_records_reject_all_credit(self):
        cases = {
            "wrong-tree": self.att(FMT, tree=self.base_tree, state="wrong-tree"),
            "wrong-parents": self.att(FMT, parents=(), state="wrong-parents"),
            "stale": self.att(FMT, at=NOW - 86401, state="stale"),
            "future": self.att(FMT, at=NOW + 301, state="stale"),
            "invalid-time": self.att(FMT, at=0, state="stale"),
            "lane-mismatch": self.att(FMT, lane="cross", state="lane-mismatch"),
            "bad-stamp": replace(self.att(FMT), stamp="bad", state="bad-stamp"),
            "undeclared": self.att(FMT, gate="rust/unknown/test", state="undeclared"),
            "unknown-provenance": self.att(FMT, via="unknown"),
        }
        for name, record in cases.items():
            with self.subTest(name=name):
                self.assert_runs(self.evaluate(head=self.commit([record])), "evidence")

    def test_conflicting_duplicate_after_valid_record_is_rejected(self):
        self.assert_runs(
            self.evaluate(
                head=self.commit([self.att(FMT), self.att(FMT, at=NOW - 100)])
            ),
            "duplicate",
        )

    def test_freshness_boundaries_and_invalid_verification_time(self):
        for at in (NOW - 86400, NOW + 60):
            self.assertTrue(
                self.evaluate(head=self.commit([self.att(FMT, at=at)]))["trusted"]
            )
        for now in (float("nan"), float("inf"), 0, True, "unknown"):
            with self.subTest(now=now):
                self.assert_runs(self.evaluate(now=now), "evidence")

    def test_base_graphql_wrapper_is_used_instead_of_head_script(self):
        self.write(
            "ci/gh_graphql.py", "raise RuntimeError('must never run head wrapper')\n"
        )
        graphql, rest = subject.github_reads(self.root, self.base)
        real_run = subprocess.run

        def invoke(args, **kwargs):
            if args[0] == "git":
                return real_run(args, **kwargs)
            wrapper_path = Path(args[4])
            self.assertEqual(wrapper_path.read_text(), "# base-owned fixture wrapper\n")
            self.assertIn("query=fixture-query", args)
            return subprocess.CompletedProcess(args, 0, stdout='{"data":{}}')

        with patch.object(subject.subprocess, "run", side_effect=invoke):
            self.assertEqual(graphql("fixture-query", {"number": 17}), {"data": {}})

    def test_malformed_trailers_even_beside_valid_evidence(self):
        for body in (
            "not json",
            "{}",
            '{"v":99}',
            self.att(FMT).to_json().replace(str(NOW - 30), '"bad"'),
        ):
            with self.subTest(body=body):
                self.assert_runs(
                    self.evaluate(
                        head=self.commit(
                            [self.att(FMT)], extra="Ci-Attestation: " + body
                        )
                    ),
                    "malformed",
                )

    def test_missing_and_legacy_only_evidence_do_not_claim_target_gates(self):
        result = self.evaluate(
            head=self.commit(
                extra="Local-Gate: lint=pass cross=pass windows-native=pass"
            )
        )
        self.assertFalse(any(job["skip"] for job in result["jobs"].values()))
        self.assertIn("missing", json.dumps(result))

    def test_compile_only_gate_cannot_claim_native_runtime(self):
        result = self.evaluate(head=self.commit([self.att(FMT), self.att(LINUX)]))
        self.assertTrue(result["jobs"]["build-project.yaml:rust-tests-linux"]["skip"])
        self.assertFalse(
            result["jobs"]["build-project.yaml:rust-tests-windows"]["skip"]
        )
        self.assert_runs(
            self.evaluate(
                head=self.commit(
                    [self.att(WINDOWS, lane="cross", state="lane-mismatch")]
                )
            ),
            "lane-mismatch",
        )

    def test_native_unit_evidence_does_not_claim_tier2(self):
        result = self.evaluate(head=self.commit([self.att(WINDOWS)]))
        self.assertTrue(result["jobs"]["build-project.yaml:rust-tests-windows"]["skip"])
        self.assertFalse(
            result["jobs"]["build-project.yaml:rust-tier2-windows"]["skip"]
        )
        self.assert_runs(
            self.evaluate(
                head=self.commit([self.att(TIER2, lane="cross", state="lane-mismatch")])
            ),
            "lane-mismatch",
        )

    def test_trust_surface_edits_and_renames_force_real_checks(self):
        paths = (
            "push",
            "ci/new.py",
            ".github/actions/a/action.yml",
            "local-gate.toml",
            "ci-attestations.yml",
            "rust-toolchain.toml",
            "Cargo.toml",
            "rust/a/Cargo.toml",
            "Cargo.lock",
            ".cargo/config.toml",
        )
        for path in paths:
            with self.subTest(path=path):
                blob = self.git(
                    "hash-object", "-w", "--stdin", input_text="untrusted\n"
                )
                self.git("update-index", "--add", "--cacheinfo", "100644", blob, path)
                tree = self.git("write-tree")
                self.assert_runs(
                    self.evaluate(head=self.commit(tree=tree)), "trust-surface"
                )
                self.git("read-tree", self.tree)
        self.git("update-index", "--force-remove", "ci/pr_attestation.py")
        self.assert_runs(
            self.evaluate(head=self.commit(tree=self.git("write-tree"))),
            "trust-surface",
        )

    def test_working_tree_policy_is_never_used(self):
        self.write("ci-attestations.yml", "version: 999\n")
        result = self.evaluate()
        self.assertTrue(result["trusted"], result)

    def test_unknown_base_policy_and_unavailable_objects_fail_closed(self):
        self.assert_runs(self.evaluate(base="0" * 40), "commit")
        self.assert_runs(self.evaluate(head="0" * 40), "commit")
        self.write("ci-attestations.yml", POLICY.replace("version: 1", "version: 99"))
        self.git("add", "ci-attestations.yml")
        bad_base = self.git(
            "commit-tree", self.git("write-tree"), input_text="bad policy"
        )
        self.assert_runs(self.evaluate(base=bad_base), "policy")

    def test_declared_input_binding_uses_base_patterns(self):
        policy_text = POLICY.replace(
            "lint: {max-age-hours: 24}",
            'lint:\n    max-age-hours: 24\n    inputs: ["src/**"]',
        )
        self.write("ci-attestations.yml", policy_text)
        self.git("add", "ci-attestations.yml")
        self.tree = self.git("write-tree")
        self.base = self.git("commit-tree", self.tree, input_text="input policy")
        self.facts["data"]["repository"]["pullRequest"]["baseRefOid"] = self.base
        key_head = self.commit()
        key = self.proofs(
            {
                "operation": "evidence",
                "text": policy_text,
                "root": str(self.root),
                "head": key_head,
                "now": NOW,
            }
        )["keys"]["lint"]
        record = self.att(FMT, tree="1" * 40, parents=("2" * 40,), key=key)
        self.assertTrue(self.evaluate(head=self.commit([record]))["trusted"])
        self.assert_runs(
            self.evaluate(
                head=self.commit(
                    [replace(record, key="wrong", state="wrong-tree").sealed()]
                )
            ),
            "evidence",
        )

    def test_executable_must_match_immutable_base(self):
        self.write("ci/pr_attestation.py", "raise RuntimeError('head code')\n")
        self.git("add", "ci/pr_attestation.py")
        bad_base = self.git(
            "commit-tree", self.git("write-tree"), input_text="other verifier"
        )
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
        text = (Path(__file__).resolve().parents[1] / "ci-attestations.yml").read_text()
        for target in (
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
            "aarch64-pc-windows-msvc",
            "x86_64-apple-darwin",
            "aarch64-apple-darwin",
        ):
            for check in ("clippy", "dylint"):
                match = re.search(
                    rf"rust/{target}/{check}: \{{lane: ([a-z-]+)\}}", text
                )
                self.assertIsNotNone(match)
                self.assertIn(f"  {match[1]}: {{max-age-hours: 24}}", text)


if __name__ == "__main__":
    unittest.main()
