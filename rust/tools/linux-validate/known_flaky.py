#!/usr/bin/env -S uv run --script
"""Run CTest with one audited retry for issue-linked known flakes (Refs #4944)."""

from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from xml.etree import ElementTree as ET


def load_registry(path: Path) -> list[tuple[re.Pattern[str], str]]:
    """Reject every incomplete entry, even when the normal suite passes."""
    entries = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        match = re.fullmatch(r"(\S+)\s+#([1-9][0-9]*)\s+(\S.*)", line)
        try:
            if match is None:
                raise ValueError("expected <ctest name regex> #<open issue number> <reason>")
            entries.append((re.compile(match[1]), match[2]))
        except (ValueError, re.error) as exc:
            raise ValueError(f"{path}:{number}: {exc}") from exc
    return entries


def results(path: Path) -> dict[str, bool | None]:
    """Read CTest's own results: True passed, False failed, None skipped."""
    root = ET.parse(path).getroot()
    if root.tag != "testsuite":
        raise ValueError("expected a CTest JUnit testsuite")
    tests = {}
    for case in root.findall("testcase"):
        name = case.attrib["name"]
        if not name or name in tests:
            raise ValueError("empty or duplicate CTest test name")
        failed = case.find("failure") is not None or case.find("error") is not None
        status = case.get("status")
        if failed or status == "fail":
            tests[name] = False
        elif status == "disabled":
            tests[name] = None
        elif case.find("skipped") is not None or status == "notrun":
            skipped = case.find("skipped")
            reason = skipped.get("message", "") if skipped is not None else ""
            # CTest also uses <skipped> for failed Not Run cases (missing
            # executables/dependencies). Only explicit skip policies are benign.
            if not re.fullmatch(r"SKIP_RETURN_CODE=-?[0-9]+|SKIP_REGULAR_EXPRESSION_MATCHED|Disabled", reason):
                raise ValueError(f"unclassified Not Run result for {name}: {reason}")
            tests[name] = None
        elif status == "run":
            tests[name] = True
        else:
            raise ValueError(f"unknown CTest status for {name}: {status}")
    return tests


def run(command: list[str], build: Path, registry: list[tuple[re.Pattern[str], str]], mode: str) -> int:
    # Fresh reports prevent an OFF/ON, shard or reused-tree failure from being
    # mistaken for this invocation. Keep the normal CTest output in the caller's log.
    with tempfile.TemporaryDirectory(prefix="known-flaky-", dir=build) as directory:
        report = Path(directory) / "initial.xml"
        status = subprocess.run([*command, "--output-junit", str(report)], check=False).returncode
        if status != 8:  # CTest test failures; usage/infrastructure errors never retry.
            return status
        try:
            failures = sorted(name for name, passed in results(report).items() if passed is False)
        except (OSError, ET.ParseError, ValueError, KeyError) as exc:
            print(f"known-flaky: cannot establish failed tests: {exc}", file=sys.stderr)
            return status
        issues = {name: next((issue for pattern, issue in registry if pattern.fullmatch(name)), None)
                  for name in failures}
        if not failures or not all(issues.values()):
            return status
        print("== known-flaky: retrying once: " + ", ".join(f"{name} (#{issues[name]})" for name in failures),
              flush=True)
        report = Path(directory) / "retry.xml"
        retry_status = subprocess.run(
            [*command, "--rerun-failed", "--no-tests=error", "--output-junit", str(report)],
            check=False,
        ).returncode
        if retry_status:
            return retry_status
        try:
            retried = results(report)
            if set(retried) != set(failures) or not all(value is True for value in retried.values()):
                raise ValueError("retry must pass every originally failed test, without skips")
        except (OSError, ET.ParseError, ValueError, KeyError) as exc:
            print(f"known-flaky: cannot establish retry success: {exc}", file=sys.stderr)
            return status
        for name in failures:
            message = f"known-flaky {name} failed then passed on retry (#{issues[name]})"
            print(f"::warning::{message}", flush=True)
            if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
                with Path(summary).open("a", encoding="utf-8") as stream:
                    stream.write(f"- {message} (Tier 2 {mode})\n")
        return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--registry", required=True, type=Path)
    parser.add_argument("--build", required=True, type=Path)
    parser.add_argument("--mode", required=True, choices=("OFF", "ON"))
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if not args.command or args.command[0] != "--" or len(args.command) < 2:
        parser.error("expected -- <CTest command, including environment/display wrappers>")
    try:
        registry = load_registry(args.registry)
    except (OSError, ValueError) as exc:
        parser.error(str(exc))
    return run(args.command[1:], args.build, registry, args.mode)


if __name__ == "__main__":
    raise SystemExit(main())
