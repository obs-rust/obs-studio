#!/usr/bin/env -S uv run --script
"""Roll cache ledgers across CI runs into one table per lane and event.

    uv run --no-project ci/cache_baseline.py --ledger-dir DIR      # records on disk
    uv run --no-project ci/cache_baseline.py --collect             # 20 merge-group + 20 master

--collect downloads `cache-ledger-*` artifacts. A run from before the ledger has
none, so it falls back to that run's job logs (--no-backfill turns that off):
those give zccache counters and setup-soldr layers, but no miss reasons. The
table prints the number of runs each row really rests on; n is never assumed.
"""

from __future__ import annotations

import argparse
import json
import re
import statistics
import subprocess
import sys
import tempfile
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
from pathlib import Path

import cache_ledger

REPO = "obs-rust/obs-studio"
TIMEOUT = 300
# Lanes that print compiler-cache evidence in their logs; others are timed only.
LOG_LANES = re.compile(r"Fast Gate|Tier 2 Linux .*\(build\)|Rust Tests|Rust Tier 2 \(|Ubuntu|macOS|Windows")


def load_records(directory: Path) -> tuple[list[dict], int]:
    """Valid records and the number of skipped files; bad files never stop a rollup."""
    records, skipped = [], 0
    for path in sorted(directory.rglob("*.json")):
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            skipped += 1
            continue
        if isinstance(data, dict) and data.get("lane"):
            records.append(data)
        else:
            skipped += 1
    return records, skipped


def _int(value: object) -> int:
    return value if type(value) is int and value >= 0 else 0


def rollup(records: list[dict]) -> list[dict]:
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for record in records:
        groups[(str(record.get("lane")), str(record.get("event") or "?"))].append(record)
    rows = []
    for (lane, event), members in sorted(groups.items()):
        hits = sum(_int(m.get("hits")) for m in members)
        misses = sum(_int(m.get("misses")) for m in members)
        changed = sum(_int(m.get("misses_from_changed_inputs")) for m in members)
        walls = [m["wall_seconds"] for m in members if isinstance(m.get("wall_seconds"), (int, float))]
        reasons: dict[str, int] = defaultdict(int)
        for member in members:
            for reason, count in (member.get("miss_reasons") or {}).items():
                reasons[reason] += _int(count)
        rows.append({
            "lane": lane, "event": event, "n": len({m.get("run_id") or id(m) for m in members}),
            "jobs": len(members), "hits": hits, "misses": misses, "misses_from_changed_inputs": changed,
            "rate": cache_ledger.eligible_hit_rate(hits, misses, changed),
            "bypassed": any(m.get("wrapper_state") == "bypassed" for m in members),
            "measured": any(m.get("counter_source") not in (None, "none", "timing-only") for m in members),
            "changed_known": bool(members) and all(m.get("changed_inputs_known") for m in members),
            "wall": statistics.median(walls) if walls else None, "reasons": dict(reasons),
        })
    return rows


def rate_cell(row: dict) -> str:
    if row["rate"] is not None:
        scope = " non-Rust" if row["bypassed"] else ""
        bound = "" if row["changed_known"] else " (≥)"
        return f"{row['rate'] * 100:.1f}%{scope}{bound}"
    if row["bypassed"]:
        return "n/a (bypassed)"
    return "not measured" if not row["measured"] else "n/a (no compilations)"


def table(rows: list[dict]) -> str:
    out = ["| lane | event | runs (n) | eligible hit rate | hits / misses | median wall | Rust wrapper |",
           "|---|---|---:|---:|---:|---:|---|"]
    for row in rows:
        wall = f"{row['wall'] / 60:.1f} min" if row["wall"] is not None else "?"
        counts = f"{row['hits']} / {row['misses']}" if row["hits"] or row["misses"] else "-"
        marker = "bypassed" if row["bypassed"] else "active" if row["measured"] else "?"
        out.append(f"| {row['lane']} | {row['event']} | {row['n']} | {rate_cell(row)} | {counts} | {wall} | {marker} |")
    return "\n".join(out)


def record_from_log(job: dict, run: dict, log: str | None) -> dict:
    meta = {"job": job.get("name"), "lane": normalize_lane(str(job.get("name"))), "sha": run.get("headSha"),
            "event": run.get("event"), "run_id": run.get("databaseId"), "os": None, "arch": None}
    text = log or ""
    counters = cache_ledger.counters_from_log(text)
    wrapper = cache_ledger.wrapper_from_log(text)
    source = "job-log" if counters else ("job-log-no-counters" if text else "timing-only")
    record = cache_ledger.build_record(
        meta=meta, wrapper=wrapper, counters=counters, source=source, versions={},
        layers=cache_ledger.layers_from_log(text), changed_known=False,
    )
    record["wall_seconds"] = job_seconds(job)
    return record


def normalize_lane(name: str) -> str:
    return name.split(" / ", 1)[-1]


def job_seconds(job: dict) -> float | None:
    try:
        start = datetime.fromisoformat(job["startedAt"].replace("Z", "+00:00"))
        end = datetime.fromisoformat(job["completedAt"].replace("Z", "+00:00"))
    except (KeyError, AttributeError, ValueError):
        return None
    return (end - start).total_seconds() if end >= start else None


def gh(*args: str) -> str | None:
    try:
        return subprocess.run(["gh", *args], capture_output=True, check=True, encoding="utf-8",
                              errors="replace", timeout=TIMEOUT).stdout
    except (OSError, subprocess.SubprocessError):
        return None


def list_runs(event: str, limit: int, branch: str | None) -> list[dict]:
    args = ["run", "list", "--repo", REPO, "--event", event, "--status", "success", "--limit", str(limit),
            "--workflow", "Pull" if event == "merge_group" else "Push",
            "--json", "databaseId,headSha,event"]
    if branch:
        args += ["--branch", branch]
    try:
        return json.loads(gh(*args) or "[]")
    except ValueError:
        return []


def collect_run(run: dict, backfill: bool) -> list[dict]:
    run_id = str(run["databaseId"])
    with tempfile.TemporaryDirectory() as tmp:
        if gh("run", "download", run_id, "--repo", REPO, "--pattern", "cache-ledger-*", "--dir", tmp) is not None:
            records, _ = load_records(Path(tmp))
            if records:
                return records
    if not backfill:
        return []
    raw = gh("run", "view", run_id, "--repo", REPO, "--json", "jobs")
    try:
        jobs = json.loads(raw or "{}").get("jobs", [])
    except (ValueError, AttributeError):
        return []
    records = []
    for job in jobs:
        if job.get("conclusion") != "success":
            continue
        name = str(job.get("name"))
        log = gh("run", "view", "--repo", REPO, "--job", str(job.get("databaseId")), "--log") if LOG_LANES.search(name) else None
        records.append(record_from_log(job, run, log))
    return records


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ledger-dir", type=Path)
    parser.add_argument("--collect", action="store_true")
    parser.add_argument("--merge-group", type=int, default=20)
    parser.add_argument("--master", type=int, default=20)
    parser.add_argument("--no-backfill", action="store_true")
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--dump", type=Path, help="write every record here as JSON")
    args = parser.parse_args(argv)
    records: list[dict] = []
    skipped = 0
    requested = args.merge_group + args.master if args.collect else 0
    if args.ledger_dir:
        loaded, skipped = load_records(args.ledger_dir)
        records += loaded
    runs: list[dict] = []
    if args.collect:
        runs = list_runs("merge_group", args.merge_group, None) + list_runs("push", args.master, "master")
        with ThreadPoolExecutor(max_workers=max(1, args.workers)) as pool:
            for result in pool.map(lambda r: collect_run(r, not args.no_backfill), runs):
                records += result
    if args.dump:
        args.dump.write_text(json.dumps(records, indent=1, sort_keys=True), encoding="utf-8")
    print(table(rollup(records)))
    print(f"\n{len(records)} job records; {skipped} unreadable files skipped"
          + (f"; {len(runs)} of {requested} requested runs found" if args.collect else ""))
    print("Rates are pooled hits over eligible misses. (≥): changed paths unknown, so no miss was excluded.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
