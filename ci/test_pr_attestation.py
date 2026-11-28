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
import shutil
import io
import json
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest
import zlib
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

# Keep original per-job legacy fixtures while declaring additive client gates.
POLICY = POLICY.replace("gates:\n", "gates:\n" + "".join(
    f"  {gate}: {{lane: lint}}\n" for gate in subject.CLIENT_GATES
))


class ObjectFixture:
    """Write real loose Git objects; fixture construction needs no Git processes.

    Only the test builder uses this. The verifier still reads the resulting
    repository with real Git, including its real merge-base and shallow logic.
    """

    def __init__(self, root):
        self.root = root
        self.index = {}
        self.trees = {}
        self.commits = {}

    def object(self, kind, body):
        raw = f"{kind} {len(body)}\0".encode() + body
        sha = hashlib.sha1(raw).hexdigest()
        path = self.root / ".git/objects" / sha[:2] / sha[2:]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(zlib.compress(raw))
        return sha

    def tree(self, entries):
        children = {}
        for path, sha in entries.items():
            name, sep, rest = path.partition("/")
            if sep:
                children.setdefault(name, {})[rest] = sha
            else:
                children[name] = sha
        body = bytearray()
        for name, value in sorted(children.items(), key=lambda item: item[0] + ("/" if isinstance(item[1], dict) else "")):
            mode, sha = ("40000", self.tree(value)) if isinstance(value, dict) else ("100644", value)
            body.extend(f"{mode} {name}\0".encode() + bytes.fromhex(sha))
        sha = self.object("tree", body)
        self.trees[sha] = dict(entries)
        return sha

    def run(self, *args, input_text=None):
        command, *values = args
        if command == "add":
            paths = sorted(p.relative_to(self.root).as_posix() for p in self.root.rglob("*")
                           if p.is_file() and ".git" not in p.relative_to(self.root).parts) if values == ["."] else values
            for path in paths:
                self.index[path] = self.object("blob", (self.root / path).read_bytes())
        elif command == "write-tree":
            return self.tree(self.index)
        elif command == "commit-tree":
            tree, *parents = values
            headers = [f"tree {tree}"] + [f"parent {parent}" for parent in parents[1::2]]
            identity = "Local fixture <fixture@example.invalid> 1794519000 +0000"
            headers += [f"author {identity}", f"committer {identity}"]
            message = input_text + ("" if input_text.endswith("\n") else "\n")
            sha = self.object("commit", ("\n".join(headers) + "\n\n" + message).encode())
            self.commits[sha] = tree
            return sha
        elif command == "read-tree":
            tree = self.commits.get(values[0], values[0])
            self.index = dict(self.trees[tree])
        elif command == "hash-object":
            return self.object("blob", input_text.encode())
        elif command == "update-index":
            if values[0] == "--force-remove":
                self.index.pop(values[1], None)
            else:
                assert values[:3] == ["--add", "--cacheinfo", "100644"], values
                self.index[values[4]] = values[3]
        else:
            raise AssertionError(f"unsupported fixture operation: {args}")
        return ""


REAL_GIT = subject.Git


class FixtureGit(REAL_GIT):
    """Reuse successful immutable reads across identical fixture copies.

    Keys contain the exact command and full object IDs. Shallow repositories
    always go through Git: shallow boundaries change ancestry without changing
    object IDs. Failures are never cached. All distinct queries execute real Git.
    """

    reads = {}

    def run(self, *args):
        git_dir = self.root / ".git"
        if (args[0] not in {"show", "diff"}
                or any((git_dir / path).exists() for path in
                       ("shallow", "info/grafts", "refs/replace", "packed-refs", "objects/info/alternates"))):
            return super().run(*args)
        ids = re.findall(r"(?<![0-9a-f])[0-9a-f]{40}(?![0-9a-f])", " ".join(args))
        if not ids or not all((self.root / ".git/objects" / sha[:2] / sha[2:]).is_file() for sha in ids):
            return super().run(*args)
        key = (args, (git_dir / "config").read_bytes())
        if cached := self.reads.get(key):
            output, objects = cached
            try:
                if all(hashlib.sha1(zlib.decompress((git_dir / "objects" / sha[:2] / sha[2:]).read_bytes())).hexdigest() == sha
                       for sha in objects):
                    return output
            except (OSError, zlib.error):
                pass
        output = super().run(*args)
        objects = tuple(path.parent.name + path.name for path in (git_dir / "objects").glob("??/*") if path.is_file())
        self.reads[key] = (output, objects)
        return output


class TrustContract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temp = tempfile.TemporaryDirectory(prefix="pr-attestation-template-")
        cls.addClassCleanup(temp.cleanup)
        builder = cls("runTest")
        builder.root = Path(temp.name)
        subprocess.run(["git", "init", "-q", "--object-format=sha1", "--template=", str(builder.root)],
                       check=True, capture_output=True)
        builder.objects = ObjectFixture(builder.root)
        builder.write("ci-attestations.yml", POLICY)
        builder.write("ci/pr_attestation.py", Path(subject.__file__).read_text())
        builder.write("ci/gh_graphql.py", "# base-owned fixture wrapper\n")
        builder.write("src/value.txt", "base\n")
        builder.git("add", ".")
        builder.base_tree = builder.git("write-tree")
        builder.base = builder.git("commit-tree", builder.base_tree, input_text="base")
        builder.write("src/value.txt", "head\n")
        builder.git("add", "src/value.txt")
        builder.tree = builder.git("write-tree")
        builder.proof_reports = {}
        builder.head = builder.commit([builder.att(FMT)])
        cls.template = builder
        FixtureGit.reads = {}
        patcher = patch.object(subject, "Git", FixtureGit)
        patcher.start()
        cls.addClassCleanup(patcher.stop)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="pr-attestation-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        # copytree copies bytes, never hardlinks: index, shallow boundaries,
        # objects and working files are independently mutable in every test.
        shutil.copytree(self.template.root, self.root, dirs_exist_ok=True)
        self.objects = deepcopy(self.template.objects)
        self.objects.root = self.root
        for name in ("base_tree", "base", "tree", "head", "proof_reports"):
            setattr(self, name, deepcopy(getattr(self.template, name)))
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
        self.public_memberships = {}

    def git(self, *args, input_text=None):
        return self.objects.run(*args, input_text=input_text)

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
        if "/public_members/" in path:
            answer = self.public_memberships.get(login, {"public_member": False})
            if isinstance(answer, Exception):
                raise answer
            return deepcopy(answer)
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

    def test_planner_outputs_drive_actual_workflow_job_allocation(self):
        from ci_scope import scope
        from test_merge_queue import job_block

        root = Path(__file__).resolve().parents[1]
        build = (root / ".github/workflows/build-project.yaml").read_text()
        formatting = (root / ".github/workflows/check-format.yaml").read_text()
        paths = ["libobs/obs.c.h", "rust/obs-util/src/value.rs", "frontend/value.cpp", "plugins/win-capture/value.c",
                 "plugins/mac/foo.swift", "plugins/rtmp-services/data/services.json"]
        selected = scope(paths, event="pull_request")
        result = self.evaluate(head=self.complete_client())
        result["gates"].update({"swift/all/fmt": "valid", "data/all/services-schema": "valid"})
        full = subject.admission(result, paths, event="pull_request")
        for case in ("complete", "missing-test", "nonmember", "verifier-error"):
            with self.subTest(case=case):
                current = deepcopy(result)
                if case == "missing-test":
                    current["gates"].pop("rust/x86_64-unknown-linux-gnu/client-test")
                elif case in ("nonmember", "verifier-error"):
                    current["trusted"] = False
                plan = subject.admission(current, paths, event="pull_request")
                outputs = io.StringIO()
                subject.write_outputs(plan, outputs)
                values = dict(line.split("=", 1) for line in outputs.getvalue().splitlines())
                scope_values = {key: str(value).lower() for key, value in vars(selected).items()}
                scope_values["deferred"] = "true"
                scope_values["cross_lint"] = "true"
                scope_values["cross_targets"] = "win-x64"
                for name in ("rust-tests-linux", "rust-cross-lint", "rust-cross-lint-release",
                             "rust-windows-link", "cmocka-link", "cross-compile", "rust-linux-tier2-build",
                             "linux-qt-frontend", "rust-tests-windows-wine", "rust-tier2-windows-wine"):
                    header = job_block(build, name).split("    steps:", 1)[0]
                    condition = re.search(r"^    if: (.+)$", header, re.M)[1]
                    expression = condition.removeprefix("${{").removesuffix("}}")
                    expression = expression.replace("github.event_name", repr("pull_request"))
                    expression = re.sub(r"needs\.check-event\.outputs\.([\w_-]+)",
                                        lambda m: repr(scope_values.get(m[1], "")), expression)
                    expression = re.sub(r"needs\.ci-mode\.outputs\.([\w_-]+)",
                                        lambda m: repr(values.get(m[1], "false")), expression)
                    expression = expression.replace("needs.rust-tests-linux.result", repr("success"))
                    expression = expression.replace("needs.check-event.result", repr("success"))
                    expression = expression.replace("needs.ci-mode.result", repr("success"))
                    expression = re.sub(r"inputs\.[\w_-]+", "False", expression)
                    expression = expression.replace("always()", "True").replace("cancelled()", "False")
                    expression = expression.replace("&&", " and ").replace("||", " or ")
                    expression = re.sub(r"!(?!=)", " not ", expression)
                    allocated = eval(expression, {"__builtins__": {}, "fromJSON": json.loads})
                    must_run = case != "complete"
                    self.assertEqual(bool(allocated), must_run, (case, name, condition, plan))
                swift = job_block(formatting, "swift-format")
                self.assertIn("inputs.swift", swift)
                self.assertEqual(full["jobs"]["check-format.yaml:swift-format"]["skip"], True)

    def test_invalid_bindings_and_trust_edits_retain_execution_plan(self):
        for state in ("expired", "wrong-tree", "wrong-parents", "malformed"):
            records = [RecordFixture(gate, self.tree, (self.base,), "lint", at=NOW - 30,
                                     state=state if index == 0 else "valid").sealed()
                       for index, gate in enumerate(subject.CLIENT_GATES)]
            changes = {"expired": {"at": NOW - 24 * 3600 - 1},
                       "wrong-tree": {"tree": "0" * 40},
                       "wrong-parents": {"parents": ("1" * 40,)}}
            records[0] = replace(records[0], **changes.get(state, {})).sealed()
            extra = "Ci-Attestation: {broken" if state == "malformed" else ""
            plan = self.planned(head=self.commit(records, extra=extra))
            self.assertFalse(plan["fast_path"])
            self.assertFalse(plan["jobs"]["build-project.yaml:rust-windows-link"]["skip"])
        self.write("ci-attestations.yml", "version: 999\n")
        self.git("add", "ci-attestations.yml")
        self.tree = self.git("write-tree")
        plan = self.planned(head=self.complete_client())
        self.assertFalse(plan["fast_path"])
        self.assertIn("trust-surface", json.dumps(plan))

    def test_two_pr_merge_commit_never_uses_member_trailers(self):
        first = self.complete_client()
        second = self.commit([self.att(FMT)], extra="second PR")
        merged = self.commit([], parents=(first, second))
        plan = self.planned(head=merged, event="merge_group")
        self.assertFalse(plan["fast_path"])
        self.assertFalse(any(job["skip"] for job in plan["jobs"].values()))

    def test_workflow_consumes_only_base_owned_unified_decision(self):
        root = Path(__file__).resolve().parents[1]
        build = (root / ".github/workflows/build-project.yaml").read_text()
        mode = build.split("  ci-mode:\n", 1)[1].split("  rust-tests-linux:", 1)[0]
        self.assertIn('git show "$BASE:ci/pr_attestation.py"', mode)
        self.assertIn('uv run --no-project', mode)
        self.assertNotIn("ATTEST_MODE", mode)
        self.assertNotIn("local-gate verify", mode)
        self.assertNotIn("ci/lane_proof.py check", mode)
        self.assertIn("steps.gate.outputs.skip_rust-tests-linux", mode)
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
                "orgs/obs-rust/public_members/private-member",
                "orgs/obs-rust/memberships/private-member",
                "orgs/obs-rust/public_members/signer-member",
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

    def test_public_members_need_no_privileged_org_membership_lookup(self):
        for login in ("private-member", "signer-member"):
            self.public_memberships[login] = {"public_member": True, "user": {"login": login}}
            self.memberships[login] = RuntimeError("org Members read not granted")
        result = self.planned(head=self.complete_client())
        self.assertTrue(result["fast_path"], result)
        self.assertFalse(any("/memberships/" in path for kind, path in self.calls if kind == "rest"))

    def test_public_api_errors_do_not_fall_through_to_private_credit(self):
        self.public_memberships["private-member"] = RuntimeError("HTTP 503")
        self.assert_runs(self.evaluate(), "membership")

    def test_authenticated_public_membership_adapter_handles_only_known_statuses(self):
        graphql, rest = subject.github_reads(self.root, self.base)
        for status, code, expected in (("204 No Content", 0, True), ("404 Not Found", 1, False)):
            result = subprocess.CompletedProcess([], code, stdout=f"HTTP/2.0 {status}\n\n", stderr="")
            with patch.object(subject.subprocess, "run", return_value=result):
                self.assertIs(rest("orgs/obs-rust/public_members/private-member")["public_member"], expected)
        for status in ("403 Forbidden", "500 Internal Server Error", "200 OK"):
            result = subprocess.CompletedProcess([], 1, stdout=f"HTTP/2.0 {status}\n\n", stderr="denied")
            with patch.object(subject.subprocess, "run", return_value=result), self.assertRaises(subprocess.CalledProcessError):
                rest("orgs/obs-rust/public_members/private-member")
        killed = subprocess.CompletedProcess([], -9, stdout="HTTP/2.0 404 Not Found\n", stderr="")
        with patch.object(subject.subprocess, "run", return_value=killed), self.assertRaises(subprocess.CalledProcessError):
            rest("orgs/obs-rust/public_members/private-member")

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
            "ruff.toml",
            "CMakePresets.json",
            ".clang-format",
            ".swift-format",
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

    def cli_plan(self, head):
        # Exercise both production diff consumers with real git history. Only
        # external identity/time/prooft reads are replaced by existing fixtures.
        self.evaluate(head=head)
        evaluate = subject.evaluate
        output = io.StringIO()
        with (
            patch.object(subject, "github_reads", return_value=(self.graphql, self.rest)),
            patch.object(subject, "evaluate", side_effect=lambda *a, **kw: evaluate(*a, **kw, proofs=self.proofs)),
            patch.object(subject.time, "time", return_value=NOW),
            patch.dict(os.environ, {"GITHUB_OUTPUT": ""}),
            patch("sys.stdout", output),
        ):
            self.assertEqual(subject.main([
                "--repo-dir", str(self.root), "--base", self.base,
                "--head", head, "--number", "17", "--event", "pull_request",
            ]), 0)
        return json.loads(output.getvalue())

    def advanced_master(self):
        branch_point = self.base
        self.git("read-tree", branch_point)
        for path in ("ci/master-only.py", "plugins/mac/master.swift",
                     "plugins/rtmp-services/data/services.json"):
            self.write(path, "master-only\n")
            self.git("add", path)
        self.base = self.git("commit-tree", self.git("write-tree"), "-p", branch_point,
                             input_text="master advances")
        self.facts["data"]["repository"]["pullRequest"]["baseRefOid"] = self.base
        self.git("read-tree", branch_point)
        self.write("libobs/value.c.h", "PR-only\n")
        self.git("add", "libobs/value.c.h")
        self.tree = self.git("write-tree")
        return branch_point

    def attested_branch(self, parents):
        records = [RecordFixture(gate, self.tree, parents, "lint", at=NOW - 30).sealed()
                   for gate in subject.CLIENT_GATES]
        return self.commit(records, parents=parents)

    def test_merge_base_excludes_master_only_trust_and_scope_changes(self):
        branch_point = self.advanced_master()
        plan = self.cli_plan(self.attested_branch((branch_point,)))
        self.assertTrue(plan["fast_path"], plan)
        self.assertEqual(plan["reasons"], [])
        self.assertFalse(plan["swift"], plan)
        self.assertFalse(plan["data"], plan)

    def test_merge_base_keeps_pr_owned_trust_edits(self):
        branch_point = self.advanced_master()
        self.write("ci/pr-owned.py", "PR-owned\n")
        self.git("add", "ci/pr-owned.py")
        self.tree = self.git("write-tree")
        plan = self.cli_plan(self.attested_branch((branch_point,)))
        self.assertFalse(plan["fast_path"], plan)
        self.assertIn("trust-surface: ci/pr-owned.py", json.dumps(plan))
        self.assertNotIn("ci/master-only.py", json.dumps(plan))
        self.assertFalse(plan["jobs"]["build-project.yaml:rust-tests-linux"]["skip"])

    def test_merge_base_after_merging_master_keeps_only_pr_changes(self):
        branch_point = self.advanced_master()
        branch_head = self.attested_branch((branch_point,))
        self.git("read-tree", self.base)
        self.git("add", "libobs/value.c.h")
        self.tree = self.git("write-tree")
        plan = self.cli_plan(self.attested_branch((branch_head, self.base)))
        self.assertTrue(plan["fast_path"], plan)
        self.assertFalse(plan["swift"], plan)
        self.assertFalse(plan["data"], plan)

    def test_merge_base_missing_history_runs_all_checks(self):
        for history in ("unrelated", "shallow"):
            with self.subTest(history=history):
                head = self.complete_client()
                if history == "unrelated":
                    self.base = self.git("commit-tree", self.base_tree, input_text="unrelated base")
                    self.facts["data"]["repository"]["pullRequest"]["baseRefOid"] = self.base
                else:
                    (self.root / ".git/shallow").write_text(head + "\n")
                plan = self.cli_plan(head)
                self.assert_runs(plan, "commit/policy/evidence")
                self.assertFalse(plan["fast_path"], plan)

    def test_merge_base_git_error_runs_all_checks(self):
        head = self.complete_client()
        run = subject.Git.run

        def failing_diff(git, *args):
            if args[0] == "diff":
                raise subject.GitError("diff unavailable")
            return run(git, *args)

        with patch.object(subject.Git, "run", failing_diff):
            plan = self.cli_plan(head)
        self.assert_runs(plan, "diff unavailable")
        self.assertFalse(plan["fast_path"], plan)

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

    def test_fixture_setup_reuses_objects_without_sharing_mutable_state(self):
        other = type(self)("runTest")
        try:
            with patch.object(subprocess, "run", wraps=subprocess.run) as run:
                other.setUp()
            self.assertEqual(run.call_count, 0)
            other.write("src/value.txt", "isolated\n")
            other.facts.clear()
            other.proof_reports.clear()
            other.git("update-index", "--force-remove", "src/value.txt")
            (other.root / ".git/shallow").write_text(other.head + "\n")
            self.assertEqual((self.root / "src/value.txt").read_text(), "head\n")
            self.assertTrue(self.facts)
            self.assertTrue(self.proof_reports)
            self.assertIn("src/value.txt", self.objects.index)
            self.assertFalse((self.root / ".git/shallow").exists())
            self.assertTrue(self.evaluate()["trusted"])
        finally:
            other.doCleanups()

    def test_commit_metadata_uses_one_git_read(self):
        with patch.object(subprocess, "run", wraps=subprocess.run) as run:
            commit = REAL_GIT(self.root).commit(self.head)
        self.assertEqual(run.call_count, 1)
        self.assertEqual((commit.sha, commit.tree, commit.parents),
                         (self.head, self.tree, (self.base,)))

    def test_fixture_objects_match_git_plumbing(self):
        def real(*args, input_text=None):
            return subprocess.run(
                ["git", "-c", "core.autocrlf=false", "-c", "core.safecrlf=false",
                 "-c", f"core.attributesFile={os.devnull}", "-c", "commit.gpgSign=false",
                 "-C", str(self.root), *args], input=input_text,
                text=True, capture_output=True, check=True,
                env={**os.environ, "GIT_ATTR_NOSYSTEM": "1", "GIT_AUTHOR_NAME": "Local fixture",
                     "GIT_AUTHOR_EMAIL": "fixture@example.invalid",
                     "GIT_COMMITTER_NAME": "Local fixture",
                     "GIT_COMMITTER_EMAIL": "fixture@example.invalid",
                     "GIT_AUTHOR_DATE": "1794519000 +0000",
                     "GIT_COMMITTER_DATE": "1794519000 +0000"},
            ).stdout.strip()

        real("read-tree", self.tree)
        for path in ("sort.dir/file", "sort.file", "nested/deep/value"):
            self.write(path, "fixture\n")
            self.git("add", path)
            real("add", path)
        self.git("update-index", "--force-remove", "src/value.txt")
        real("update-index", "--force-remove", "src/value.txt")
        blob = self.git("hash-object", "-w", "--stdin", input_text="blob\n")
        self.assertEqual(blob, real("hash-object", "-w", "--stdin", input_text="blob\n"))
        self.git("update-index", "--add", "--cacheinfo", "100644", blob, "cached.txt")
        real("update-index", "--add", "--cacheinfo", "100644", blob, "cached.txt")
        tree = self.git("write-tree")
        self.assertEqual(tree, real("write-tree"))
        args = ("commit-tree", tree, "-p", self.base, "-p", self.head)
        commit = self.git(*args, input_text="merge\n\nmessage\n")
        self.assertEqual(commit, real(*args, input_text="merge\n\nmessage\n"))
        self.git("read-tree", commit)
        self.assertEqual(self.git("write-tree"), tree)
        real("fsck", "--strict", "--no-dangling", commit)

    def test_immutable_read_cache_never_hides_shallow_boundaries_or_missing_objects(self):
        reader = FixtureGit(self.root)
        self.assertEqual(reader.commit(self.head), REAL_GIT(self.root).commit(self.head))
        args = ("diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", f"{self.base}...{self.head}")
        self.assertEqual(reader.run(*args), REAL_GIT(self.root).run(*args))
        (self.root / ".git/shallow").write_text(self.head + "\n")
        with self.assertRaises(subject.GitError):
            reader.run(*args)
        with self.assertRaises(subject.GitError):
            reader.commit("0" * 40)

    def test_immutable_read_cache_never_hides_corrupt_objects(self):
        reader = FixtureGit(self.root)
        self.assertEqual(reader.commit(self.head), REAL_GIT(self.root).commit(self.head))
        path = self.root / ".git/objects" / self.head[:2] / self.head[2:]
        path.write_bytes(b"corrupt")
        with self.assertRaises(subject.GitError):
            reader.commit(self.head)
        other = type(self)("runTest")
        try:
            other.setUp()
            self.assertEqual(REAL_GIT(other.root).commit(other.head).sha, other.head)
        finally:
            other.doCleanups()


if __name__ == "__main__":
    unittest.main()
