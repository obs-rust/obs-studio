#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Run the cross-built Windows Tier 2 cmocka executables under Wine on Linux.

Input is the tree ci/cmocka_link_check.py --all-tests builds (hybrid obs.dll
plus every unchanged cmocka executable). CTest only lists the tests: this
script runs each listed `.exe` through Wine, headless, so the merge queue needs
no Windows runner (obs-rust/obs-studio#5844, #5843). The C-reference baseline
and the quarantined tests below stay native in the nightly
`rust-tier2-windows` job.

Wine never touches a desktop: DISPLAY and WAYLAND_DISPLAY are unset for every
call, the crash debugger and menu builder are disabled, and the prefix is
created non-interactively.
"""
from __future__ import annotations

import argparse
import json
import os
import shlex
import shutil
import signal
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Tests that fail for a reason that is not a Rust/libobs defect, with the
# reason. They keep running natively in the nightly rust-tier2-windows job.
# Quarantine only a genuine Wine or cross-toolchain gap, never a flaky test.
QUARANTINE: dict[str, str] = {
    "test_disk_space_monitor": (
        "All four cases time out waiting on the monitor's worker thread (70 s) on the CI runner under "
        "Wine, three runs in a row; the same executable passes under Wine 11 on an idle workstation. "
        "A thread-scheduling gap under Wine on the runner; root cause not yet isolated."
    ),
    "test_platform": (
        "test_mbs_unconvertible_is_empty: Wine's ucrtbase wcstombs converts U+4E2D in the C locale "
        "(returns 2) where Windows' UCRT reports it unconvertible (0). A Wine locale gap."
    ),
    "test_platform_conversions": (
        "test_wcs_to_mbs_c_locale: same Wine ucrtbase C-locale wcstombs difference as test_platform."
    ),
    "test_scene_audio_window_steps_rust": (
        "window_u64_edges: the test picks its expectation with "
        "`_MSC_VER && !__clang__`; the cross build compiles it with clang-cl, so it expects the "
        "wrapping util_mul_div64 while obs.dll uses MSVC-style exact 128-bit math. "
        "A cross-toolchain difference, not a Wine gap, and the C test must not be edited."
    ),
}


def wine_env(prefix: Path) -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if key not in ("DISPLAY", "WAYLAND_DISPLAY")}
    env.update(
        WINEPREFIX=str(prefix),
        WINEDEBUG="-all",
        WINEARCH="win64",
        WINEDLLOVERRIDES="mscoree,mshtml=;winedbg.exe=d;winemenubuilder.exe=d",
    )
    return env


def wine_command() -> list[str]:
    """`$WINE`, else ci/wine-runner.sh, the headless Wine entry point shared with
    the Windows Rust tests (#5845): it creates the prefix with the crash dialog
    and debugger off. `$WINE` may be a wrapper such as `nix shell ... -c wine`.
    """
    return shlex.split(os.environ["WINE"]) if os.environ.get("WINE") else [str(ROOT / "ci" / "wine-runner.sh")]


def wineserver(prefix: Path, *flags: str) -> None:
    """Best-effort wineserver control, with no inherited pipes.

    `-p` keeps the server alive for the run. Distro layouts differ (Ubuntu's
    wine64 has no `wineserver` on PATH), so a missing binary is not an error.
    """
    wine = wine_command()
    found = os.environ.get("WINESERVER") or shutil.which("wineserver") or next(
        (str(c) for c in (Path(wine[-1]).with_name("wineserver"), Path("/usr/lib/wine/wineserver64"),
                          Path("/usr/lib/wine/wineserver")) if c.is_file()), None)
    if found is None:
        return
    prefix_env = wine_env(prefix)
    subprocess.run([*wine[:-1], found, *flags] if wine[-1].endswith("wine") else [found, *flags], env=prefix_env,
                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                   check=False, timeout=120)


def plan(build: Path) -> list[dict]:
    out = subprocess.run(["ctest", "--test-dir", str(build), "--show-only=json-v1"],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out)["tests"]


def run_test(test: dict, build: Path, env: dict[str, str], timeout: int) -> tuple[str, str, float, str]:
    props = {p["name"]: p["value"] for p in test.get("properties", [])}
    command = test["command"]
    cwd = props.get("WORKING_DIRECTORY") or str(Path(command[0]).parent)
    run_env = dict(env)
    for item in props.get("ENVIRONMENT", []):
        key, _, value = item.partition("=")
        if key != "PATH":
            run_env[key] = value
    started = time.monotonic()
    # Own process group: a hung test leaves wine's helper processes behind.
    with subprocess.Popen([*wine_command(), *command], cwd=cwd, env=run_env, stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, errors="replace", start_new_session=True) as proc:
        try:
            output, _ = proc.communicate(timeout=timeout)
            code = proc.returncode
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            output, _ = proc.communicate()
            code, output = 124, f"timeout after {timeout}s\n{output or ''}"
    if "SKIP_RETURN_CODE" in props and code == int(props["SKIP_RETURN_CODE"]):
        return test["name"], "skip", time.monotonic() - started, output
    passed = (code != 0) if props.get("WILL_FAIL") else (code == 0)
    return test["name"], "pass" if passed else "fail", time.monotonic() - started, output


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, default=ROOT / "build_cmocka_link_win-x64")
    parser.add_argument("--deps", type=Path, help="obs-deps bin directory holding the FFmpeg DLLs")
    parser.add_argument("--prefix", type=Path, default=ROOT / ".wine-tier2")
    parser.add_argument("--jobs", type=int, default=min(4, os.process_cpu_count() or 1))
    parser.add_argument("--timeout", type=int, default=90)
    parser.add_argument("--include-quarantined", action="store_true")
    args = parser.parse_args()
    args.deps = args.deps or next(iter(sorted((ROOT / ".deps").glob("obs-deps-*-x64/bin"))), None)
    if args.deps is None:
        parser.error("no obs-deps bin directory under .deps; pass --deps")
    return args


def start_wine(prefix: Path, build: Path, deps: Path) -> dict[str, str]:
    wineserver(prefix, "-p")
    env = wine_env(prefix)
    # The runner creates the prefix on its first call. Make that call now, with
    # no pipes: the first client otherwise starts services.exe and winedevice
    # holding its output pipe open, and its test would never see EOF.
    subprocess.run([*wine_command(), "wineboot", "-u"], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                   stderr=subprocess.DEVNULL, check=False, timeout=300)
    # obs.dll and w32-pthreads.dll sit in the build root, FFmpeg in the deps.
    env["WINEPATH"] = f"Z:{build};Z:{deps.resolve()}"
    return env


def test_exe(part: str, build: Path) -> Path | None:
    """The Windows executable a CTest command part names, if it is one of ours.

    CTest on Windows finds `.../test_x` as `.../test_x.exe`; registrations written
    without the suffix (test_audio_io, test_video_io) mean the same.
    """
    path = Path(part)
    # xvfb-run-or-display.sh sits in the build tree too; it is not a test.
    if not path.is_relative_to(build) or path.suffix not in ("", ".exe"):
        return None
    return path if path.suffix == ".exe" else path.with_name(path.name + ".exe")


def runnable_tests(build: Path) -> list[dict] | None:
    """CTest's Windows executables, or None when one was not cross-built."""
    tests: list[dict] = []
    unbuilt: list[str] = []
    for test in plan(build):
        # CTest drops the command of a test whose executable it cannot find.
        command = test.get("command")
        if not command:
            # Registered as `.../test_x` with no suffix: CTest cannot resolve it
            # on Linux, where Windows' CTest would find test_x.exe.
            guess = build / "test" / "cmocka" / f"{test['name']}.exe"
            if guess.exists():
                test["command"] = [str(guess)]
                tests.append(test)
            else:
                unbuilt.append(test["name"])
            continue
        # The executable, possibly behind a CMAKE_CROSSCOMPILING_EMULATOR or an
        # xvfb-run wrapper (the display tests, when CI has xvfb); CMake script tests (no executable under build) run
        # on the Linux lanes.
        found = [(i, exe) for i, part in enumerate(command) if (exe := test_exe(part, build))]
        if not found:
            continue
        index, exe = found[0]
        if not exe.exists():
            unbuilt.append(test["name"])
            continue
        test["command"] = [str(exe), *command[index + 1:]]
        tests.append(test)
    if unbuilt:
        print(f"ERROR: tests that were not cross-built: {sorted(unbuilt)}")
        return None
    return tests


def run_all(selected: list[dict], build: Path, env: dict[str, str], args: argparse.Namespace) -> tuple[list[str], list[str]]:
    """Run once in parallel (RUN_SERIAL tests alone), then retry failures serially."""
    failures: list[dict] = []

    def serial(test: dict) -> bool:
        return any(p["name"] == "RUN_SERIAL" and p["value"] for p in test.get("properties", []))

    def record(test: dict, result: tuple[str, str, float, str]) -> None:
        name, status, seconds, _ = result
        print(f"{status.upper():4} {name} ({seconds:.1f}s)", flush=True)
        if status == "fail":
            failures.append(test)

    parallel = [t for t in selected if not serial(t)]
    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        for test, result in zip(parallel, pool.map(lambda t: run_test(t, build, env, args.timeout), parallel)):
            record(test, result)
    for test in (t for t in selected if serial(t)):
        record(test, run_test(test, build, env, args.timeout))
    # Wine under load occasionally kills or hangs a process that passes alone
    # (observed: test_axisang, test_cubetexture_access). Two serial reruns
    # separate that from a real failure; a test must fail three times to fail the lane.
    failed: list[str] = []
    flaky: list[str] = []
    for test in failures:
        for attempt in (1, 2):
            name, status, seconds, output = run_test(test, build, env, args.timeout)
            print(f"RETRY {attempt} {status.upper():4} {name} ({seconds:.1f}s)", flush=True)
            if status != "fail":
                flaky.append(name)
                break
        else:
            failed.append(name)
            print(output[-4000:])
    if flaky:
        print(f"Recovered on serial retry: {', '.join(flaky)}")
    return failed, flaky


def main() -> int:
    args = parse_args()
    build = args.build.resolve()
    prefix = args.prefix.resolve()
    env = start_wine(prefix, build, args.deps)
    tests = runnable_tests(build)
    if tests is None:
        return 1
    held = {t["name"] for t in tests if t["name"] in QUARANTINE and not args.include_quarantined}
    selected = [t for t in tests if t["name"] not in held]
    print(f"Tier 2 Wine: {len(selected)} tests, {len(held)} quarantined, {args.jobs} workers", flush=True)
    for name in sorted(held):
        print(f"  quarantined {name}: {QUARANTINE[name]}")
    failed, flaky = run_all(selected, build, env, args)
    print(f"Tier 2 Wine: {len(selected) - len(failed)} passed ({len(flaky)} after retry), {len(failed)} failed, {len(held)} quarantined")
    wineserver(prefix, "-k")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
