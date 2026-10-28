#!/usr/bin/env -S uv run --script
"""Checks the wrappers that start a private PipeWire daemon for the PipeWire
and PulseAudio tests (test_pipewire_cursor_hdr.py, test_pipewire_context_fail.py,
test_pulse_monitor_device.py).

Port of `test_pipewire_wrapper.sh` (obs-rust/obs-studio#962). They used to give
up after waiting 5 s for the daemon's socket and report the test as skipped
(77), which ctest counts as passed. With TMPDIR in a deep directory (a build
tree, a worktree) the daemon's socket path is longer than a Unix socket allows
(108 bytes): pipewire exits at once ("socket path ... exceeds 108 bytes"), and
every PipeWire and PulseAudio test was silently skipped.

  - a long TMPDIR: the daemon must still start and the test must run;
  - a daemon that exits at once: a failure, not a skip;
  - control: no pipewire binary at all (as in CI) still skips.

The test binary is a stub that checks it can reach the daemon's socket.
Skips (77) when pipewire is not installed.
usage: test_pipewire_wrapper.py <dir with the wrapper scripts>
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
from collections.abc import Callable
from pathlib import Path
from typing import NamedTuple

SKIP = 77
TIMEOUT_SECONDS = 60
WRAPPERS = ("test_pipewire_cursor_hdr.py", "test_pipewire_context_fail.py")
PULSE_WRAPPER = "test_pulse_monitor_device.py"
PULSE_TOOLS = ("wireplumber", "pipewire-pulse", "pactl", "dbus-run-session")
# A PATH with the shell tools but no pipewire (the control case).
SHELL_TOOLS = ("sh", "mktemp", "mkdir", "rm", "sleep", "kill", "cat")
USAGE = "usage: test_pipewire_wrapper.py <dir with the wrapper scripts>"

# Reports whether the daemon socket the wrapper set up is there: exit 0 if so,
# 3 if the test ran without a daemon (as the real tests then skip their cases
# and pass).
STUB = """#!/bin/sh
for d in "$PIPEWIRE_RUNTIME_DIR" "$XDG_RUNTIME_DIR"; do
\t[ -n "$d" ] && [ -S "$d/pipewire-0" ] && exit 0
done
echo "stub: no pipewire-0 socket in PIPEWIRE_RUNTIME_DIR/XDG_RUNTIME_DIR"
exit 3
"""

# A pipewire that exits at once, as one that cannot open its socket does.
DYING_PIPEWIRE = """#!/bin/sh
echo "fake pipewire: cannot start"
exit 1
"""

# The PulseAudio wrapper's stub: the private sound server must answer pactl.
PULSE_STUB = """#!/bin/sh
pactl info >/dev/null 2>&1 && exit 0
echo "stub: the private sound server does not answer"
exit 3
"""

# The tail of a case's output that a failure prints, and the cap's exit code.
OUTPUT_TAIL_LINES = 20
TIMEOUT_EXIT = 124  # `timeout`'s exit code


class Run(NamedTuple):
    """A case's exit status plus the tail of what it wrote (#2772)."""

    exit_code: int
    output: str


RunCase = Callable[..., Run]


def write_stub(path: Path, body: str) -> None:
    path.write_text(body)
    path.chmod(0o755)


def path_with(directory: Path) -> str:
    """`PATH=<directory>:$PATH`, the shell's prefix form."""
    return f"{directory}{os.pathsep}{os.environ.get('PATH', '')}"


def wrapper_env(base: dict[str, str]) -> dict[str, str]:
    """A case's environment, with uv's interpreter pinned (#2772).

    The control case strips `PATH`, which leaves uv unable to discover one
    there; uv then falls back to its own discovery and can spend the whole
    case budget downloading a Python (obs-rust/obs-studio#2772, where the case
    came back as `exit 124` with no explanation). Pinning the interpreter and
    forbidding downloads keeps the case measuring the wrapper's skip path.
    """
    env = dict(base)
    env["UV_PYTHON"] = sys.executable
    env["UV_PYTHON_DOWNLOADS"] = "never"
    return env


def captured_output(path: Path, limit: int = OUTPUT_TAIL_LINES) -> str:
    """The last non-blank `limit` lines the case wrote, or ''."""
    try:
        text = path.read_text(errors="replace")
    except OSError:
        return ""
    lines = [line for line in text.splitlines() if line.strip()]
    return "\n".join(lines[-limit:])


def run_wrapper(uv: str, wrapper: Path, stub: Path, out: Path, env: dict[str, str]) -> Run:
    """`timeout 60 uv run --script <wrapper> <stub>`, combined output to `out`."""
    try:
        with out.open("wb") as handle:
            completed = subprocess.run(
                [uv, "run", "--script", str(wrapper), str(stub)],
                env=env,
                stdout=handle,
                stderr=subprocess.STDOUT,
                timeout=TIMEOUT_SECONDS,
                check=False,
            )
            exit_code = completed.returncode
    except subprocess.TimeoutExpired:
        exit_code = TIMEOUT_EXIT
    # Read after the handle is closed: the tail is the case's own evidence.
    return Run(exit_code, captured_output(out))


class Report:
    """The shell's `check`, plus its `failed` accumulator."""

    def __init__(self) -> None:
        self.failed = 0

    def check(self, name: str, want: int, got: Run) -> None:
        if got.exit_code == want:
            print(f"ok:   {name} (exit {got.exit_code})")
        else:
            print(f"FAIL: {name}: exit status {got.exit_code}, expected {want}")
            self.evidence(got)
            self.failed = 1

    def check_dying(self, name: str, got: Run) -> None:
        # A daemon that dies must fail the wrapper: any status but 0 (passed),
        # 77 (skipped) or 3 (the test ran without a daemon and so skipped).
        if got.exit_code not in (0, SKIP, 3):
            print(f"ok:   {name}, daemon exits at once: fails (exit {got.exit_code})")
        else:
            print(
                f"FAIL: {name}, daemon exits at once: exit status {got.exit_code}, "
                "expected the wrapper to fail"
            )
            self.evidence(got)
            self.failed = 1

    @staticmethod
    def evidence(got: Run) -> None:
        """Print what the case wrote, so a failure is diagnosable (#2772)."""
        if got.exit_code == TIMEOUT_EXIT:
            print(f"      timed out at {TIMEOUT_SECONDS:g} s before the case finished")
        if not got.output:
            print("      the case wrote nothing")
            return
        print("      captured output:")
        for line in got.output.splitlines():
            print(f"      | {line}")



def check_pipewire_wrappers(report: Report, run_case: RunCase, target: Path) -> None:
    # Both wrappers are Python now (#962, GEN-005): run them the way their
    # ctest callers do.
    for wrapper in WRAPPERS:
        report.check(
            f"{wrapper}, long TMPDIR: the daemon starts and the test runs",
            0,
            run_case(wrapper, target, long_tmpdir=True),
        )
        report.check_dying(wrapper, run_case(wrapper, target, long_tmpdir=False))


def check_pulse_wrapper(report: Report, run_case: RunCase, target: Path) -> None:
    # The PulseAudio wrapper also needs WirePlumber, pipewire-pulse, pactl and
    # dbus-run-session; its stub checks the private sound server answers.
    if not all(shutil.which(tool) is not None for tool in PULSE_TOOLS):
        print(
            "skipped: test_pulse_monitor_device.py checks "
            "(WirePlumber, pipewire-pulse, pactl or dbus-run-session missing)"
        )
        return
    report.check(
        f"{PULSE_WRAPPER}, long TMPDIR: the sound server starts and the test runs",
        0,
        run_case(PULSE_WRAPPER, target, long_tmpdir=True),
    )
    report.check_dying(PULSE_WRAPPER, run_case(PULSE_WRAPPER, target, long_tmpdir=False))


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(USAGE, file=sys.stderr)
        return 2
    scripts = Path(argv[1])

    if shutil.which("pipewire") is None:
        print("pipewire not found: skipping")
        return SKIP

    # The control case at the end runs with a stripped PATH, so resolve uv by
    # absolute path for it.
    uv = shutil.which("uv")
    if uv is None:
        print("uv not found: skipping")
        return SKIP

    with tempfile.TemporaryDirectory(prefix="obs-pw-wrapper-test.") as tmp:
        work = Path(tmp)
        out = work / "out"
        report = Report()

        stub = work / "stub"
        write_stub(stub, STUB)

        # A TMPDIR deep enough that <TMPDIR>/<wrapper dir>/pipewire-0-manager
        # is over 108 bytes.
        deep_tmpdir = work / "a-build-tree-in-a-deeply-nested-worktree-directory" / "build-tmp"
        deep_tmpdir.mkdir(parents=True, exist_ok=True)

        bin_dying = work / "bin-dying"
        bin_dying.mkdir(parents=True, exist_ok=True)
        write_stub(bin_dying / "pipewire", DYING_PIPEWIRE)

        bin_none = work / "bin-none"
        bin_none.mkdir(parents=True, exist_ok=True)
        for tool in SHELL_TOOLS:
            found = shutil.which(tool)
            if found is not None:
                link = bin_none / tool
                if not link.exists():
                    link.symlink_to(found)

        def run_case(wrapper: str, target: Path, *, long_tmpdir: bool) -> Run:
            env = dict(os.environ)
            env["PATH"] = os.environ.get("PATH", "") if long_tmpdir else path_with(bin_dying)
            if long_tmpdir:
                # The shell ran the case with `TMPDIR="$long"`: a deep *directory*,
                # not the stub path (which is the test-binary argument).
                env["TMPDIR"] = str(deep_tmpdir)
            return run_wrapper(uv, scripts / wrapper, target, out, wrapper_env(env))

        pulse_stub = work / "pulse-stub"
        write_stub(pulse_stub, PULSE_STUB)

        check_pipewire_wrappers(report, run_case, stub)
        check_pulse_wrapper(report, run_case, pulse_stub)

        # Control: no pipewire binary, as in CI. uv is called by absolute path,
        # so the stripped PATH reaches it; the interpreter is pinned in
        # wrapper_env, so uv does not spend the case's budget looking for one.
        control = dict(os.environ)
        control["PATH"] = str(bin_none)
        got = run_wrapper(
            uv, scripts / "test_pipewire_cursor_hdr.py", stub, out, wrapper_env(control)
        )
        report.check("test_pipewire_cursor_hdr.py, no pipewire binary: skips", SKIP, got)

        return report.failed


if __name__ == "__main__":
    sys.exit(main(sys.argv))
