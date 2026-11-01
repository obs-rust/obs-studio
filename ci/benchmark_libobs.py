#!/usr/bin/env -S uv run --script
"""Measure an already configured, dedicated libobs build; keep all results.

Run inside the same compiler/dependency environment used to configure it.
Only use a fresh benchmark build directory: --rebuild moves its compiler
outputs to safe-rm's trash, preserving the configured graph and cache.
"""

from __future__ import annotations

import argparse
import json
import os
import resource
import subprocess
import time
import statistics
from pathlib import Path


def measure(build: Path, output: Path, repetitions: int, jobs: int, rebuild: bool, cache_stats: bool) -> dict:
    build = build.resolve()
    output.mkdir(parents=True, exist_ok=True)
    cache = (build / "CMakeCache.txt").read_text()
    settings = {}
    for line in cache.splitlines():
        if line and not line.startswith(("#", "//")) and ":" in line and "=" in line:
            name, value = line.split("=", 1)
            settings[name.split(":", 1)[0]] = value
    source = Path(settings["CMAKE_HOME_DIRECTORY"])
    revision = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=source, capture_output=True, text=True, check=True
    ).stdout.strip()
    command = ["cmake", "--build", str(build), "--target", "libobs", "--parallel", str(jobs)]
    result = {
        "revision": revision, "target": "libobs", "jobs": jobs,
        "command": command, "settings": {
            key: value for key, value in settings.items()
            if key in {"CMAKE_BUILD_TYPE", "CMAKE_C_COMPILER", "CMAKE_C_COMPILER_LAUNCHER",
                       "CMAKE_CXX_COMPILER", "ENABLE_PCH", "ENABLE_UNITY_BUILD", "ENABLE_RUST_LIBOBS",
                       "OBS_LIBOBS_BACKEND", "OBS_BUILD_C_REFERENCE", "OBS_BRAND_RUST"}
        },
        "runs": [],
    }
    for index in range(repetitions):
        if cache_stats:
            if not os.environ.get("CCACHE_DIR"):
                raise ValueError("Set a dedicated CCACHE_DIR for benchmark statistics")
            subprocess.run(["ccache", "--zero-stats"], check=True, stdout=subprocess.DEVNULL)
        if rebuild:
            artifacts = [p for p in build.rglob("*") if p.is_file() and p.suffix in {".o", ".gch", ".pch"}]
            if artifacts:
                subprocess.run(["safe-rm", "--", *map(str, artifacts)], check=True, stdout=subprocess.DEVNULL)
        before = resource.getrusage(resource.RUSAGE_CHILDREN)
        load = os.getloadavg()
        start = time.monotonic()
        with (output / f"build-{index + 1}.log").open("w") as log:
            process = subprocess.run(command, cwd=source, stdout=log, stderr=subprocess.STDOUT)
        after = resource.getrusage(resource.RUSAGE_CHILDREN)
        result["runs"].append({
            "wall_seconds": round(time.monotonic() - start, 3),
            "cpu_seconds": round(after.ru_utime + after.ru_stime - before.ru_utime - before.ru_stime, 3),
            "load_average": load, "returncode": process.returncode,
        })
        if cache_stats:
            stats = subprocess.run(["ccache", "--show-stats", "--verbose"], capture_output=True, text=True, check=True)
            (output / f"cache-{index + 1}.log").write_text(stats.stdout)
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        if process.returncode:
            raise RuntimeError(f"Build failed; see {output / f'build-{index + 1}.log'}")
    return result


def flatten(sample: dict, prefix: str = "") -> dict:
    values = {}
    for key, value in sample.items():
        if isinstance(value, dict):
            values.update(flatten(value, f"{prefix}{key}."))
        elif isinstance(value, (int, float)) and not isinstance(value, bool):
            values[prefix + key] = value
    return values


def summarize_runtime(runs: list[dict]) -> dict:
    """Median, min, max and relative spread ((max - min) / median) per numeric field."""
    if not runs:
        raise ValueError("no runtime samples")
    flat = [flatten(run) for run in runs]
    summary = {}
    for key in flat[0]:
        values = [run[key] for run in flat if key in run]
        median = statistics.median(values)
        summary[key] = {
            "median": median, "min": min(values), "max": max(values),
            "spread": round((max(values) - min(values)) / median, 4) if median else 0.0,
        }
    return summary


def measure_runtime(executable: Path, output: Path, repetitions: int, seconds: float, warmup: float) -> dict:
    """Run obs_runtime_bench (test/cmocka/obs_runtime_bench.c) and keep each JSON sample."""
    output.mkdir(parents=True, exist_ok=True)
    command = [str(executable), "--seconds", str(seconds), "--warmup", str(warmup)]
    result = {"command": command, "runs": []}
    for index in range(repetitions):
        load = os.getloadavg()
        process = subprocess.run(command, cwd=executable.parent, capture_output=True, text=True)
        (output / f"runtime-{index + 1}.log").write_text(process.stdout + process.stderr)
        if process.returncode:
            raise RuntimeError(f"runtime benchmark exited {process.returncode}; see runtime-{index + 1}.log")
        lines = [line for line in process.stdout.splitlines() if line.startswith("{")]
        if not lines:
            raise RuntimeError(f"no JSON in runtime-{index + 1}.log")
        result["runs"].append({**json.loads(lines[-1]), "load_average": load})
    result["summary"] = summarize_runtime([{k: v for k, v in run.items() if k != "load_average"} for run in result["runs"]])
    (output / "runtime.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path)
    parser.add_argument("--runtime", type=Path, metavar="EXE", help="Run obs_runtime_bench instead of timing a build")
    parser.add_argument("--seconds", type=float, default=10.0)
    parser.add_argument("--warmup", type=float, default=2.0)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--jobs", type=int, default=8)
    parser.add_argument("--rebuild", action="store_true", help="Trash this dedicated build's objects before each trial")
    parser.add_argument("--cache-stats", action="store_true", help="Reset/report ccache stats in the dedicated CCACHE_DIR")
    args = parser.parse_args()
    if args.repetitions < 1 or args.jobs < 1:
        parser.error("repetitions and jobs must be positive")
    if args.runtime:
        print(json.dumps(measure_runtime(args.runtime, args.output, args.repetitions, args.seconds, args.warmup)["summary"], indent=2))
        raise SystemExit(0)
    if not args.build:
        parser.error("--build or --runtime is required")
    print(json.dumps(measure(args.build, args.output, args.repetitions, args.jobs, args.rebuild, args.cache_stats), indent=2))
