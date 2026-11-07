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
$GITHUB_OUTPUT. Push, schedule and dispatch runs never consult proofs.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from datetime import datetime
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
    ".github/workflows/*",
    ".github/actions/*",
    "rust/tools/linux-validate/*",
    "rust-toolchain.toml",
)


def git(*args: str) -> str:
    return subprocess.run(["git", *args], cwd=ROOT, check=True, capture_output=True, text=True).stdout


def touches_surface(base: str, head: str) -> list[str]:
    # The base is an ancestor of the head (a PR merge commit or a queue group
    # commit), so a two-dot diff is the change and needs only both trees.
    changed = git("diff", "--name-only", base, head).split()
    return [p for p in changed if any(fnmatch.fnmatch(p, s) for s in SURFACES)]


def lane_key(lane_id: str, base: str, head: str = "HEAD", image: str | None = None) -> str:
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
        image = f"{os.environ.get('ImageOS', 'local')}/{os.environ.get('ImageVersion', '')}"
    header = {"v": PROOF_VERSION, "lane": lane_id, "run": list(lane.run), "image": image}
    h = hashlib.sha256(json.dumps(header, sort_keys=True).encode())
    for entry in inputs:
        h.update(b"\0" + entry.path.encode() + b"\0" + entry.sha.encode())
    return h.hexdigest()


def artifact_name(lane_id: str, key: str) -> str:
    return f"{PREFIX}{lane_id}-{key}"


def api(path: str, token: str) -> dict:
    req = urllib.request.Request(
        f"https://api.github.com{path}",
        headers={"Authorization": f"Bearer {token}", "Accept": "application/vnd.github+json",
                 "X-GitHub-Api-Version": "2022-11-28"},
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


def lookup(lane_id: str, key: str, repo: str, token: str, fetch=api, now: float | None = None) -> tuple[bool, str]:
    now = time.time() if now is None else now
    name = artifact_name(lane_id, key)
    try:
        listing = fetch(f"/repos/{repo}/actions/artifacts?name={name}&per_page=20", token)
    except (urllib.error.URLError, TimeoutError, ValueError) as exc:
        return False, f"artifact lookup failed: {exc}"
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
            return True, f"proof from run {run_id} ({run.get('event')}, {run.get('head_branch')})"
        reasons.append(why)
    return False, "; ".join(reasons) or "no proof for this key"


def output(**values: str) -> None:
    lines = "".join(f"{k}={v}\n" for k, v in values.items())
    print(lines, end="")
    path = os.environ.get("GITHUB_OUTPUT")
    if path:
        with open(path, "a", encoding="utf-8") as f:
            f.write(lines)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
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
        hit, reason = lookup(args.lane, args.key, args.repo, token)
        output(hit=str(hit).lower(), reason=reason)
        return 0

    surfaces = touches_surface(args.base, args.head)
    if surfaces:
        output(hit="false", key="", reason=f"changes proof surfaces: {', '.join(surfaces[:5])}")
        return 0
    try:
        key = lane_key(args.lane, args.base, args.head)
    except (LookupError, subprocess.CalledProcessError) as exc:
        output(hit="false", key="", reason=f"no key: {exc}")
        return 0
    if args.cmd == "key":
        output(key=key, name=artifact_name(args.lane, key))
        return 0
    hit, reason = lookup(args.lane, key, args.repo, token)
    output(hit=str(hit).lower(), key=key, name=artifact_name(args.lane, key), reason=reason)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
