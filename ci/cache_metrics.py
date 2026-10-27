#!/usr/bin/env -S uv run --script
"""Read-only CI/cache evidence collector for #1286; needs authenticated gh.

Raw run/job/changed-file/cache responses remain under --output. Sampling is
stratified by event and outcome across the requested window. Cohorts use
the actual compare diff where available, not branch names or skipped tests.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import math
import re
import statistics
import subprocess
from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
from pathlib import Path


def gh(*args: str):
    return json.loads(subprocess.run(["gh", *args], check=True, capture_output=True, text=True).stdout)


def stamp(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def seconds(start: str | None, end: str | None) -> float:
    return max(0.0, (stamp(end) - stamp(start)).total_seconds()) if start and end else 0.0


def creation_to_start_seconds(job: dict) -> float | None:
    """Observed creation to start; may include dependency waiting, not pure dispatch latency."""
    if job.get("conclusion") == "skipped" or not job.get("created_at") or not job.get("started_at"):
        return None
    duration = (stamp(job["started_at"]) - stamp(job["created_at"])).total_seconds()
    return duration if duration >= 0 else None


def p95(values: list[float]) -> float | None:
    """Nearest-rank sample p95; small samples usually return their maximum."""
    return sorted(values)[math.ceil(len(values) * 0.95) - 1] if values else None


def cohort(files: list[str]) -> str:
    if not files:
        return "unknown"
    def documentation(name: str) -> bool:
        # Retired from master (#2686), but the runs being classified are
        # historical: their changed-file lists still name the snapshots, and
        # those runs were documentation-only.
        return name.endswith(".md") or name.startswith("docs/ci-evidence/")

    if all(documentation(name) for name in files):
        return "docs"
    if all(documentation(name) or name.startswith(("ci/", ".github/", ".claude/"))
           or name in {"ci.toml", "local-gate.toml"} for name in files):
        return "ci-config"
    return "code"


def lane_summaries(records: list[dict]) -> dict:
    """Successful jobs by event/source cohort, with literal step timings.

    Parent workflow outcomes are disclosed; an already successful job may
    belong to a workflow that later failed or was canceled. Missing/unused
    steps never become zero-duration samples.
    """
    buckets = defaultdict(list)
    for record in records:
        for job in record['jobs']:
            if job['conclusion'] == 'success' and job.get('started_at') and job.get('completed_at'):
                key = f"{record['event']}/{record['cohort']}/{job['name']}"
                buckets[key].append((record, job))
    result = {}
    for key, samples in buckets.items():
        steps = defaultdict(list)
        durations = [seconds(job['started_at'], job['completed_at']) for _, job in samples]
        dispatch = [value for _, job in samples if (value := creation_to_start_seconds(job)) is not None]
        for _, job in samples:
            for step in job.get('steps', []):
                if step.get('conclusion') == 'success' and step.get('started_at') and step.get('completed_at'):
                    steps[step['name']].append(seconds(step['started_at'], step['completed_at']))
        result[key] = {
            'successful_job_samples': len(samples),
            'parent_run_outcomes': dict(Counter(record['conclusion'] for record, _ in samples)),
            'median_job_seconds': statistics.median(durations),
            'p95_job_seconds': p95(durations),
            'creation_to_start_samples': len(dispatch),
            'median_creation_to_start_seconds': statistics.median(dispatch) if dispatch else None,
            'p95_creation_to_start_seconds': p95(dispatch),
            'steps': {name: {'samples': len(values), 'median_seconds': statistics.median(values)}
                      for name, values in sorted(steps.items())},
        }
    return result


def parse_cache_logs(text: str) -> dict:  # noqa: C901
    """Keep independent observations; archive restores are not compiler hits.

    Reports from different tools/scopes are never summed. Missing counters
    remain absent, and repeated archive messages are not a transfer total.
    """
    result = {}
    for line in text.splitlines():
        fields = line.split("\t", 2)
        if len(fields) != 3:
            continue
        name, _, message = fields
        observations = {}
        archive = re.search(r"Cache Size:.*?\((\d+) B\)", message)
        if archive:
            observations["archive_bytes"] = int(archive[1])
        compiled = re.search(r"Compilations:\s+(\d+) total \((\d+) cached, (\d+) cold, (\d+) non-cacheable\)", message)
        session = re.search(r"final zccache session stats: hits=(\d+) misses=(\d+) compilations=(\d+) non_cacheable=(\d+) errors=(\d+)", message)
        hits = re.search(r"\bHits:\s+(\d+)\s*/\s*(\d+)", message)
        if compiled:
            observations["compiler_reports"] = dict(zip(("total", "cached", "cold", "non_cacheable"), map(int, compiled.groups())), kind="zccache-build")
        elif session:
            observations["compiler_reports"] = dict(zip(("hits", "misses", "compilations", "non_cacheable", "errors"), map(int, session.groups())), kind="soldr-session")
        elif hits:
            observations["compiler_reports"] = {"kind": "ccache", "hits": int(hits[1]), "cacheable": int(hits[2])}
            if "flatpak" in name.casefold():
                # The pinned builder restores the whole state directory,
                # including statistics, after the separate ccache restore.
                # There is no counter reset/baseline after that final restore.
                observations["compiler_reports"].update(
                    counter_scope="restored-cumulative", job_delta_available=False)
        tests = re.search(r"(\d+) tests failed out of (\d+)", message)
        if tests:
            observations["ctest_reports"] = {"total": int(tests[2]), "failed": int(tests[1])}
        else:
            passed = re.search(r"\b100% tests passed out of (\d+)", message)
            if passed:
                observations["ctest_reports"] = {"total": int(passed[1]), "failed": 0}
        saved = re.search(r"Cache saved with key: (\S+)", message)
        if saved:
            observations["saved_keys"] = saved[1]
        version = re.search(r"\bzccache (\d+\.\d+\.\d+)\s*$", message)
        if version:
            observations["zccache_versions"] = version[1]
        for kind, value in observations.items():
            result.setdefault(name, {}).setdefault(kind, []).append(value)
    return result


def parse_rust_test_logs(text: str) -> dict:  # noqa: C901
    """Observed Cargo/nextest phases; absent markers remain unknown.

    Build summaries include linking. First execution marker to last result is
    an output span, not total process time. Nextest's own reported execution
    duration is separate; Cargo doctests remain a separate command sample.
    Linker-only costs and rebuild reasons are not available from these lines.
    """
    result = {}
    active = {}
    ansi = re.compile(r"(?:\x1b|\^\[)\[[0-?]*[ -/]*[@-~]")
    for line in text.splitlines():
        fields = line.split("\t", 2)
        if len(fields) != 3:
            continue
        job, _, message = fields
        message = ansi.sub("", message)
        timestamp, separator, body = message.partition(" ")
        if not separator:
            continue
        try:
            now = stamp(timestamp)
        except ValueError:
            continue
        if body.startswith("+ soldr cargo "):
            active.pop(job, None)
            nextest = body.startswith("+ soldr cargo nextest run ")
            cargo = body.startswith("+ soldr cargo test ")
            if not (nextest or cargo) or "--no-run" in body.split():
                continue
            runner = 'nextest' if nextest else ('cargo-doctests' if '--doc' in body.split() else 'cargo')
            observation = {"runner": runner, "command_started_at": timestamp,
                           "cargo_build_and_link_seconds": None,
                           "command_to_first_test_seconds": None, "observed_test_output_seconds": None,
                           "observed_command_to_last_result_seconds": None,
                           "nextest_reported_execution_seconds": None,
                           "nextest_summaries": [], "cargo_results": [], "result_summaries": 0}
            result.setdefault(job, []).append(observation)
            active[job] = (observation, now, None)
        if job not in active:
            continue
        observation, started, first_test = active[job]
        finished = re.search(r"Finished `test` profile .*? in ((?:\d+(?:\.\d+)?[hms]\s*)+)$", body)
        if finished:
            observation["cargo_build_and_link_seconds"] = sum(
                float(number) * {"h": 3600, "m": 60, "s": 1}[unit]
                for number, unit in re.findall(r"(\d+(?:\.\d+)?)([hms])", finished[1]))
        starting = re.search(r"\bStarting (\d+) tests? across (\d+) binar(?:y|ies)\b", body)
        if observation['runner'] == 'nextest' and starting:
            observation['nextest_start'] = {'tests_scheduled': int(starting[1]), 'binaries': int(starting[2])}
        execution_marker = (starting if observation['runner'] == 'nextest' else
                            re.search(r"\b(?:Running (?:unittests\b|tests/|benches/|examples/)|Doc-tests\b)", body))
        if first_test is None and now >= started and execution_marker:
            first_test = now
            observation["command_to_first_test_seconds"] = (now - started).total_seconds()
            active[job] = (observation, started, first_test)
        summary = re.search(r"\bSummary\s+\[\s*((?:\d+(?:\.\d+)?[hms]\s*)+)\]\s+(\d+)(?:/(\d+))? tests? run:\s*(.*)$", body)
        is_result = False
        if observation['runner'] == 'nextest' and summary:
            observation['nextest_reported_execution_seconds'] = sum(
                float(number) * {'h': 3600, 'm': 60, 's': 1}[unit]
                for number, unit in re.findall(r"(\d+(?:\.\d+)?)([hms])", summary[1]))
            record = {'tests_run': int(summary[2]), 'summary_text': summary[4],
                      'counts': {status.strip(): int(number) for number, status in
                                 re.findall(r"(\d+)\s+([a-z][a-z -]*)(?:,|$)", summary[4])}}
            if summary[3] is not None:
                record['tests_scheduled'] = int(summary[3])
            observation['nextest_summaries'].append(record)
            is_result = True
        elif observation['runner'] != 'nextest' and 'test result:' in body:
            observation['cargo_results'].append({status: int(number) for number, status in
                re.findall(r"(\d+) (passed|failed|ignored|measured|filtered out)\b", body)})
            is_result = True
        if is_result:
            observation["result_summaries"] += 1
            if now >= started:
                observation['observed_command_to_last_result_seconds'] = (now - started).total_seconds()
            if first_test is not None and now >= first_test:
                observation["observed_test_output_seconds"] = (now - first_test).total_seconds()
    return result


def collect_logs(repo: str, run_id: int, attempt: int, output: Path) -> dict:
    path = output / f"run-{run_id}-attempt-{attempt}.log"
    try:
        if not path.exists():
            command = ["gh", "run", "view", str(run_id), "-R", repo, "--attempt", str(attempt), "--log"]
            text = subprocess.run(command, check=True, capture_output=True, text=True).stdout
            path.write_text(text)
        text = path.read_text()
        return {"jobs": parse_cache_logs(text), "rust_tests": parse_rust_test_logs(text), "error": None}
    except subprocess.CalledProcessError as error:
        return {"jobs": {}, "error": str(error)}


def collect(repo: str, start: str, end: str, output: Path, per_group: int, include_logs: bool = False) -> dict:  # noqa: C901
    output.mkdir(parents=True, exist_ok=True)
    raw = []
    coverage = {}
    for workflow in ("ci.yml", "push.yaml"):
        listing = gh("run", "list", "-R", repo, "--workflow", workflow, "--limit", "2000", "--json",
                     "databaseId,status,conclusion,event,headSha,headBranch,createdAt,updatedAt,url")
        raw.extend(listing)
        oldest = min((r["createdAt"] for r in listing), default=None)
        coverage[workflow] = {"listed": len(listing), "oldest_created_at": oldest,
                              "window_covered": len(listing) < 2000 or stamp(oldest) <= stamp(start)}
    runs = [r for r in raw if stamp(start) <= stamp(r["createdAt"]) < stamp(end)]
    (output / "runs.json").write_text(json.dumps(runs, indent=2) + "\n")
    groups = defaultdict(list)
    for run in runs:
        if run["status"] == "completed":
            groups[(run["event"], run["conclusion"])].append(run)
    selected = []
    for group in groups.values():
        group.sort(key=lambda r: r["createdAt"])
        count = min(per_group, len(group))
        selected.extend(group[round(i * (len(group) - 1) / max(1, count - 1))] for i in range(count))
        # Ensure scarce docs scenarios are inspected too. This only chooses
        # candidates; classification below still requires the actual diff.
        docs = [r for r in group if (r.get("headBranch") or "").startswith(("docs/", "doc/"))]
        count = min(per_group, len(docs))
        selected.extend(docs[round(i * (len(docs) - 1) / max(1, count - 1))] for i in range(count))
    selected = list({r["databaseId"]: r for r in selected}.values())
    pull_requests = gh("pr", "list", "-R", repo, "--state", "all", "--limit", "2000", "--json",
                       "number,headRefName,headRefOid,baseRefOid,createdAt,closedAt,headRepositoryOwner")

    def inspect(run: dict) -> dict:  # noqa: C901
        run_id = run["databaseId"]
        cached_path = output / f"run-{run_id}.json"
        cached = json.loads(cached_path.read_text()) if cached_path.exists() else None
        cache_valid = cached and all(cached.get(key) == run[key] for key in ("updatedAt", "headSha", "status", "conclusion"))
        if cache_valid:
            jobs = cached["jobs"]
            detail = {"run_attempt": cached["run_attempt"], "pull_requests": []}
        else:
            detail = gh("api", f"repos/{repo}/actions/runs/{run_id}")
            pages = gh("api", f"repos/{repo}/actions/runs/{run_id}/jobs?per_page=100", "--paginate", "--slurp")
            jobs = [job for page in pages for job in page["jobs"]]
        # A later PR/base update cannot change an unchanged historical sample.
        base = cached.get("comparison_base") if cache_valid else None
        queue = re.search(r"/pr-\d+-([a-f0-9]{40})$", (run.get("headBranch") or ""))
        if queue:
            base = queue.group(1)
        elif detail.get("pull_requests"):
            base = detail["pull_requests"][0]["base"]["sha"]
        elif base is None and run["event"] == "pull_request":
            # GitHub often drops associations from old/merged run metadata.
            # Resolve only an unambiguous branch/lifetime match; never guess
            # docs from a missing association or the branch name itself.
            candidates = [pr for pr in pull_requests if pr["headRefName"] == run["headBranch"]
                          and pr["createdAt"] <= run["createdAt"]
                          and (not pr["closedAt"] or run["createdAt"] <= pr["closedAt"])]
            exact = [pr for pr in candidates if pr["headRefOid"] == run["headSha"]]
            candidates = exact or candidates
            if len(candidates) == 1:
                base = candidates[0]["baseRefOid"]
        files = []
        comparison_error = None
        if cached and cached.get("comparison_base") == base and not cached.get("comparison_error"):
            files = cached["changed_files"]
        elif base:
            try:
                compare = gh("api", f"repos/{repo}/compare/{base}...{run['headSha']}")
                files = [f["filename"] for f in compare["files"]]
                if len(files) >= 300:
                    comparison_error = "GitHub compare file limit; cohort unknown"
            except subprocess.CalledProcessError as error:
                comparison_error = str(error)
        gates = [j for j in jobs if j["name"] == "CI Gate ✅" and j.get("completed_at")]
        record = {
            **run, "cohort": "unknown" if comparison_error else cohort(files),
            "changed_files": files, "comparison_base": base, "comparison_error": comparison_error,
            "run_attempt": detail["run_attempt"], "jobs": jobs,
            "gate_seconds": seconds(run["createdAt"], gates[-1]["completed_at"]) if gates else None,
            "runner_minutes": round(sum(seconds(j.get("started_at"), j.get("completed_at"))
                                        for j in jobs if j["conclusion"] != "skipped") / 60, 3),
        }
        if include_logs:
            record["log_evidence"] = collect_logs(repo, run_id, detail["run_attempt"], output)
        (output / f"run-{run_id}.json").write_text(json.dumps(record, indent=2) + "\n")
        return record

    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        records = list(pool.map(inspect, selected))
    caches = gh("api", f"repos/{repo}/actions/caches?per_page=100", "--paginate", "--slurp")
    entries = [entry for page in caches for entry in page["actions_caches"]]
    (output / "caches.json").write_text(json.dumps(entries, indent=2) + "\n")
    summary = {"start": start, "end": end, "captured_at": datetime.now(timezone.utc).isoformat(),
               "listed_runs": len(runs), "sampled_runs": len(records),
               "sample_per_event_outcome": per_group, "cohorts": {},
               "sampling": "Evenly spaced by event/outcome, plus verified documentation candidates",
               "listing_coverage": coverage, "outcomes": {}, "lanes": {},
               "cache_bytes": sum(c["size_in_bytes"] for c in entries), "cache_entries": len(entries),
               "limitations": ["Latest run attempt only; overwritten attempts are not reconstructed.",
                               "Markdown and docs/ci-evidence JSON are documentation; other JSON can be product code.",
                               "Unknown changed-file comparisons are not classified as docs.",
                               "Master pushes without PR metadata have an unknown source cohort.",
                               "Per-workflow listing limit is 2000; inspect coverage for larger windows.",
                               "Creation-to-start intervals may include dependency waiting; they do not isolate runner dispatch latency or prove capacity exhaustion.",
                               "p95 uses nearest rank; sparse samples often return their maximum, not a stable population tail."]}
    buckets = defaultdict(list)
    outcomes = defaultdict(lambda: defaultdict(int))
    lanes = defaultdict(list)
    for run in runs:
        outcomes[run["event"]][run["conclusion"] or run["status"]] += 1
    summary["outcomes"] = dict(outcomes)
    for record in records:
        buckets[f"{record['event']}/{record['cohort']}"].append(record)
        for job in record["jobs"]:
            if job["conclusion"] != "success":
                continue
            setup = sum(seconds(step.get("started_at"), step.get("completed_at"))
                        for step in job.get("steps", []) if "set up soldr" in step["name"].lower()
                        and not step["name"].lower().startswith("post"))
            lanes[f"{record['event']}/{job['name']}"].append({
                "job_seconds": seconds(job.get("started_at"), job.get("completed_at")),
                "soldr_setup_seconds": setup,
            })
    for name, bucket in buckets.items():
        passing = [r["gate_seconds"] for r in bucket if r["conclusion"] == "success" and r["gate_seconds"] is not None]
        summary["cohorts"][name] = {
            "sampled_runs": len(bucket), "successful_gate_samples": len(passing),
            "median_successful_gate_seconds": statistics.median(passing) if passing else None,
            "p95_successful_gate_seconds": p95(passing),
            "canceled_runner_minutes": round(sum(r["runner_minutes"] for r in bucket if r["conclusion"] == "cancelled"), 3),
        }
    for name, samples in lanes.items():
        summary["lanes"][name] = {"successful_job_samples": len(samples), **{
            f"median_{key}": statistics.median(sample[key] for sample in samples)
            for key in ("job_seconds", "soldr_setup_seconds")
        }}
    summary["lane_cohorts"] = lane_summaries(records)
    if include_logs:
        summary["log_evidence"] = [{"run_id": r["databaseId"], "event": r["event"], "cohort": r["cohort"],
                                    **r["log_evidence"]} for r in records]
        summary["limitations"].extend([
            "Log counters have different compiler/job scopes and are not summed.",
            "Archive byte observations can repeat and are not total transfer bytes.",
            "Missing/expired logs or absent counters are unknown, never zero hits.",
            "Flatpak ccache counters include restored history; job deltas are unavailable without a final-restore baseline.",
            "Compiler-version changes require separate cold/warm cohorts.",
        ])
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    return summary


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", default="obs-rust/obs-studio")
    parser.add_argument("--start", default=(datetime.now(timezone.utc) - timedelta(days=1)).strftime("%Y-%m-%dT%H:%M:%SZ"))
    parser.add_argument("--end", default=datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--per-group", type=int, default=12)
    parser.add_argument("--include-logs", action="store_true", help="Collect cached per-attempt logs and observed compiler/cache/test counters")
    args = parser.parse_args()
    if args.per_group < 1 or stamp(args.start) >= stamp(args.end):
        parser.error("Use a positive per-group count and an increasing UTC time window")
    print(json.dumps(collect(args.repo, args.start, args.end, args.output, args.per_group, args.include_logs), indent=2))
