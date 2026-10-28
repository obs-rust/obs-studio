#!/usr/bin/env -S uv run --script
"""Run Tier 2's RUN_SERIAL tests beside the suite, on their own CPUs (#2773).

    uv run --no-project serial-runner.py run <registry.json> --cpus 0-1,2-3
        [--log-dir DIR] [--timeout SECONDS] [--xvfb ARGS] [--env NAME=VALUE ...]
    uv run --no-project serial-runner.py suite-cpus --serial 0-1,2-3
    uv run --no-project serial-runner.py pin --cpus 4-23 [--env NAME=VALUE ...] -- CMD ...

Inside one Tier 2 container the five test_deinterlace_2x_field_order tests are
RUN_SERIAL (#1343, #1643): ctest runs them one at a time while every other core
idles, which on a large host is most of the lane (measured in #2773). CI gives
them a runner of their own (#2071, #2072); a local host cannot, so
OBS_TIER2_SERIAL_CPUS gives them their own *cores* instead.

`run` reads the CTest registry (`ctest --show-only=json-v1`), takes the tests
that carry the RUN_SERIAL property, and starts each one as its own process on
its own CPU pair. Deliberately not `ctest -R`: RUN_SERIAL keeps those tests
one-at-a-time even under -j, and a second concurrent ctest would race the
suite part's Testing/Temporary/CTestCostData.txt.

`suite-cpus` prints this process's affinity minus the serial set, the CPU list
the suite part is pinned to, so the two parts never share a core.

`pin` execs a command on a CPU set; validate.sh uses it for the suite part so
the harness needs no `taskset`.

Exit status: 0 when every test passed, 1 when one failed, 2 on a usage error.
"""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path


def parse_cpus(text: str) -> list[int]:
    """`0-3,8,10-11` as a sorted CPU list; ValueError on anything else."""
    cpus: set[int] = set()
    for part in text.split(","):
        if not part.isdigit() and "-" not in part:
            raise ValueError(f"not a CPU list: '{text}'")
        first, _, last = part.partition("-")
        if not first.isdigit() or (last and not last.isdigit()):
            raise ValueError(f"not a CPU list: '{text}'")
        start, stop = int(first), int(last or first)
        if stop < start:
            raise ValueError(f"reversed CPU range in '{text}'")
        cpus.update(range(start, stop + 1))
    if not cpus:
        raise ValueError(f"empty CPU list: '{text}'")
    return sorted(cpus)


def format_cpus(cpus: list[int]) -> str:
    """The inverse of parse_cpus: runs of consecutive CPUs become ranges."""
    parts: list[str] = []
    index = 0
    while index < len(cpus):
        end = index
        while end + 1 < len(cpus) and cpus[end + 1] == cpus[end] + 1:
            end += 1
        parts.append(str(cpus[index]) if end == index else f"{cpus[index]}-{cpus[end]}")
        index = end + 1
    return ",".join(parts)


def cpu_pairs(serial_cpus: list[int], count: int) -> list[list[int]]:
    """Split the serial CPUs into `count` consecutive pairs, in order."""
    if len(serial_cpus) < 2 * count:
        raise ValueError(
            f"{count} serial test(s) need {2 * count} CPUs, OBS_TIER2_SERIAL_CPUS gives {len(serial_cpus)}"
        )
    return [serial_cpus[2 * index : 2 * index + 2] for index in range(count)]


def properties(test: dict) -> dict:
    """A CTest registry test's properties by name (ENVIRONMENT, TIMEOUT, ...)."""
    return {item.get("name", ""): item.get("value") for item in test.get("properties", [])}


def start_test(
    test: dict, pair: list[int], args: argparse.Namespace, log_dir: Path | None
) -> tuple[str, subprocess.Popen, Path | None, float, object]:
    """Start one RUN_SERIAL test pinned to `pair`, its output in its own log."""
    name = test.get("name", "test")
    props = properties(test)
    command = [*test.get("command", [])]
    if not command:
        raise ValueError(f"[{name}] the registry has no command")
    if args.xvfb:
        command = ["xvfb-run", *shlex.split(args.xvfb), *command]
    environment = dict(os.environ)
    # ENVIRONMENT is a test property, not a top-level key: the deinterlace
    # entries select their rate with CMOCKA_TEST_FILTER (#2072).
    for item in [*props.get("ENVIRONMENT", []), *args.env]:
        key, _, value = item.partition("=")
        environment[key] = value
    handle = (log_dir / f"{name}.log").open("wb") if log_dir else None
    process = subprocess.Popen(
        command,
        cwd=props.get("WORKING_DIRECTORY") or None,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=handle or subprocess.DEVNULL,
        stderr=subprocess.STDOUT,
        start_new_session=True,
        preexec_fn=lambda cpus=set(pair): os.sched_setaffinity(0, cpus),
    )
    print(f"== [serial] {name}: started on CPUs {format_cpus(pair)}", flush=True)
    path = log_dir / f"{name}.log" if log_dir else None
    return name, process, path, time.monotonic(), handle


def collect(started: list[tuple[str, subprocess.Popen, Path | None, float, object]], timeout: float) -> int:
    """Wait for every started test, print a verdict each; the number that failed."""
    failures = 0
    for name, process, path, since, handle in started:
        try:
            status = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, 9)
            process.wait()
            status = -9
        if handle:
            handle.close()
        elapsed = time.monotonic() - since
        if status == 0:
            print(f"== [serial] {name}: PASS ({elapsed:.1f} s)", flush=True)
            continue
        failures += 1
        reason = "timeout" if status == -9 else f"exit {status}"
        print(f"== [serial] {name}: FAIL ({reason}, {elapsed:.1f} s)", flush=True)
        if path:
            print(f"== [serial] {name}: log {path}", flush=True)
            print("\n".join(path.read_text(encoding="utf-8", errors="replace").splitlines()[-40:]), flush=True)
    print(f"== [serial] {len(started) - failures}/{len(started)} passed", flush=True)
    return failures


def run_serial_tests(tests: list[dict], args: argparse.Namespace) -> int:
    """Start every test, then collect them; 1 if any failed."""
    pairs = cpu_pairs(parse_cpus(args.cpus), len(tests))
    log_dir = Path(args.log_dir) if args.log_dir else None
    if log_dir:
        log_dir.mkdir(parents=True, exist_ok=True)
    print(f"== [serial] {len(tests)} RUN_SERIAL test(s) on {format_cpus(parse_cpus(args.cpus))}", flush=True)
    started = [start_test(test, pair, args, log_dir) for test, pair in zip(tests, pairs, strict=True)]
    failures = collect(started, args.timeout)
    return 1 if failures else 0


def suite_cpus(serial: str) -> list[int]:
    """This process's affinity minus the serial set; the suite part's CPUs."""
    affinity = set(os.sched_getaffinity(0))
    wanted = set(parse_cpus(serial))
    outside = sorted(wanted - affinity)
    if outside:
        print(f"WARNING: OBS_TIER2_SERIAL_CPUS names CPUs outside this host's affinity: {format_cpus(outside)}", file=sys.stderr)
    remaining = sorted(affinity - wanted)
    if not remaining:
        raise ValueError(f"OBS_TIER2_SERIAL_CPUS={serial} leaves the suite part no CPU")
    return remaining


def pinned(args: argparse.Namespace) -> int:
    """Exec a command on a CPU set, with the given environment."""
    environment = dict(os.environ)
    for item in args.env:
        key, _, value = item.partition("=")
        environment[key] = value
    try:
        os.sched_setaffinity(0, set(parse_cpus(args.cpus)))
    except (OSError, ValueError) as error:
        print(f"FAIL: cannot pin to CPUs {args.cpus}: {error}", file=sys.stderr)
        return 2
    try:
        os.execvpe(args.command[0], args.command, environment)
    except OSError as error:
        print(f"FAIL: cannot run {args.command[0]}: {error}", file=sys.stderr)
        return 2


def serial_tests(registry: dict) -> list[dict]:
    return [
        test
        for test in registry.get("tests", [])
        if any(item.get("name") == "RUN_SERIAL" and item.get("value") for item in test.get("properties", []))
    ]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    commands = parser.add_subparsers(dest="subcommand", required=True)

    run = commands.add_parser("run", help="run the RUN_SERIAL tests, one process per CPU pair")
    run.add_argument("registry", type=Path)
    run.add_argument("--cpus", required=True, help="the serial part's CPUs, e.g. 0-1,2-3")
    run.add_argument("--log-dir", help="keep each test's output in <dir>/<test>.log")
    run.add_argument("--timeout", type=float, default=900.0, help="seconds per test (default 900)")
    run.add_argument("--xvfb", help="wrap each command in `xvfb-run <ARGS>`")
    run.add_argument("--env", action="append", default=[], help="NAME=VALUE for the children")

    suite = commands.add_parser("suite-cpus", help="print the affinity minus the serial CPUs")
    suite.add_argument("--serial", required=True)

    pin = commands.add_parser("pin", help="exec a command on a CPU set")
    pin.add_argument("--cpus", required=True)
    pin.add_argument("--env", action="append", default=[])
    pin.add_argument("command", nargs=argparse.REMAINDER)

    args = parser.parse_args(argv)
    try:
        if args.subcommand == "suite-cpus":
            print(format_cpus(suite_cpus(args.serial)))
            return 0
        if args.subcommand == "pin":
            if not args.command or args.command[0] == "--":
                args.command = args.command[1:]
            if not args.command:
                print("FAIL: pin needs a command after --", file=sys.stderr)
                return 2
            return pinned(args)
        registry = json.loads(args.registry.read_text(encoding="utf-8"))
        return run_serial_tests(serial_tests(registry), args)
    except ValueError as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
