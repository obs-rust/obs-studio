#!/usr/bin/env -S uv run --script
"""Run every #142 Criterion comparison repeatedly and record paired medians.

Usage: uv run rust/tools/benchmarks/record.py --cpu 7 --output baseline.json
       uv run rust/tools/benchmarks/record.py --category format --output format.json

CATEGORIES must all be in a full run; the port benches add other groups,
which are recorded too (#3003). --category runs and requires only the named
categories, for the before/after numbers of one kernel (#3009).

--max-ratio R turns a recorded pair above the Rust/C time bound into a failed
run, so a known breach is reproducible as a check instead of a report row
(#3138). The bound is the one the release criteria use.
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
# The C flags the oracle is compiled with (#3127). build.rs clamps an optimized
# profile's C to the shipped libobs level (-O2 -g, RelWithDebInfo) instead of the
# bench profile's opt-level 3, because a -O3 oracle vectorizes audio loops the
# shipped C leaves scalar. -ffp-contract=off keeps the C strict IEEE so Tier 3
# bit comparison against the Rust stays meaningful; the shipped C fuses on macOS
# arm64, so a small shipped-C advantage there is expected. MSVC gets the same
# level as /O2. Recorded so a baseline names the C it measured, not the profile.
C_FLAGS = "-O2 -ffp-contract=off -fno-strict-aliasing"
# The Rust/C median time ratio the release criteria bound (#1295, #3138).
BOUND = 1.10
# Pairs the maintainer exempted from that ratio by name (#3138), with the
# absolute cost, the machine it was measured on and the issue that decided it.
# An exempt pair stays in the report; only the `--max-ratio` gate skips it.
# `docs/rust-port/release-criteria.md` scopes the bound to pairs where the Rust
# side is not a thin shim: this one is one `extern "C"` trampoline around a
# `deque_push_front` the C keeps header-inline, called once per audio tick, so
# the ratio measures the boundary and not a kernel. The 5% full-app bound of
# #142 still covers it.
BOUND_EXEMPTIONS = {
    "audio_mix/buffering_add_one_tick": (
        "46 ns Rust vs 40 ns C (1.13x on a loaded box 2026-10-31, 1.15-1.21x in "
        "#3141's paired runs) on a Ryzen 7 3700X: one FFI trampoline on a "
        "once-per-audio-tick call, no hot spot left after #3141; exempt decided "
        "in #3138, full-app cost still bounded by #142"
    ),
}


def report_metadata(
    *,
    timestamp: str,
    fingerprint: str,
    cpu: str,
    required: set[str] | None,
    command: list[str],
    options: list[str],
    baselines: list[str],
    base_commit: str = "",
    rustc: str = "",
    cc: str = "",
    environment: dict | None = None,
    affinity: list[int] | None = None,
) -> dict:
    """The report fields that describe how and where a run measured."""
    return {
        "schema": 1,
        "timestamp_utc": timestamp,
        "source_sha256": fingerprint,
        "base_commit": base_commit,
        "cpu": cpu,
        "affinity": affinity,
        "os": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
        },
        "rustc": rustc,
        "cc": cc,
        "environment": environment if environment is not None else {},
        "c_flags": C_FLAGS,
        "threshold_rust_over_c": BOUND,
        "bound_exemptions": dict(BOUND_EXEMPTIONS),
        "criterion": "0.8.2",
        "profile": "bench (release, opt-level 3)",
        "command": command + options,
        "categories": sorted(required or CATEGORIES),
        "baselines": baselines,
    }


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


def pair_status(key: str, ratio: float, exempt: bool) -> str:
    """The report's per-pair verdict, naming the bound or the exemption."""
    if key.startswith("dstr/printf_control/"):
        return "C control in both builds"
    if ratio <= BOUND:
        return f"within {BOUND:.0%} bound"
    if exempt:
        return f"exempt from {BOUND:.0%} bound (#3138)"
    return f"over {BOUND:.0%} bound"


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
        row["status"] = pair_status(key, ratio, key in BOUND_EXEMPTIONS)
        rows.append(row)
    return rows


def over_bound(results: list[dict], bound: float = BOUND) -> list[dict]:
    """Rows whose median Rust/C ratio is above `bound` (#3138).

    `dstr/printf_control` runs C in both builds, so its ratio says nothing
    about the port and is never a breach. A pair named in `BOUND_EXEMPTIONS`
    is skipped by name: it stays in the report with its ratio and absolute
    numbers, but the maintainer decided the bound does not apply to it (#3138).
    """
    return [
        row for row in results
        if row["status"] != "C control in both builds"
        and row["benchmark"] not in BOUND_EXEMPTIONS
        and row["rust_over_c"] > bound
    ]


def enforce_bound(results: list[dict], bound: float) -> None:
    """Print the exempt pairs and the breaches, then fail on any breach.

    An exempt pair that is over the bound is printed with its recorded reason
    so a reader of the log sees the pair was measured, not dropped (#3138).
    """
    for row in results:
        if row["benchmark"] in BOUND_EXEMPTIONS and row["rust_over_c"] > bound:
            print(
                f"exempt ({row['rust_over_c']:.2f}x): {row['benchmark']} — "
                f"{BOUND_EXEMPTIONS[row['benchmark']]}",
                flush=True,
            )
    breaches = over_bound(results, bound)
    for row in breaches:
        print(
            f"over {bound:.2f}: {row['benchmark']} "
            f"{row['rust_over_c']:.2f}x the C ({row['status']})",
            flush=True,
        )
    if breaches:
        raise SystemExit(1)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--cpu", type=int)
    parser.add_argument("--category", action="append", default=[], choices=sorted(CATEGORIES),
                        help="run and require only this category (repeatable)")
    parser.add_argument("--max-ratio", type=float,
                        help="fail the run when a pair's median Rust/C ratio is above this")
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
    report = report_metadata(
        timestamp=timestamp,
        fingerprint=fingerprint,
        cpu=cpu_name(),
        required=required,
        command=command,
        options=options,
        baselines=baselines,
        base_commit=version(["git", "rev-parse", "HEAD"]),
        rustc=version(["soldr", "rustc", "-vV"]),
        cc=version([os.environ.get("CC", "cl" if os.name == "nt" else "cc"), "--version"]),
        environment={key: os.environ.get(key) for key in ["CC", "CFLAGS", "RUSTFLAGS"]},
        affinity=sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
    )
    report["results"] = summarize(runs)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Recorded {len(report['results'])} complete comparisons: {args.output}", flush=True)
    if args.max_ratio is not None:
        enforce_bound(report["results"], args.max_ratio)


if __name__ == "__main__":
    main()
