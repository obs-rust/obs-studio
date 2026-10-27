#!/usr/bin/env -S uv run --script
"""build-project.yaml installs soldr before it calls it, and never runs bare `rustup`.

The `macos-build` and `windows-build` jobs install the toolchain `rust-toolchain.toml`
pins and add a cross-target standard library. Both go through `soldr rustup` so the
pinned toolchain, cache and Dylint wiring apply; a bare `rustup` bypasses Soldr's
Rust toolchain wrapper (ci-lint RUST-001/TOOL-001). The sanctionsised form is the
one `release-rust.yaml` already uses (`soldr rustup target add <triple>`).

Calling `soldr` needs soldr, so every job that runs a `soldr` subcommand must carry a
`zackees/setup-soldr` step before it. #2650 rewrote the toolchain step in those two
jobs without adding one, and both app builds then died with `command not found:
soldr` (#2699). setup-soldr provisions the pinned toolchain itself, so the only
`soldr rustup` call left is the cross-target `target add`.

    uv run python -m unittest ci/test_soldr_rustup.py
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WORKFLOW = ROOT / ".github/workflows/build-project.yaml"

# A `rustup` token not preceded by `soldr ` on the same line.
BARE_RUSTUP = re.compile(r"(?<!soldr )\brustup\b")

# A job key: a two-space-indented mapping key, e.g. `  macos-build:`.
JOB_KEY = re.compile(r"^  ([A-Za-z0-9_.-]+):\s*$")

# A soldr subcommand invocation, not a comment or a version bump.
SOLDR_CALL = re.compile(r"\bsoldr\s+(?:cargo|rustup|nextest|install|run)\b")

SETUP_SOLDR = "zackees/setup-soldr"

APP_BUILD_JOBS = ("macos-build", "windows-build")


def bare_rustup_lines(text: str) -> list[str]:
    """Every non-comment line that invokes `rustup` without the `soldr ` wrapper."""
    return [
        line
        for line in text.splitlines()
        if not line.lstrip().startswith("#") and BARE_RUSTUP.search(line)
    ]


def soldr_jobs_without_setup(text: str) -> list[str]:
    """Jobs that run a `soldr` subcommand with no `setup-soldr` step before it."""
    offenders: list[str] = []
    job: str | None = None
    setup_seen = False
    call_seen = False

    def close() -> None:
        if job is not None and call_seen and not setup_seen:
            offenders.append(job)

    for line in text.splitlines():
        key = JOB_KEY.match(line)
        if key:
            close()
            job, setup_seen, call_seen = key.group(1), False, False
            continue
        if job is None or line.lstrip().startswith("#"):
            continue
        if SETUP_SOLDR in line:
            setup_seen = True
        elif SOLDR_CALL.search(line):
            call_seen = True
    close()
    return offenders


class NoBareRustup(unittest.TestCase):
    def test_build_project_wraps_rustup_in_soldr(self):
        offenders = bare_rustup_lines(WORKFLOW.read_text())
        self.assertEqual(
            offenders,
            [],
            "install the toolchain through `soldr rustup` (RUST-001/TOOL-001): "
            + "; ".join(offenders),
        )

    def test_cross_targets_are_installed_through_soldr(self):
        text = WORKFLOW.read_text()
        self.assertIn("soldr rustup target add x86_64-apple-darwin", text)
        self.assertIn("soldr rustup target add aarch64-pc-windows-msvc", text)


class SoldrIsInstalledBeforeItIsCalled(unittest.TestCase):
    def test_every_job_that_calls_soldr_sets_it_up_first(self):
        offenders = soldr_jobs_without_setup(WORKFLOW.read_text())
        self.assertEqual(
            offenders,
            [],
            "these jobs run `soldr` with no zackees/setup-soldr step before it "
            "(the job then dies with `command not found: soldr`): "
            + ", ".join(offenders),
        )

    def test_app_build_jobs_set_soldr_up(self):
        text = WORKFLOW.read_text()
        self.assertEqual(
            set(APP_BUILD_JOBS) & set(soldr_jobs_without_setup(text)),
            set(),
            "the macOS and Windows app builds must set soldr up themselves; "
            "they are the only jobs PR CI cannot run",
        )

    def test_setup_soldr_provisions_the_toolchain(self):
        # setup-soldr installs the rust-toolchain.toml toolchain, so a separate
        # `soldr rustup toolchain install` in the same workflow is dead weight.
        self.assertNotIn("soldr rustup toolchain install", WORKFLOW.read_text())


if __name__ == "__main__":
    unittest.main()
