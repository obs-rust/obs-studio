#!/usr/bin/env -S uv run --script
"""Measure a baseline and candidate sequentially in one native checkout (#3004).

Usage: uv run rust/tools/benchmarks/compare_format.py --base FULL_SHA --output DIR
The output must be outside the checkout, so switching revisions cannot lose it.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CASES = {f"format/uyvx_to_{kind}/1080p" for kind in ("i420", "nv12", "i444")}


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def validate_base(root: Path, base: str) -> str:
    """Only a complete ancestor commit is accepted, never an option or rev expression."""
    if not re.fullmatch(r"[0-9a-f]{40}", base):
        raise ValueError("--base must be a full lowercase 40-character commit SHA")
    candidate = git(root, "rev-parse", "HEAD")
    if base == candidate:
        raise ValueError("baseline and candidate must differ")
    if git(root, "rev-parse", "--verify", f"{base}^{{commit}}") != base:
        raise ValueError("baseline must name a commit object")
    subprocess.run(["git", "merge-base", "--is-ancestor", base, candidate], cwd=root, check=True)
    if git(root, "status", "--porcelain"):
        raise ValueError("benchmark checkout must be clean")
    return candidate


def measure(root: Path, output: Path, label: str) -> None:
    """Keep complete command output even when compilation or the probe fails."""
    with (output / f"{label}.log").open("w") as log:
        subprocess.run(
            ["uv", "run", "--no-project", "rust/tools/benchmarks/record.py",
             "--category", "format", "--repeats", "3", "--output", str(output / f"{label}.json")],
            cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True,
        )


def compare(root: Path, base: str, output: Path) -> None:
    candidate = validate_base(root, base)
    output = output.resolve()
    if output == root or root in output.parents:
        raise ValueError("--output must be outside the checkout")
    output.mkdir(parents=True, exist_ok=True)
    manifest = {"baseline": base, "candidate": candidate, "status": "running"}
    manifest_path = output / "comparison.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
    try:
        subprocess.run(["git", "switch", "--detach", base], cwd=root, check=True)
        measure(root, output, "baseline")
        subprocess.run(["git", "switch", "--detach", candidate], cwd=root, check=True)
        with (output / "candidate-parity.log").open("w") as log:
            subprocess.run(
                ["soldr", "cargo", "test", "-p", "obs-media-io", "--lib", "format_conversion", "--", "--nocapture"],
                cwd=root, stdout=log, stderr=subprocess.STDOUT, check=True,
            )
        measure(root, output, "candidate")
        reports = {label: json.loads((output / f"{label}.json").read_text()) for label in ("baseline", "candidate")}
        manifest["results"] = {
            label: [row for row in report["results"] if row["benchmark"] in CASES]
            for label, report in reports.items()
        }
        for label, rows in manifest["results"].items():
            if {row["benchmark"] for row in rows} != CASES:
                raise ValueError(f"{label}: missing UYVX compression measurement")
        manifest["status"] = "measured"
        print(json.dumps(manifest, indent=2))
    except BaseException:
        manifest["status"] = "failed"
        raise
    finally:
        # Retain logs/metadata on failure, and leave the runner at its candidate.
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        subprocess.run(["git", "switch", "--detach", candidate], cwd=root, check=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    # Inherited porcelain suppresses test/benchmark diagnostics; cached verdicts
    # or compiler output must never stand in for a fresh native measurement.
    os.environ.pop("SOLDR_PORCELAIN", None)
    os.environ["SOLDR_RUSTC_WRAPPER"] = "none"
    os.environ["SOLDR_VERDICTS"] = "off"
    compare(ROOT, args.base, args.output)


if __name__ == "__main__":
    main()
