#!/usr/bin/env -S uv run --script
# /// script
# dependencies = []
# ///
"""Base-owned PR trust decision API/schema v1; no workflow routing changes.

Run this file from the immutable base, through uv, with --base/--head full
commit SHAs and --number. Policy and the cached GraphQL wrapper are read from
that base's git objects. The loaded verifier must match the base as well.
Never import scripts or policy from the head to decide trust.

forge-ci/prooft 0.2.0 source pin:
683bcf0b3fcf77548054c85991f2b03fdaca77bd (used by the isolated subprocess adapter).
Uses its v1 Policy, Trailers, verify and lane_key APIs, not another trailer
format. This wrapper rejects malformed/duplicate/invalid supplied evidence
globally; omitted gates receive no credit. prooft's stricter 60-second future
clock tolerance is retained (and therefore rejects times beyond five minutes).
No legacy Local-Gate record supplies missing Ci-Attestation target gates.

API: evaluate(..., graphql=query_callback, rest=authenticated_get_callback,
now=unix_seconds, proofs=pinned_prooft_adapter) returns JSON-compatible schema_version, trusted, reasons,
gates and jobs (skip + per-job reasons). write_outputs emits trusted,
skip_<job-id> and reasons_<job-id>. Missing/unknown facts always run CI.
Production reads use GitHub GraphQL GitSignature.signer on the exact commit
and authenticated REST org membership state, including private memberships.
No permission expansion is attempted. Identity establishes accountability,
not proof that an organization member actually executed a claimed command.
"""

from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
from types import SimpleNamespace
import json
import math
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import time

PROOFT_REF = "683bcf0b3fcf77548054c85991f2b03fdaca77bd"
REPOSITORY = "obs-rust/obs-studio"
ORG = "obs-rust"
SHA = re.compile(r"[0-9a-f]{40}\Z")
LOGIN = re.compile(r"[A-Za-z0-9][A-Za-z0-9-]{0,38}\Z")
# Emit explicit real-run outputs even if base policy/objects cannot be read.
# Unmapped jobs cannot skip. Later routing may consume these outputs only.
JOBS = (
    "check-format.yaml:checks",
    "check-format.yaml:swift-format",
    "ci.yml:validate-data",
    *(
        f"build-project.yaml:{job}"
        for job in (
            "rust-tests-linux",
            "rust-windows-link",
            "cross-compile",
            "rust-cross-lint",
            "rust-cross-proof",
            "rust-tests-windows",
            "rust-tests-macos",
            "rust-tier2-windows",
            "rust-tier2-macos",
            "rust-linux-tier2-build",
            "rust-linux-tier2",
            "linux-qt-frontend",
        )
    ),
)
QUERY = """query($owner:String!,$name:String!,$number:Int!,$head:GitObjectID!) {
  repository(owner:$owner,name:$name) {
    nameWithOwner
    pullRequest(number:$number) {
      number baseRefOid headRefOid headRepository { nameWithOwner }
      author { login __typename }
    }
    object(oid:$head) { ... on Commit {
      oid signature { isValid state signer { login __typename } }
    } }
  }
}"""


PROOFT_SOURCE = f"prooft @ git+https://github.com/forge-ci/prooft@{PROOFT_REF}"
# Executed in an isolated interpreter supplied by the immutable source pin.
# All parsing/binding/freshness remains in upstream prooft. No head imports.
PROOFT_PROGRAM = r"""
import json, sys
from dataclasses import asdict
from pathlib import Path
from prooft.attestation import Trailers
from prooft.git import Git
from prooft.keys import lane_key
from prooft.miniyaml import parse
from prooft.policy import Policy
from prooft.verify import verify
request = json.load(sys.stdin)
raw = parse(request["text"])
if not isinstance(raw, dict) or type(raw.get("version")) is not int or raw["version"] != 1:
    raise ValueError("policy: unsupported version")
policy = Policy.parse(request["text"])
if request["operation"] == "policy":
    result = asdict(policy)
    result["lanes"] = policy.lanes
else:
    git = Git(Path(request["root"]))
    commit = git.commit(request["head"])
    keys = {lane: lane_key(git, request["head"], lane, patterns) for lane, patterns in policy.inputs.items()}
    verdict = verify(commit, policy, now=request["now"], keys=keys)
    trailers = Trailers.parse(commit.message)
    result = {
        "attestations": [{"gate": a.gate, "via": a.via} for a in trailers.attestations],
        "malformed": list(trailers.malformed),
        "statuses": [{"gate": s.gate, "state": str(s.state)} for s in verdict.statuses],
        "keys": keys,
    }
print(json.dumps(result))
"""


class GitError(RuntimeError):
    pass


@dataclass(frozen=True)
class Commit:
    sha: str
    tree: str
    parents: tuple[str, ...]
    message: str


@dataclass(frozen=True)
class Git:
    root: Path

    def run(self, *args):
        result = subprocess.run(
            ["git", "-C", str(self.root), *args],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode:
            raise GitError(f"commit lookup failed: {result.stderr.strip()}")
        return result.stdout

    def show(self, rev, path):
        return self.run("show", f"{rev}:{path}")

    def commit(self, rev):
        sha = self.run("rev-parse", "--verify", f"{rev}^{{commit}}").strip()
        return Commit(
            sha,
            self.run("rev-parse", f"{sha}^{{tree}}").strip(),
            tuple(self.run("log", "-1", "--format=%P", sha).split()),
            self.run("log", "-1", "--format=%B", sha),
        )


def prooft_api(request):
    # -I removes current-directory/PYTHONPATH imports. The subprocess cwd is
    # outside the head checkout and uv never reads project configuration.
    command = shlex.split(f"uv run --no-project --with '{PROOFT_SOURCE}' python -I -c")
    with tempfile.TemporaryDirectory(prefix="base-prooft-") as temp:
        result = subprocess.run(
            [*command, PROOFT_PROGRAM],
            input=json.dumps(request),
            cwd=temp,
            capture_output=True,
            text=True,
            timeout=120,
        )
    if result.returncode:
        raise ValueError("prooft: pinned API unavailable or rejected policy/evidence")
    return json.loads(result.stdout)


def trust_surface(path):
    parts = path.split("/")
    return (
        path
        in {"push", "local-gate.toml", "ci-attestations.yml", "rust-toolchain.toml"}
        or parts[0] in {"ci", ".github"}
        or ".cargo" in parts
        or parts[-1] in {"Cargo.toml", "Cargo.lock"}
    )


def base_policy(git, base, proofs):
    text = git.show(base, "ci-attestations.yml")
    report = proofs({"operation": "policy", "text": text})
    policy = SimpleNamespace(**report, text=text)
    if not policy.gates or not policy.jobs:
        raise ValueError("policy: empty gate/job declarations")
    for lane in policy.lanes:
        if policy.max_age_hours.get(lane) != 24:
            raise ValueError(
                f"policy: {lane} must explicitly declare 24-hour freshness"
            )
    ids = [job.split(":", 1)[1] for job in policy.jobs]
    if len(ids) != len(set(ids)) or any(
        not re.fullmatch(r"[A-Za-z0-9_-]+", job) for job in ids
    ):
        raise ValueError("policy: ambiguous or invalid output job ids")
    return policy


def base_objects(git, base, head):
    if not SHA.fullmatch(base) or not SHA.fullmatch(head):
        raise ValueError("commit: immutable full SHAs required")
    git.commit(base)
    commit = git.commit(head)
    if git.show(base, "ci/pr_attestation.py") != Path(__file__).read_text(
        encoding="utf-8"
    ):
        raise ValueError("verifier: loaded code does not match immutable base")
    return commit


def reject_surfaces(git, base, head):
    # --no-renames exposes both removed and added names; -z handles odd paths.
    paths = git.run(
        "diff", "--no-ext-diff", "--no-renames", "--name-only", "-z", base, head
    ).split("\0")
    touched = sorted(path for path in paths if path and trust_surface(path))
    if touched:
        raise ValueError("trust-surface: " + ", ".join(touched))


def member(login, rest):
    try:
        response = rest(f"orgs/{ORG}/memberships/{login}")
        if (
            response.get("state") != "active"
            or response.get("user", {}).get("login", "").lower() != login.lower()
        ):
            return f"membership: {login} is not a verified active {ORG} member"
    except (
        OSError,
        RuntimeError,
        ValueError,
        TypeError,
        AttributeError,
        subprocess.SubprocessError,
    ):
        return f"membership: lookup unavailable or denied for {login}"
    return None


def authenticated_identity(data, *, base, head, number):
    if data.get("errors"):
        raise ValueError("identity: GitHub GraphQL lookup failed")
    repo = data["data"]["repository"]
    pr = repo["pullRequest"]
    commit = repo["object"]
    if (
        repo["nameWithOwner"] != REPOSITORY
        or pr["number"] != number
        or pr["baseRefOid"] != base
        or pr["headRefOid"] != head
        or commit["oid"] != head
    ):
        raise ValueError("identity: GitHub PR/base/head does not match exact inputs")
    if (pr.get("headRepository") or {}).get("nameWithOwner") != REPOSITORY:
        raise ValueError("same-repository: fork or unknown head repository")
    signature = commit.get("signature") or {}
    signer = signature.get("signer") or {}
    if (
        signature.get("isValid") is not True
        or signature.get("state") != "VALID"
        or not signer.get("login")
    ):
        raise ValueError("signature: exact HEAD has no valid authenticated signer")
    author = pr.get("author") or {}
    identities = (author, signer)
    if any(
        identity.get("__typename") != "User"
        or not LOGIN.fullmatch(identity.get("login", ""))
        for identity in identities
    ):
        raise ValueError("identity: author/signer is absent or not a verified user")
    return tuple(dict.fromkeys(identity["login"] for identity in identities))


def identity_reasons(graphql, rest, *, base, head, number):
    try:
        if type(number) is not int or number <= 0:
            raise ValueError("identity: positive PR number required")
        data = graphql(
            QUERY, {"owner": ORG, "name": "obs-studio", "number": number, "head": head}
        )
        logins = authenticated_identity(data, base=base, head=head, number=number)
    except (
        KeyError,
        OSError,
        RuntimeError,
        ValueError,
        TypeError,
        AttributeError,
        subprocess.SubprocessError,
    ) as exc:
        return [
            f"identity: lookup unavailable or invalid ({type(exc).__name__}): {exc}"
        ]
    return [reason for login in logins if (reason := member(login, rest))]


def evidence(commit, policy, git, head, now, proofs):
    if isinstance(now, bool) or not math.isfinite(now) or now <= 0:
        raise ValueError("evidence: invalid verification time")
    report = proofs(
        {
            "operation": "evidence",
            "text": policy.text,
            "root": str(git.root.resolve()),
            "head": head,
            "now": now,
        }
    )
    records = report["attestations"]
    count = sum(
        line.lstrip().lower().startswith("ci-attestation:")
        for line in commit.message.splitlines()
    )
    if report["malformed"] or count != len(records):
        raise ValueError("evidence: malformed Ci-Attestation trailer")
    if any(att["via"] not in {"run", "reused"} for att in records):
        raise ValueError("evidence: malformed execution provenance")
    duplicates = sorted(
        gate for gate, count in Counter(a["gate"] for a in records).items() if count > 1
    )
    if duplicates:
        raise ValueError("evidence: duplicate gates: " + ", ".join(duplicates))
    statuses = report["statuses"]
    errors = [
        f"{status['gate']}={status['state']}"
        for status in statuses
        if status["state"] not in {"valid", "missing"}
    ]
    if errors:
        raise ValueError("evidence: " + "; ".join(errors))
    return {status["gate"]: status["state"] for status in statuses}


def decision(policy, reasons, gates):
    jobs = {}
    declared = policy.jobs if policy else {}
    for job in sorted(set(JOBS) | set(declared)):
        required = declared.get(job, ())
        lacking = [
            f"{gate}={gates.get(gate, 'missing')}"
            for gate in required
            if gates.get(gate) != "valid"
        ]
        job_reasons = list(reasons) or lacking
        if not required:
            job_reasons.append("no declared client gates; run selected CI")
        jobs[job] = {
            "skip": not job_reasons,
            "reasons": job_reasons or ["all declared gates valid"],
        }
    return {
        "schema_version": 1,
        "trusted": not reasons,
        "reasons": reasons,
        "gates": gates,
        "jobs": jobs,
    }


def evaluate(root, *, base, head, number, event, graphql, rest, now, proofs=prooft_api):
    """Pure decision boundary over local git objects and injected authenticated reads."""
    policy = None
    gates = {}
    git = Git(Path(root))
    try:
        commit = base_objects(git, base, head)
        policy = base_policy(git, base, proofs)
        if event != "pull_request":
            return decision(policy, [f"event: {event!r} cannot skip CI"], gates)
        reject_surfaces(git, base, head)
        reasons = identity_reasons(graphql, rest, base=base, head=head, number=number)
        if not reasons:
            gates = evidence(commit, policy, git, head, now, proofs)
        return decision(policy, reasons, gates)
    except (
        GitError,
        OSError,
        ValueError,
        TypeError,
        OverflowError,
        IndexError,
        KeyError,
        AttributeError,
        subprocess.SubprocessError,
    ) as exc:
        return decision(policy, [f"commit/policy/evidence: {exc}"], gates)


def write_outputs(result, output):
    output.write(f"trusted={str(result['trusted']).lower()}\n")
    for job, status in result["jobs"].items():
        job_id = job.split(":", 1)[1]
        output.write(f"skip_{job_id}={str(status['skip']).lower()}\n")
        output.write(
            f"reasons_{job_id}={json.dumps(status['reasons'], separators=(',', ':'))}\n"
        )


def github_reads(root, base):
    """Execute only the cached GraphQL wrapper from BASE; no head imports."""

    def graphql(query, variables):
        wrapper = Git(root).show(base, "ci/gh_graphql.py")
        if wrapper is None:
            raise ValueError("missing base GraphQL wrapper")
        with tempfile.TemporaryDirectory(prefix="base-graphql-") as temp:
            path = Path(temp) / "gh_graphql.py"
            path.write_text(wrapper, encoding="utf-8")
            args = [
                "uv",
                "run",
                "--no-project",
                "--script",
                str(path),
                "-f",
                "query=" + query,
            ]
            for key, value in variables.items():
                args.extend(["-F" if type(value) is int else "-f", f"{key}={value}"])
            return json.loads(
                subprocess.run(
                    args, capture_output=True, text=True, check=True, timeout=120
                ).stdout
            )

    def rest(path):
        return json.loads(
            subprocess.run(
                ["gh", "api", path],
                capture_output=True,
                text=True,
                check=True,
                timeout=60,
            ).stdout
        )

    return graphql, rest


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--head", required=True)
    parser.add_argument("--number", required=True, type=int)
    parser.add_argument("--event", default=os.environ.get("GITHUB_EVENT_NAME", ""))
    parser.add_argument("--repo-dir", type=Path, default=Path.cwd())
    args = parser.parse_args(argv)
    graphql, rest = github_reads(args.repo_dir, args.base)
    result = evaluate(
        args.repo_dir,
        base=args.base,
        head=args.head,
        number=args.number,
        event=args.event,
        graphql=graphql,
        rest=rest,
        now=time.time(),
    )
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a", encoding="utf-8") as stream:
            write_outputs(result, stream)
    return 0


if __name__ == "__main__":
    sys.exit(main())
