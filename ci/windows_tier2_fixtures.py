#!/usr/bin/env -S uv run --script
"""Run the unchanged compile-only stale-manifest assertion on Linux, then seal it.

This test never executed its fixture EXE on Windows. Its configure and link
assertions belong on the producer, using the same release MSVC target profile.
"""

import argparse
import json
from pathlib import Path
import subprocess
import time

from windows_tier2_probe import digest

INPUTS = (
    "test/cmocka/check_core_module_manifest_source.cmake",
    "libobs/cmake/obs-core-modules.cmake",
)


def prepare(root: Path, build: Path, output: Path, source_sha: str) -> dict:
    command = [
        "cmake",
        f"-DOBS_CORE_MODULES_CMAKE={root / INPUTS[1]}",
        f"-DTEST_FIXTURE_ROOT={build / 'prepared-stale-manifest'}",
        "-DTEST_GENERATOR=Ninja",
        "-DTEST_BUILD_CONFIG=RelWithDebInfo",
        f"-DTEST_TOOLCHAIN_FILE={build / 'tier2-fixture-toolchain.cmake'}",
        "-P",
        str(root / INPUTS[0]),
    ]
    started = time.monotonic()
    subprocess.run(command, check=True)
    receipt = {
        "schema": 1,
        "test": "test_core_module_manifest_source",
        "source_sha": source_sha,
        "target": "x86_64-pc-windows-msvc",
        "profile": "RelWithDebInfo",
        "exit_code": 0,
        "inputs": {name: digest(root / name) for name in INPUTS},
        "toolchain_sha256": digest(build / "tier2-fixture-toolchain.cmake"),
        "command": command,
        "seconds": round(time.monotonic() - started, 3),
    }
    output.write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--source-sha", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    actual = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=root, text=True
    ).strip()
    if args.source_sha != actual:
        raise ValueError("fixture source SHA differs from checkout")
    prepare(root, args.build.resolve(), args.output.resolve(), args.source_sha)


if __name__ == "__main__":
    main()
