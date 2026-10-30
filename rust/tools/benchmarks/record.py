#!/usr/bin/env -S uv run --script
"""Run every #142 Criterion comparison repeatedly and record paired medians.

Usage: uv run rust/tools/benchmarks/record.py --cpu 7 --output baseline.json
       uv run rust/tools/benchmarks/record.py --category format --output format.json

CATEGORIES must all be in a full run; the port benches add other groups,
which are recorded too (#3003). --category runs and requires only the named
categories, for the before/after numbers of one kernel (#3009).
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess

ROOT = Path(__file__).resolve().parents[3]
CATEGORIES = {
    "bmem", "dstr", "vec2", "vec3", "vec4", "crc32", "avc", "hevc", "av1",
    "decl", "calldata", "proc", "signal", "conversions",
    # The SIMD candidates of #3009 (audio_mix: the mix add and the output tick).
    "format", "volmeter", "audio_mix",
}
COMMAND = ["soldr", "cargo", "bench", "-p", "obs-c-oracle", "--bench", "port_comparison", "--"]


def source_hash() -> str:
    paths = {ROOT / "Cargo.toml", ROOT / "Cargo.lock"}
    for directory in ["rust", "libobs", "deps/simde", "deps/libcaption"]:
        paths.update(
            path for path in (ROOT / directory).rglob("*")
            if path.is_file() and path.suffix in {".rs", ".c", ".h", ".toml"}
            and "target" not in path.relative_to(ROOT).parts
        )
    digest = hashlib.sha256()
    for path in sorted(paths):
        digest.update(str(path.relative_to(ROOT)).encode() + b"\0")
        digest.update(path.read_bytes())
    return digest.hexdigest()


def bench_command(categories: list[str]) -> list[str]:
    """The cargo bench command; with categories, a Criterion filter for them."""
    if not categories:
        return list(COMMAND)
    return COMMAND + ["^(" + "|".join(categories) + ")/"]


def cpu_name() -> str:
    """The CPU model: /proc/cpuinfo on Linux, sysctl on macOS and the
    registry on Windows (where platform.processor() is only "arm" or the
    CPU family), else platform.processor()."""
    if platform.system() == "Darwin":
        result = subprocess.run(["sysctl", "-n", "machdep.cpu.brand_string"], text=True, capture_output=True, check=False)
        return result.stdout.strip() or platform.processor()
    if platform.system() == "Windows":
        import winreg

        path = r"HARDWARE\DESCRIPTION\System\CentralProcessor\0"
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, path) as key:
            return str(winreg.QueryValueEx(key, "ProcessorNameString")[0]).strip() or platform.processor()
    if Path("/proc/cpuinfo").exists():
        lines = Path("/proc/cpuinfo").read_text().splitlines()
        return next((line.split(":", 1)[1].strip() for line in lines if line.startswith("model name")), platform.processor())
    return platform.processor()


def version(command: list[str]) -> str:
    try:
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
        return (result.stdout + result.stderr).strip()
    except OSError as error:
        return str(error)


def read_run(directory: Path, baseline: str, required: set[str] | None = None) -> dict:
    """Every pair of one baseline. A full run (required None) needs all CATEGORIES and 70 pairs."""
    entries = {}
    for path in directory.rglob("benchmark.json"):
        if path.parent.name != baseline:
            continue
        benchmark = json.loads(path.read_text())
        backend = benchmark["function_id"]
        if backend not in {"rust", "c"}:
            raise ValueError(f"unexpected backend {backend}: {path}")
        key = benchmark["group_id"]
        if benchmark["value_str"] is not None:
            key += "/" + benchmark["value_str"]
        median = json.loads((path.parent / "estimates.json").read_text())["median"]
        point = median["point_estimate"]
        if point <= 0:
            raise ValueError(f"invalid duration {point}: {path}")
        if backend in entries.setdefault(key, {}):
            raise ValueError(f"duplicate measurement {key}/{backend}")
        entries[key][backend] = {
            "median_ns": point,
            "confidence_interval_ns": [
                median["confidence_interval"]["lower_bound"],
                median["confidence_interval"]["upper_bound"],
            ],
        }
    missing = (CATEGORIES if required is None else required) - {key.split("/")[0] for key in entries}
    if missing or (required is None and len(entries) < 70):
        raise ValueError(f"incomplete benchmark inventory ({len(entries)} pairs, missing {sorted(missing)})")
    for key, pair in entries.items():
        if set(pair) != {"rust", "c"}:
            raise ValueError(f"missing comparison for {key}")
    return entries


def summarize(runs: list[dict]) -> list[dict]:
    keys = set(runs[0])
    if any(set(run) != keys for run in runs):
        raise ValueError("benchmark inventory changed between runs")
    rows = []
    for key in sorted(keys):
        row = {"benchmark": key}
        for backend in ["rust", "c"]:
            points = [run[key][backend]["median_ns"] for run in runs]
            row[backend] = {
                "median_ns": statistics.median(points),
                "range_ns": [min(points), max(points)],
                "runs": [run[key][backend] for run in runs],
            }
        ratio = row["rust"]["median_ns"] / row["c"]["median_ns"]
        ratios = [run[key]["rust"]["median_ns"] / run[key]["c"]["median_ns"] for run in runs]
        row["rust_over_c"] = ratio
        row["paired_run_ratio_range"] = [min(ratios), max(ratios)]
        row["status"] = (
            "C control in both builds" if key.startswith("dstr/printf_control/")
            else "over 10% bound" if ratio > 1.10 else "within 10% bound"
        )
        rows.append(row)
    return rows


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--cpu", type=int)
    parser.add_argument("--category", action="append", default=[], choices=sorted(CATEGORIES),
                        help="run and require only this category (repeatable)")
    args = parser.parse_args()
    if args.repeats < 3:
        parser.error("at least three complete runs are required for a median and range")
    if args.cpu is not None:
        if not hasattr(os, "sched_setaffinity"):
            parser.error("--cpu requires OS affinity support")
        os.sched_setaffinity(0, {args.cpu})
    fingerprint = source_hash()
    command = bench_command(args.category)
    required = set(args.category) or None
    # Smoke executes fixture checks and exercises ownership without timing.
    subprocess.run(command + ["--test"], cwd=ROOT, check=True)
    target = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    options = ["--sample-size", "30", "--warm-up-time", "0.1", "--measurement-time", "0.4", "--nresamples", "10000", "--noplot"]
    baselines, runs = [], []
    for repeat in range(args.repeats):
        baseline = f"ports-{timestamp}-{repeat + 1}"
        print(f"Recording {repeat + 1}/{args.repeats}: {baseline}", flush=True)
        subprocess.run(command + options + ["--save-baseline", baseline], cwd=ROOT, check=True)
        runs.append(read_run(target / "criterion", baseline, required))
        baselines.append(baseline)
        if source_hash() != fingerprint:
            raise ValueError("source changed during measurement; discard this run")
    cpu = cpu_name()
    report = {
        "schema": 1, "timestamp_utc": timestamp, "source_sha256": fingerprint,
        "base_commit": version(["git", "rev-parse", "HEAD"]),
        "cpu": cpu, "affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
        "os": {"system": platform.system(), "release": platform.release(), "machine": platform.machine()},
        "rustc": version(["soldr", "rustc", "-vV"]),
        "cc": version([os.environ.get("CC", "cl" if os.name == "nt" else "cc"), "--version"]),
        "environment": {key: os.environ.get(key) for key in ["CC", "CFLAGS", "RUSTFLAGS"]},
        "criterion": "0.8.2", "profile": "bench (release, opt-level 3)",
        "command": command + options, "categories": sorted(required or CATEGORIES), "baselines": baselines,
        "threshold_rust_over_c": 1.10, "results": summarize(runs),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Recorded {len(report['results'])} complete comparisons: {args.output}", flush=True)


if __name__ == "__main__":
    main()
