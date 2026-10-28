#!/usr/bin/env -S uv run --script
"""The sanitizers workflow's two multi-line steps, as one module (#962).

    uv run --no-project ci/sanitizers_step.py select --tools all
    uv run --no-project ci/sanitizers_step.py triage <out>/<tool>

GEN-005 gives a workflow `run:` step a one-line shell budget. These are the two
steps of `.github/workflows/sanitizers.yaml` that were over it; the harness
itself (`rust/tools/sanitize/sanitize.py`, `triage.py`) was already Python.

`select` is `jobs.matrix.steps[0]` (`id: pick`). It turns the `tools` input into
the `tools` matrix list and appends it to `$GITHUB_OUTPUT`: the literal `all`
means the three passes, anything else is that single pass, exactly as the
if/else did.

`triage` is `jobs.sanitize.steps[6]` (`Triage 🩺`). A missing results directory
is a build failure, reported in the step summary with status 1; otherwise it
runs `triage.py` over the directory, echoes its report to the log and appends
it to `$GITHUB_STEP_SUMMARY`, and returns the child's status -- what the
`| tee -a` under `bash -e -o pipefail` propagated. `FAIL_ON_FINDINGS=true` (the
workflow's `fail_on_findings` input) adds `--fail-on-findings`.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
TRIAGE = ROOT / "rust" / "tools" / "sanitize" / "triage.py"

# sanitize.py builds and runs these three passes; `all` is the default.
PASSES = ("asan", "tsan", "valgrind")

NO_RESULTS = "No results (build failed?)"

USAGE = "usage: sanitizers_step.py select --tools <all|asan|tsan|valgrind> | triage <out>/<tool>"


def append(path: str, text: str) -> bool:
    """Append to a runner-provided file; False when the variable is unset."""
    if not path:
        return False
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(text)
    return True


def select(tools: str) -> int:
    """The `pick` step: the matrix list goes to `$GITHUB_OUTPUT`."""
    picks = list(PASSES) if tools.strip() == "all" else [tools.strip()]
    # No spaces, the same bytes the shell's echo produced.
    line = f"tools={json.dumps(picks, separators=(',', ':'))}"
    if not append(os.environ.get("GITHUB_OUTPUT", ""), line + "\n"):
        print("GITHUB_OUTPUT is not set", file=sys.stderr)
        return 1
    return 0


def wants_fail_on_findings() -> bool:
    return os.environ.get("FAIL_ON_FINDINGS", "").strip().lower() in {"1", "true", "yes"}


def triage(out: Path) -> int:
    """The `Triage 🩺` step: report the pass, or the missing results."""
    if not out.is_dir():
        append(os.environ.get("GITHUB_STEP_SUMMARY", ""), NO_RESULTS + "\n")
        return 1
    argv = [sys.executable, str(TRIAGE), str(out)]
    if wants_fail_on_findings():
        argv.append("--fail-on-findings")
    done = subprocess.run(argv, capture_output=True, text=True, check=False)
    sys.stdout.write(done.stdout)
    sys.stderr.write(done.stderr)
    append(os.environ.get("GITHUB_STEP_SUMMARY", ""), done.stdout)
    return done.returncode


def main(argv: list[str]) -> int:
    if not argv:
        print(USAGE, file=sys.stderr)
        return 2
    command, rest = argv[0], argv[1:]
    if command == "select" and len(rest) == 2 and rest[0] == "--tools":
        return select(rest[1])
    if command == "triage" and len(rest) == 1:
        return triage(Path(rest[0]))
    print(USAGE, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
