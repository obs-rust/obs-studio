#!/usr/bin/env -S uv run --script
"""Record where a libobs build spends its time: the module-cache baseline (#3257).

    uv run rust/tools/benchmarks/libobs_build_phases.py --output out.json

The script configures a libobs-only tree (hybrid by default), builds it cold,
then for each edit case appends an inert change to one tracked file,
rebuilds, and restores the file byte for byte. Each sample records:

- wall and child CPU time of `cmake --build`;
- every entry the build appended to `.ninja_log`, attributed to its unity
  group (`libobs/build/libobs.<group>+.h.c`), the libobs link, or the Rust
  staticlib step (Corrosion/cargo);
- zccache's counters before and after, when its daemon reports them;
- the wall time of `soldr cargo build -p obs-c-oracle`, whose build.rs
  hashes `libobs/` and recompiles the oracle's C on any edit there.

The edit cases are the ones #3256's sub-issues have to improve: no change,
one `.c.h`, the shared `libobs.pch.h`, a ported-source-set `.json`, and a
Rust crate. Siblings rerun this script for their "after" numbers.
"""

from __future__ import annotations

import argparse
import contextlib
import datetime
import json
import os
import platform
import re
import resource
import shutil
import subprocess
import time
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]

CONFIGURE_FLAGS = [
    "-G",
    "Ninja",
    "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
    "-DOBS_BUILD_NUMBER=1",
    "-DOBS_VERSION_OVERRIDE=33.0.0",
    "-DENABLE_FRONTEND=OFF",
    "-DENABLE_SCRIPTING=OFF",
    "-DENABLE_BROWSER=OFF",
    "-DENABLE_WEBSOCKET=OFF",
    "-DENABLE_AJA=OFF",
    "-DENABLE_WEBRTC=OFF",
    "-DENABLE_NVENC=OFF",
    "-DENABLE_QSV11=OFF",
    "-DENABLE_VST=OFF",
    "-DENABLE_WAYLAND=OFF",
    "-DENABLE_UNIT_TESTS=OFF",
    "-DCMAKE_C_COMPILER_LAUNCHER=zccache",
    "-DCMAKE_CXX_COMPILER_LAUNCHER=zccache",
]

GROUP = re.compile(r"/libobs\.(?P<group>[^/+]+)\+\.h\.(?:c|cpp|m)\.o$")
LINK = re.compile(r"(^|/)(lib)?obs(\.so(\.\d+)*|\.dll|\.dylib|\.\d+\.dylib)$")
RUST = re.compile(r"(_rust|rust-static|cargo-build|corrosion)", re.IGNORECASE)

# Appended to a file of each kind; inert for the compiler and the JSON parser.
EDITS = {"c": b"\n/* libobs_build_phases probe */\n", "rust": b"\n// libobs_build_phases probe\n", "json": b"\n"}


@dataclass
class Entry:
    """One `.ninja_log` line: start and end in ms, mtime, output path."""

    start: str
    end: str
    mtime: str
    output: str

    @property
    def seconds(self) -> float:
        return (int(self.end) - int(self.start)) / 1000


@dataclass(frozen=True)
class Case:
    label: str
    path: str | None
    kind: str | None


def cases(root: Path) -> list[Case]:
    """The edit cases, each naming one tracked file (none for no-change)."""
    del root  # the paths are relative to it; the test checks they exist
    return [
        Case("no-change", None, None),
        Case("c-h", "libobs/obs.c.h", "c"),
        Case("pch", "libobs/build/libobs.pch.h", "c"),
        Case("ported-json", "libobs/build/ported-source-sets/audio-controls.json", "json"),
        Case("rust-crate", "rust/obs-util/src/lib.rs", "rust"),
    ]


@contextlib.contextmanager
def edited(path: Path, kind: str):
    """Append the inert edit for `kind`, then restore the original bytes."""
    original = path.read_bytes()
    path.write_bytes(original + EDITS[kind])
    try:
        yield
    finally:
        path.write_bytes(original)


def log_offset(log: Path) -> int:
    """The byte length of `.ninja_log` now; a build appends after it."""
    return log.stat().st_size if log.exists() else 0


def read_entries(log: Path, offset: int) -> list[Entry]:
    """The entries appended to `.ninja_log` since `offset`."""
    if not log.exists():
        return []
    with log.open("rb") as handle:
        handle.seek(offset)
        text = handle.read().decode("utf-8", "replace")
    entries = []
    for line in text.splitlines():
        if line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) >= 4:
            entries.append(Entry(*fields[:4]))
    return entries


def summarize(entries: list[Entry]) -> dict:
    """Seconds per unity group, link and Rust step, and the compile total."""
    groups: dict[str, float] = {}
    link = rust = compile_total = 0.0
    for entry in entries:
        match = GROUP.search(entry.output)
        if match:
            name = match.group("group")
            groups[name] = round(groups.get(name, 0.0) + entry.seconds, 3)
            compile_total += entry.seconds
        elif LINK.search(entry.output):
            link += entry.seconds
        elif RUST.search(entry.output):
            rust += entry.seconds
    return {
        "groups": groups,
        "compiled_groups": len(groups),
        "compile_seconds": round(compile_total, 3),
        "link_seconds": round(link, 3),
        "rust_seconds": round(rust, 3),
        "ninja_entries": len(entries),
    }


def zccache_counters() -> dict[str, float]:
    """zccache's numeric status fields, or {} when its daemon is not up."""
    try:
        done = subprocess.run(["zccache", "status", "--json"], capture_output=True, text=True, timeout=10)
        data = json.loads(done.stdout) if done.returncode == 0 else {}
    except (OSError, ValueError, subprocess.TimeoutExpired):
        return {}
    flat: dict[str, float] = {}

    def walk(prefix: str, value) -> None:
        if isinstance(value, dict):
            for key, inner in value.items():
                walk(f"{prefix}{key}.", inner)
        elif isinstance(value, (int, float)) and not isinstance(value, bool):
            flat[prefix.rstrip(".")] = value

    walk("", data)
    return flat


def timed(command: list[str], cwd: Path = ROOT) -> dict:
    """Wall and child CPU seconds of `command`, with its exit status."""
    before = resource.getrusage(resource.RUSAGE_CHILDREN)
    start = time.monotonic()
    done = subprocess.run(command, cwd=cwd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True)
    wall = time.monotonic() - start
    after = resource.getrusage(resource.RUSAGE_CHILDREN)
    cpu = (after.ru_utime - before.ru_utime) + (after.ru_stime - before.ru_stime)
    result = {"wall_seconds": round(wall, 3), "cpu_seconds": round(cpu, 3), "returncode": done.returncode}
    if done.returncode != 0:
        result["stderr_tail"] = done.stderr[-2000:]
    return result


def build(build_dir: Path, jobs: int | None) -> dict:
    """One timed libobs build with its ninja and zccache breakdown."""
    log = build_dir / ".ninja_log"
    offset = log_offset(log)
    cache_before = zccache_counters()
    command = ["cmake", "--build", str(build_dir), "--target", "libobs"]
    if jobs:
        command += ["--parallel", str(jobs)]
    result = timed(command)
    result.update(summarize(read_entries(log, offset)))
    cache_after = zccache_counters()
    result["zccache_delta"] = {
        key: cache_after[key] - cache_before.get(key, 0)
        for key in cache_after
        if cache_after[key] != cache_before.get(key, 0)
    }
    return result


def oracle_build() -> dict:
    """The oracle crate's build, which reruns build.rs on any libobs edit."""
    return timed(["soldr", "cargo", "build", "-p", "obs-c-oracle"])


def tool_version(command: list[str]) -> str:
    try:
        return subprocess.run(command, capture_output=True, text=True).stdout.splitlines()[0]
    except (OSError, IndexError):
        return "unknown"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--build-dir", type=Path, default=ROOT / "build_libobs_phases")
    parser.add_argument("--samples", type=int, default=3)
    parser.add_argument("--jobs", type=int, help="cmake --build --parallel (default: the generator's)")
    parser.add_argument("--fresh", action="store_true", help="delete the build dir first (a cold build)")
    parser.add_argument("--no-oracle", action="store_true", help="skip the obs-c-oracle build timing")
    parser.add_argument("--oracle-only", action="store_true",
                        help="time only the obs-c-oracle build per case (for a host without the libobs deps)")
    parser.add_argument("--case", action="append", default=[], help="run only these case labels (repeatable)")
    args = parser.parse_args()
    if args.samples < 1:
        parser.error("--samples must be at least 1")
    return args


def report_header(jobs: int | None) -> dict:
    load = list(os.getloadavg()) if hasattr(os, "getloadavg") else None
    revision = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=ROOT, capture_output=True, text=True)
    return {
        "issue": "#3257",
        "revision": revision.stdout.strip(),
        "timestamp": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "host": {"system": platform.system(), "machine": platform.machine(), "cpus": os.cpu_count(), "load_average": load},
        "tools": {
            "cmake": tool_version(["cmake", "--version"]),
            "cc": tool_version(["cc", "--version"]),
            "rustc": tool_version(["soldr", "rustc", "--version"]),
        },
        "configure_flags": CONFIGURE_FLAGS,
        "jobs": jobs,
    }


def quiet(command: list[str]) -> None:
    subprocess.run(command, cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def run_cases(selected: list[Case], samples: int, measure, restore) -> dict:
    """`measure()` with each case's edit applied, `samples` times per case;
    `restore()` rebuilds the reverted tree untimed between samples."""
    out = {}
    for case in selected:
        rows = []
        for sample in range(samples):
            print(f"{case.label} {sample + 1}/{samples}", flush=True)
            context = edited(ROOT / case.path, case.kind) if case.path else contextlib.nullcontext()
            with context:
                rows.append(measure())
            restore()
        out[case.label] = {"path": case.path, "samples": rows}
    return out


def write(report: dict, output: Path) -> None:
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {output}", flush=True)


def main() -> None:
    args = parse_args()
    if args.fresh and args.build_dir.exists():
        shutil.rmtree(args.build_dir)
    report = report_header(args.jobs)
    selected = [case for case in cases(ROOT) if not args.case or case.label in args.case]
    oracle_restore = ["soldr", "cargo", "build", "-p", "obs-c-oracle"]
    if args.oracle_only:
        report["initial_oracle"] = oracle_build()
        report["cases"] = run_cases(selected, args.samples, lambda: {"oracle": oracle_build()}, lambda: quiet(oracle_restore))
        write(report, args.output)
        return
    print("configure", flush=True)
    report["configure"] = timed(["cmake", "-S", str(ROOT), "-B", str(args.build_dir), *CONFIGURE_FLAGS])
    if report["configure"]["returncode"] != 0:
        write(report, args.output)
        raise SystemExit("configure failed; see stderr_tail in the report")
    print("initial build", flush=True)
    report["initial_build"] = build(args.build_dir, args.jobs)
    if not args.no_oracle:
        report["initial_oracle"] = oracle_build()

    def measure() -> dict:
        row = {"build": build(args.build_dir, args.jobs)}
        if not args.no_oracle:
            row["oracle"] = oracle_build()
        return row

    def restore() -> None:
        quiet(["cmake", "--build", str(args.build_dir), "--target", "libobs"])
        if not args.no_oracle:
            quiet(oracle_restore)

    report["cases"] = run_cases(selected, args.samples, measure, restore)
    report["host"]["load_average_end"] = list(os.getloadavg()) if hasattr(os, "getloadavg") else None
    write(report, args.output)

if __name__ == "__main__":
    main()
