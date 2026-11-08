#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["ci-lint @ git+https://github.com/zackees/ci.yml@a1917ce76598904ce515cd7eec83450c317ea6f2"]
# ///
"""Lane proofs: skip a CI lane whose exact inputs already passed (#699).

    uv run ci/lane_proof.py key    --lane LANE --base REV [--head REV]
    uv run ci/lane_proof.py lookup --lane LANE --key KEY --repo OWNER/NAME
    uv run ci/lane_proof.py check  --lane LANE --base REV --repo OWNER/NAME

A merge-queue commit always has a new tree, so the tree-bound local-gate
attestation (GATE-008) can never cover it, and every queue rebuild reran
every lane. A proof is keyed by the lane's *inputs* instead:

- **Key** = sha256 over a versioned header (proof version, lane argv, the
  runner image) and `(path, blob)` for every input of the lane, where the
  input set is the lane's `exclude`/`keep` declaration in `local-gate.toml`
  (zackees/ci.yml GATE-007: exclusion-based, audited with strace). The
  declaration is read from the BASE commit, never the head, so a change
  cannot widen its own exclusions to collide with an older proof. If the
  head changes a proof surface (SURFACES), there is no key at all.
- **Ledger** = this repository's Actions artifacts. A job that ran the lane
  and passed uploads an empty artifact named `lane-proof-v1-<lane>-<key>`
  as its last step, so the name exists only after a pass.
- **Trust**: a proof counts only if the run that uploaded it is this
  repository's ci.yml workflow on a branch in this repository (only
  people with write access can push one) or a merge-queue run, it is not
  expired, and it is younger than MAX_AGE_HOURS. Fork PR runs execute the
  fork's workflow file, so their proofs are never trusted; fork PRs still
  reuse proofs other runs published.

Everything fails closed: no key, an API error or no trusted proof means the
lane runs. `check` writes `hit=true|false`, `key` and `reason` to
$GITHUB_OUTPUT. Every check appends its decision and trusted proof run to
$GITHUB_STEP_SUMMARY when set; one in ten keys reruns as an audit sample.
Push, schedule and dispatch runs never consult proofs.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

# LANE_PROOF_REPO: another checkout to key (ci/test_lane_proof.py uses a
# scratch repository).
ROOT = Path(os.environ.get("LANE_PROOF_REPO") or Path(__file__).resolve().parent.parent)
PROOF_VERSION = 1
PREFIX = f"lane-proof-v{PROOF_VERSION}-"
MAX_AGE_HOURS = 72
TRUSTED_WORKFLOW = ".github/workflows/ci.yml"
# A change to any of these decides how lanes run or how proofs are keyed,
# so a head that touches one gets no key and runs everything.
SURFACES = (
    "local-gate.toml",
    "ci/lane_proof.py",
    "ci/local_gate.py",
    "ci/nextest_tool.py",
    "ci/nextest.toml",
    "ci/tier2_matrix.py",
    ".github/workflows/*",
    ".github/actions/*",
    "rust/tools/linux-validate/*",
    "rust-toolchain.toml",
)


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout


def touches_surface(base: str, head: str) -> list[str]:
    # The base is an ancestor of the head (a PR merge commit or a queue group
    # commit), so a two-dot diff is the change and needs only both trees.
    changed = git("diff", "--name-only", base, head).split()
    return [p for p in changed if any(fnmatch.fnmatch(p, s) for s in SURFACES)]


def lane_key(
    lane_id: str, base: str, head: str = "HEAD", image: str | None = None
) -> str:
    """The proof key of `lane_id` for `head`, with the lane declared as on `base`."""
    from ci_lint.lane_cache import lane_inputs, tree_entries
    from ci_lint.local_gate import load_gate_config_at

    config = load_gate_config_at(ROOT, base).config
    if config is None:
        raise LookupError(f"no gate declaration at {base}")
    lane = next((ln for ln in config.lanes if ln.id == lane_id), None)
    if lane is None:
        raise LookupError(f"lane {lane_id!r} is not declared at {base}")
    entries = tree_entries(ROOT, head)
    inputs = lane_inputs(entries, lane, config.run, config.source)
    if image is None:
        image = (
            f"{os.environ.get('ImageOS', 'local')}/{os.environ.get('ImageVersion', '')}"
        )
    header = {
        "v": PROOF_VERSION,
        "lane": lane_id,
        "run": list(lane.run),
        "image": image,
    }
    h = hashlib.sha256(json.dumps(header, sort_keys=True).encode())
    for entry in inputs:
        h.update(b"\0" + entry.path.encode() + b"\0" + entry.sha.encode())
    return h.hexdigest()


def artifact_name(lane_id: str, key: str) -> str:
    return f"{PREFIX}{lane_id}-{key}"


def api(path: str, token: str) -> dict:
    req = urllib.request.Request(
        f"https://api.github.com{path}",
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        return json.load(resp)


def trusted(run: dict, repo: str, now: float) -> str | None:
    """None when `run` may vouch for a proof, else the reason it may not."""
    if run.get("path") != TRUSTED_WORKFLOW:
        return f"run {run.get('id')} is {run.get('path')!r}, not {TRUSTED_WORKFLOW}"
    event = run.get("event")
    if event not in ("pull_request", "merge_group"):
        return f"run {run.get('id')} event {event!r}"
    head_repo = (run.get("head_repository") or {}).get("full_name")
    if head_repo != repo:
        return f"run {run.get('id')} head is {head_repo!r} (fork)"
    created = run.get("created_at") or ""
    try:
        age = now - datetime.fromisoformat(created.replace("Z", "+00:00")).timestamp()
    except ValueError:
        return f"run {run.get('id')} has no creation time"
    if age > MAX_AGE_HOURS * 3600:
        return f"run {run.get('id')} is {age / 3600:.0f} h old"
    return None


def lookup(
    lane_id: str, key: str, repo: str, token: str, fetch=api, now: float | None = None
) -> tuple[bool, str, int | None]:
    now = time.time() if now is None else now
    name = artifact_name(lane_id, key)
    try:
        listing = fetch(
            f"/repos/{repo}/actions/artifacts?name={name}&per_page=20", token
        )
    except (urllib.error.URLError, TimeoutError, ValueError) as exc:
        return False, f"artifact lookup failed: {exc}", None
    reasons = []
    for art in listing.get("artifacts", []):
        if art.get("expired") or art.get("name") != name:
            continue
        run_id = (art.get("workflow_run") or {}).get("id")
        if not run_id:
            continue
        try:
            run = fetch(f"/repos/{repo}/actions/runs/{run_id}", token)
        except (urllib.error.URLError, TimeoutError, ValueError) as exc:
            reasons.append(f"run {run_id}: {exc}")
            continue
        why = trusted(run, repo, now)
        if why is None:
            return (
                True,
                f"proof from run {run_id} ({run.get('event')}, {run.get('head_branch')})",
                run_id,
            )
        reasons.append(why)
    return False, "; ".join(reasons) or "no proof for this key", None


def miss_diagnostic(
    lane_id: str,
    key: str,
    base: str,
    head: str,
    repo: str,
    token: str,
    fetch=api,
    now: float | None = None,
) -> str:
    """Explain a queue miss with at most two bounded requests; never decide a hit.

    Partial history cannot establish absence. An existing different key takes
    precedence over an unchanged base SHA: the key describes the actual inputs.
    """
    nearest, run_id, pr_base, verdict = "none", "none", "none", "unknown"
    try:
        ref = " ".join(
            (
                head,
                os.environ.get("GITHUB_HEAD_REF", ""),
                os.environ.get("GITHUB_REF", ""),
            )
        )
        match = re.search(r"(?:^|/)pr-(\d+)(?:-|/|$)", ref)
        if not token or match is None:
            raise ValueError("no credentials or queue PR")
        number = int(match[1])
        now = time.time() if now is None else now
        since = datetime.fromtimestamp(
            now - MAX_AGE_HOURS * 3600, timezone.utc
        ).strftime("%Y-%m-%dT%H:%M:%SZ")
        runs = fetch(
            f"/repos/{repo}/actions/workflows/ci.yml/runs?event=pull_request&per_page=100&created=%3E%3D{since}",
            token,
        )
        candidates = {
            run["id"]: (run, pr["base"]["sha"])
            for run in runs["workflow_runs"]
            if trusted(run, repo, now) is None
            for pr in run["pull_requests"]
            if pr["number"] == number and pr["head"]["ref"] == run["head_branch"]
        }
        listing = fetch(f"/repos/{repo}/actions/artifacts?per_page=100", token)
        prefix = artifact_name(lane_id, "")
        proofs = []
        for art in listing["artifacts"]:
            name = art["name"]
            if (
                not art.get("expired")
                and name.startswith(prefix)
                and re.fullmatch(r"[0-9a-f]{64}", name[len(prefix) :])
                and art["workflow_run"]["id"] in candidates
            ):
                proofs.append(art)
        if proofs:
            art = max(proofs, key=lambda a: (a["created_at"], a.get("id", 0)))
            proof_key = art["name"][len(prefix) :]
            nearest = proof_key[:12]
            run_id = str(art["workflow_run"]["id"])
            pr_base = candidates[int(run_id)][1]
            # A matching key with no usable proof is not evidence of stale inputs.
            verdict = "stale-base" if proof_key != key else "unknown"
            # The newest artifact must be in the returned run window as well.
            if runs["total_count"] > len(runs["workflow_runs"]):
                verdict = "unknown"
        elif runs["total_count"] <= len(runs["workflow_runs"]) and listing[
            "total_count"
        ] <= len(listing["artifacts"]):
            verdict = "no-pr-proof"
    except Exception:
        verdict = "unknown"
    return f"nearest={nearest} run={run_id} pr_base={pr_base} queue_base={base} verdict={verdict}"


def summarize_miss(
    lane: str,
    key: str,
    base: str,
    head: str,
    repo: str,
    token: str,
    decision: tuple[str, bool, str],
) -> None:
    if (*decision, os.environ.get("GITHUB_EVENT_NAME")) != (
        "check",
        False,
        "no proof for this key",
        "merge_group",
    ):
        return
    diagnostic = f"nearest=none run=none pr_base=none queue_base={base} verdict=unknown"
    try:
        diagnostic = miss_diagnostic(lane, key, base, head, repo, token)
    except Exception:
        pass  # Even an unexpected diagnostic failure leaves all outputs untouched.
    try:
        summary = os.environ.get("GITHUB_STEP_SUMMARY")
        if summary:
            with open(summary, "a", encoding="utf-8") as f:
                f.write(" ".join(diagnostic.splitlines()) + "\n")
    except Exception:
        pass


def output(
    *, summary_lane: str | None = None, workflow_run: int | None = None, **values: str
) -> None:
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_lane is not None and summary:
        # Keep each decision on one line even if an API/git error spans lines.
        fields = {
            "lane": summary_lane,
            "key": values.get("key") or "none",
            "hit": values["hit"],
            "reason": values["reason"],
            "workflow_run": str(workflow_run) if workflow_run is not None else "none",
            "artifact": values.get("name") or "none",
        }
        line = "- " + " ".join(
            f"{k}={' '.join(v.splitlines())}" for k, v in fields.items()
        )
        if values["reason"] == "audit sample":
            line += "; audit failure proves a wrong skip"
        try:
            with open(summary, "a", encoding="utf-8") as f:
                f.write(line + "\n")
        except OSError:
            pass  # Observability must not break the lane decision.
    lines = "".join(f"{k}={v}\n" for k, v in values.items())
    print(lines, end="")
    path = os.environ.get("GITHUB_OUTPUT")
    if path:
        with open(path, "a", encoding="utf-8") as f:
            f.write(lines)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="cmd", required=True)
    for name in ("key", "check"):
        p = sub.add_parser(name)
        p.add_argument("--lane", required=True)
        p.add_argument("--base", required=True)
        p.add_argument("--head", default="HEAD")
        if name == "check":
            p.add_argument("--repo", required=True)
    p = sub.add_parser("lookup")
    p.add_argument("--lane", required=True)
    p.add_argument("--key", required=True)
    p.add_argument("--repo", required=True)
    args = parser.parse_args(argv)
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN", "")

    if args.cmd == "lookup":
        hit, reason, _ = lookup(args.lane, args.key, args.repo, token)
        output(hit=str(hit).lower(), reason=reason)
        return 0

    key = ""
    hit = False
    workflow_run = None
    try:
        surfaces = touches_surface(args.base, args.head)
        if surfaces:
            reason = f"changes proof surfaces: {', '.join(surfaces[:5])}"
        else:
            key = lane_key(args.lane, args.base, args.head)
            if args.cmd == "key":
                output(key=key, name=artifact_name(args.lane, key))
                return 0
            hit, reason, workflow_run = lookup(args.lane, key, args.repo, token)
            if hit and int(key[:8], 16) % 10 == 0:
                hit, reason = False, "audit sample"
    except Exception as exc:
        # Malformed ledger data and unexpected lookup failures also run the lane.
        if args.cmd != "check" and not isinstance(
            exc, (LookupError, subprocess.CalledProcessError)
        ):
            raise
        hit, workflow_run = False, None
        reason = f"proof check failed: {exc}" if key else f"no key: {exc}"
    values = {"hit": str(hit).lower(), "key": key, "reason": reason}
    if key:
        values["name"] = artifact_name(args.lane, key)
    output(
        summary_lane=args.lane if args.cmd == "check" else None,
        workflow_run=workflow_run,
        **values,
    )
    summarize_miss(
        args.lane,
        key,
        args.base,
        args.head,
        getattr(args, "repo", ""),
        token,
        (args.cmd, hit, reason),
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
