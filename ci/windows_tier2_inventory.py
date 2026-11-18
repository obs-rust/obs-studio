#!/usr/bin/env -S uv run --script
"""Compare the producer registry with independent hosted Windows CTest names.

The log snapshot is a lower-bound coverage diagnostic, not property parity or
same-commit native qualification. All missing records remain blockers.
"""

import argparse
import json
from pathlib import Path
import subprocess


def inventory(build: Path, native: dict) -> dict:
    output = subprocess.check_output(
        [
            "ctest",
            "--test-dir",
            str(build),
            "-C",
            "RelWithDebInfo",
            "--show-only=json-v1",
        ],
        text=True,
    )
    tests = json.loads(output)["tests"]
    names = {test["name"] for test in tests}
    baseline = set(native["names"])
    return {
        "complete": False,
        "coverage": "unqualified-full-registry",
        "native_revision": native["revision"],
        "native_job": native["job"],
        "native_count": len(baseline),
        "producer_count": len(tests),
        "missing_native_tests": sorted(baseline - names),
        "additional_tests": sorted(names - baseline),
        "unbuilt_tests": sorted(t["name"] for t in tests if not t.get("command")),
        "runtime_build_tests": [
            t
            for t in tests
            if t.get("command") and Path(t["command"][0]).suffix.lower() != ".exe"
        ],
        "tests": tests,
        "qualification_blockers": [
            "independent same-commit native commands/properties",
            "Cargo core/native contract Linux preparation",
            "runtime fixture prepare/assert split",
            "four complete hosted Windows passes with compilers unavailable",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument(
        "--native",
        type=Path,
        default=Path(__file__).parent / "windows-tier2/native-test-names.json",
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = inventory(args.build, json.loads(args.native.read_text()))
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(
        json.dumps(
            {
                key: value
                for key, value in result.items()
                if key not in {"tests", "runtime_build_tests"}
            },
            indent=2,
        )
    )
    return int(bool(result["missing_native_tests"] or result["unbuilt_tests"]))


if __name__ == "__main__":
    raise SystemExit(main())
