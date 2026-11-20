#!/usr/bin/env -S uv run --script
"""Overlap independent macOS Tier 2 work on one runner (#5282).

CTest's four passes share WorkingDirectory, fixtures, timing locks and
Testing/Temporary. Keep them serial. Cargo preparation only reads CMake's
artifacts and writes cargo-native-{rust,tests,core}, so it can overlap the
first two passes. Its Cargo invocations retain their existing target dirs
and run sequentially; no Cargo build lock or duplicate compilation is added.

#5009 can reuse this boundary with its cross-build preparation command.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path


def run_phase(name: str, command: list[str], log_dir: Path) -> int:
    with (log_dir / f"{name}.log").open("w", encoding="utf-8") as log:
        start = time.monotonic()
        try:
            status = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=False).returncode
        except OSError as error:
            print(f"Unable to launch {name}: {error}", file=log)
            status = 127
        print(f"\n{name}: exit {status}, elapsed {time.monotonic() - start:.1f}s", file=log)
    return status


def show_log(name: str, log_dir: Path) -> None:
    print(f"::group::macOS Tier 2 {name}", flush=True)
    with (log_dir / f"{name}.log").open(encoding="utf-8", errors="replace") as log:
        shutil.copyfileobj(log, sys.stdout)
    print("::endgroup::", flush=True)


def run_tail(validator: Path, arguments: list[str], log_dir: Path) -> int:
    log_dir.mkdir(parents=True, exist_ok=True)
    results = {}

    def command(name: str) -> list[str]:
        return ["bash", str(validator), "--phase", name, *arguments]

    # Executor shutdown joins every child even when an independent phase fails.
    # Only the main thread prints completed logs, so groups cannot interleave.
    with ThreadPoolExecutor(max_workers=2) as pool:
        pending = {pool.submit(run_phase, name, command(name), log_dir): name
                   for name in ("cmake-tests", "cargo-build")}
        for future in as_completed(pending):
            name = pending[future]
            results[name] = future.result()
            show_log(name, log_dir)

    if results["cargo-build"] == 0:
        results["cargo-tests"] = run_phase("cargo-tests", command("cargo-tests"), log_dir)
        show_log("cargo-tests", log_dir)
    else:
        print("Skipping cargo-tests: cargo-build failed", flush=True)
    return int(any(status != 0 for status in results.values()))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--validator", required=True, type=Path)
    parser.add_argument("--log-dir", required=True, type=Path)
    parser.add_argument("--repeat", default="0")
    parser.add_argument("--tests", default="")
    args = parser.parse_args()
    return run_tail(args.validator, ["--repeat", args.repeat, "--tests", args.tests], args.log_dir)


if __name__ == "__main__":
    raise SystemExit(main())
