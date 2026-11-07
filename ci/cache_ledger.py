#!/usr/bin/env -S uv run --script
"""Turn one CI job's cache data into one JSON record; never fail the job.

Sources, best first: compile journals under the zccache directory (deduplicated
across history directories, filtered to this job with --since), then the
`soldr cache report` session (soldr_cache_report.py), then a job log's
`zccache status` lines (backfill for runs from before this ledger).

SOLDR_RUSTC_WRAPPER=none is a statement about Rust only. A bypassed job with no
compilations is n/a, never 0%; one with compilations (Tier 2's zccache launcher)
reports a rate scoped to non-Rust compilers.

eligible_hit_rate = hits / (hits + misses - misses_from_changed_inputs). A miss is
attributed to a changed input when a source named on its command line, or a path
in its miss_diff, is in the changed paths. Headers a compile reaches transitively
are not on the command line, so the rate is a lower bound.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request
from collections import Counter
from datetime import UTC, datetime
from pathlib import Path

from native_cache_journal import sources
from soldr_cache_report import read_report

SCHEMA = 1
MAX_JOURNALS = 20000
MAX_PHASE_SUMMARY_BYTES = 16384
HIT_OUTCOMES = {"hit"}
NON_CACHEABLE = {"non-cacheable", "non_cacheable"}
STATUS_LINE = re.compile(r"Compilations:\s+(\d+) total \((\d+) cached, (\d+) cold, (\d+) non-cacheable\)")
LAYER_LINE = re.compile(
    r"(?:^|\s)([a-z][a-z0-9-]*)\s+(HIT|MISS)\s+(\S.*?)\s+(\S+)\s+(\S+)\s+(\S+)\s+([\d.]+)s\s*$", re.M
)
WRAPPER_LINE = re.compile(r"SOLDR_RUSTC_WRAPPER: (\S+)")
ANSI = re.compile(r"\x1b\[[0-9;]*m")
SIZE_UNITS = {"B": 1, "KB": 1024, "MB": 1024**2, "GB": 1024**3, "TB": 1024**4}


def parse_ts(value: object) -> datetime | None:
    if not isinstance(value, str):
        return None
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    return parsed if parsed.tzinfo else parsed.replace(tzinfo=UTC)


def dedupe_key(row: dict) -> tuple:
    """The same compile is copied into several history directories."""
    return tuple(json.dumps(row.get(k), sort_keys=True) for k in ("ts", "context_key", "latency_ns", "cwd", "session_id"))


def read_journals(cache_dir: Path | None, since: datetime | None = None) -> tuple[list[dict], int, int]:
    """Return (unique rows, raw row count, malformed lines). Unreadable files are skipped."""
    if cache_dir is None:
        return [], 0, 0
    unique: dict[tuple, dict] = {}
    raw = malformed = 0
    try:
        paths = sorted(cache_dir.glob("history/*/compile_journal.jsonl"))[:MAX_JOURNALS]
    except OSError:
        return [], 0, 0
    for path in paths:
        try:
            lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            continue
        for line in lines:
            if not line.strip():
                continue
            try:
                row = json.loads(line)
            except ValueError:
                malformed += 1
                continue
            if not isinstance(row, dict):
                malformed += 1
                continue
            if since is not None:
                ts = parse_ts(row.get("ts"))
                # Tier 2 restores earlier runs' history; undated rows cannot be claimed.
                if ts is None or ts < since:
                    continue
            raw += 1
            unique.setdefault(dedupe_key(row), row)
    return list(unique.values()), raw, malformed


def row_paths(row: dict, root: Path) -> set[str]:
    cwd = row.get("cwd") if isinstance(row.get("cwd"), str) else str(root)
    args = row.get("args") if isinstance(row.get("args"), list) else []
    # sources() resolves against its workspace, but a compile's paths are relative to its cwd.
    absolute = [
        str(Path(cwd) / a) if isinstance(a, str) and a and not a.startswith("-") else a for a in args
    ]
    found = set(sources(absolute, root))
    diff = row.get("miss_diff")
    changed = diff.get("changed_inputs") if isinstance(diff, dict) else None
    for item in changed if isinstance(changed, list) else []:
        if isinstance(item, str):
            try:
                found.add((root / item).resolve().relative_to(root).as_posix())
            except (ValueError, OSError):
                continue
    return found


def count_rows(rows: list[dict], changed: set[str] | None, root: Path) -> dict:
    outcomes: Counter = Counter()
    reasons: Counter = Counter()
    from_changed = 0
    resolved = root.resolve()
    for row in rows:
        outcome = row.get("outcome")
        if outcome in HIT_OUTCOMES:
            outcomes["hits"] += 1
        elif outcome == "miss":
            outcomes["misses"] += 1
            reason = row.get("miss_reason")
            reasons[reason if isinstance(reason, str) and reason else "unclassified"] += 1
            if changed and row_paths(row, resolved) & changed:
                from_changed += 1
        elif outcome in NON_CACHEABLE:
            outcomes["non_cacheable"] += 1
        elif outcome == "error":
            outcomes["errors"] += 1
        else:
            outcomes["unclassified"] += 1
    return {
        "compilations": sum(outcomes.values()),
        "hits": outcomes["hits"],
        "misses": outcomes["misses"],
        "non_cacheable": outcomes["non_cacheable"],
        "errors": outcomes["errors"],
        "miss_reasons": dict(sorted(reasons.items())),
        "misses_from_changed_inputs": from_changed,
    }


def eligible_hit_rate(hits: object, misses: object, from_changed: object = 0) -> float | None:
    """None when nothing was eligible: no data is not a 0% hit rate."""
    if not all(type(v) is int and v >= 0 for v in (hits, misses, from_changed)):
        return None
    denominator = hits + misses - min(from_changed, misses)
    return hits / denominator if denominator else None


def _size(text: str) -> int | None:
    match = re.fullmatch(r"([\d.]+)([KMGT]?B)", text)
    return int(float(match[1]) * SIZE_UNITS[match[2]]) if match else None


def layers_from_log(text: str) -> list[dict]:
    """setup-soldr's restore table: name, HIT/MISS, key, archive, inflated, files, seconds."""
    layers = []
    for match in LAYER_LINE.finditer(ANSI.sub("", text)):
        layers.append(
            {"layer": match[1], "status": match[2], "bytes": _size(match[4]), "seconds": float(match[7])}
        )
    return layers


def counters_from_log(text: str) -> dict | None:
    """The last `zccache status` block a job printed: it has no miss reasons."""
    matches = STATUS_LINE.findall(text)
    if not matches:
        return None
    total, hits, misses, non_cacheable = map(int, matches[-1])
    return {
        "compilations": total, "hits": hits, "misses": misses, "non_cacheable": non_cacheable,
        "errors": 0, "miss_reasons": {}, "misses_from_changed_inputs": 0,
    }


def wrapper_from_log(text: str) -> str | None:
    match = WRAPPER_LINE.search(text)
    return match[1] if match else None


def counters_from_report(raw: str) -> tuple[dict | None, dict]:
    """(counters or None, version fields). Every malformed shape degrades to None."""
    try:
        data = json.loads(raw)
    except (TypeError, ValueError):
        return None, {}
    if not isinstance(data, dict):
        return None, {}
    versions = {"soldr": data.get("soldr_version"), "zccache": data.get("managed_zccache_version")}
    session = data.get("last_session")
    if not isinstance(session, dict):
        return None, versions
    fields = ("compilations", "hits", "misses", "non_cacheable", "errors")
    counters = {k: session[k] if type(session.get(k)) is int else 0 for k in fields}
    counters |= {"miss_reasons": {}, "misses_from_changed_inputs": 0}
    return counters, versions


def layers_from_phase_summary(raw: str | None) -> tuple[list[dict], object]:
    """setup-soldr's JSON output, kept verbatim when small; its layout is not a stable contract."""
    if not raw or len(raw) > MAX_PHASE_SUMMARY_BYTES:
        return [], None
    try:
        data = json.loads(raw)
    except ValueError:
        return [], None
    layers = []
    entries = data.get("caches") if isinstance(data, dict) else None
    for item in entries if isinstance(entries, list) else []:
        if isinstance(item, dict) and isinstance(item.get("name"), str) and isinstance(item.get("status"), str):
            layers.append({"layer": item["name"], "status": item["status"].upper(),
                           "bytes": item.get("bytes"), "seconds": item.get("seconds")})
    return layers, data


def read_changed_paths(path: Path | None) -> set[str] | None:
    if path is None:
        return None
    try:
        return {line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()}
    except (OSError, UnicodeError):
        return None


def fetch_changed_paths(repo: str, base: str, head: str, token: str | None) -> set[str] | None:
    """GitHub's compare API caps at 300 files: a full page may be partial, so it is unknown."""
    if not (repo and base and head) or set(base) == {"0"}:
        return None
    request = urllib.request.Request(
        f"https://api.github.com/repos/{repo}/compare/{base}...{head}?per_page=300",
        headers={"Accept": "application/vnd.github+json", **({"Authorization": f"Bearer {token}"} if token else {})},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            files = json.load(response).get("files")
    except (OSError, ValueError, AttributeError, urllib.error.URLError):
        return None
    if not isinstance(files, list) or len(files) >= 300:
        return None
    return {f["filename"] for f in files if isinstance(f, dict) and isinstance(f.get("filename"), str)}


def build_record(
    *, meta: dict, wrapper: str | None, counters: dict | None, source: str, versions: dict,
    layers: list[dict], changed_known: bool, notes: list[str] | None = None,
) -> dict:
    bypassed = wrapper == "none"
    counts = counters or {"compilations": 0, "hits": 0, "misses": 0, "non_cacheable": 0, "errors": 0,
                          "miss_reasons": {}, "misses_from_changed_inputs": 0}
    rate = eligible_hit_rate(counts["hits"], counts["misses"], counts["misses_from_changed_inputs"])
    if rate is None:
        display = "n/a (bypassed)" if bypassed else "n/a (no data)" if counters is None else "n/a (no compilations)"
        scope = None
    else:
        scope = "non-rust" if bypassed else "all"
        display = f"{rate * 100:.1f}%" + (" (non-Rust)" if bypassed else "")
    return {
        "schema": SCHEMA,
        **meta,
        "soldr_version": versions.get("soldr"),
        "zccache_version": versions.get("zccache"),
        "wrapper_state": "bypassed" if bypassed else "active",
        "counter_source": source,
        **counts,
        "eligible_hit_rate": rate,
        "eligible_hit_rate_display": display,
        "rate_scope": scope,
        "changed_inputs_known": changed_known,
        "layers": layers or None,
        "notes": notes or [],
    }


def summary_line(record: dict) -> str:
    layers = record.get("layers") or []
    hit = sum(1 for layer in layers if layer.get("status") == "HIT")
    layer_text = f"; layers {hit}/{len(layers)} HIT" if layers else ""
    return (
        f"- cache ledger `{record.get('lane')}`: **{record.get('eligible_hit_rate_display')}** "
        f"({record.get('hits')} hits / {record.get('misses')} misses, source {record.get('counter_source')}){layer_text}"
    )


def collect(args: argparse.Namespace) -> dict:
    env = os.environ
    meta = {
        "job": args.job or env.get("GITHUB_JOB"), "lane": args.lane or args.job or env.get("GITHUB_JOB"),
        "os": args.os or env.get("RUNNER_OS"), "arch": args.arch or env.get("RUNNER_ARCH"),
        "sha": env.get("GITHUB_SHA"), "event": env.get("GITHUB_EVENT_NAME"), "run_id": env.get("GITHUB_RUN_ID"),
        "run_attempt": env.get("GITHUB_RUN_ATTEMPT"),
    }
    root = Path(args.workspace or env.get("GITHUB_WORKSPACE") or ".")
    changed = read_changed_paths(args.changed_paths)
    if changed is None and args.base_sha:
        changed = fetch_changed_paths(env.get("GITHUB_REPOSITORY", ""), args.base_sha, env.get("GITHUB_SHA", ""),
                                      env.get("GITHUB_TOKEN"))
    report_counters, versions = counters_from_report(read_report())
    since = parse_ts(args.since)
    cache_dir = args.cache_dir or (Path(env["ZCCACHE_CACHE_DIR"]) if env.get("ZCCACHE_CACHE_DIR") else None)
    rows, raw_rows, malformed = read_journals(cache_dir, since)
    notes = []
    if rows:
        counters, source = count_rows(rows, changed, root), "journal"
        notes.append(f"journal rows: {raw_rows} raw, {len(rows)} unique, {malformed} malformed")
    elif report_counters is not None:
        counters, source = report_counters, "soldr-report"
    else:
        counters, source = None, "none"
    layers, phase = layers_from_phase_summary(env.get("CACHE_LEDGER_SETUP_PHASE_SUMMARY"))
    record = build_record(meta=meta, wrapper=env.get("SOLDR_RUSTC_WRAPPER"), counters=counters, source=source,
                          versions=versions, layers=layers, changed_known=changed is not None, notes=notes)
    if phase is not None:
        record["setup_phase_summary"] = phase
    return record


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--summary", type=Path, help="append one Markdown line here")
    parser.add_argument("--job")
    parser.add_argument("--lane")
    parser.add_argument("--os")
    parser.add_argument("--arch")
    parser.add_argument("--cache-dir", type=Path)
    parser.add_argument("--workspace")
    parser.add_argument("--since", help="ignore journal rows older than this ISO time")
    parser.add_argument("--changed-paths", type=Path, help="file with one changed path per line")
    parser.add_argument("--base-sha", help="ask GitHub for the paths changed since this commit")
    args = parser.parse_args(argv)
    try:
        record = collect(args)
    except Exception as error:  # noqa: BLE001 - observability must never fail the job
        record = {"schema": SCHEMA, "job": args.job, "lane": args.lane, "counter_source": "none",
                  "eligible_hit_rate": None, "eligible_hit_rate_display": "n/a (ledger error)",
                  "notes": [f"ledger error: {type(error).__name__}"]}
    try:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        if args.summary:
            with args.summary.open("a", encoding="utf-8") as stream:
                stream.write(summary_line(record) + "\n")
    except OSError as error:
        print(f"cache ledger not written: {error}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
