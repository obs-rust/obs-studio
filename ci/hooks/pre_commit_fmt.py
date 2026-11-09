#!/usr/bin/env python3
"""The pre-commit hook body: format the Rust tree before it is committed.

Installed by `./install-hooks` (or `./push --install-hooks`). It runs the same
whole-workspace rustfmt the gate runs, so a commit cannot carry mis-formatted
Rust into review and make CI red for a reason a formatter could have fixed:

    soldr cargo fmt --all          (rewrite, then re-stage what it changed)

`soldr` is used rather than a bare `cargo` so the pinned toolchain and the
shared compile cache apply; when soldr is absent the hook falls back to `cargo`
and says so once. Nothing else runs here — clippy and dylint are too slow for a
commit and belong to ./push.

Exit 1 when the rewrite touched files that are staged, so the commit stops and
you see what changed; the fix is already applied, so `git add -A && git commit`
finishes the job.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path
import sys


def run(args: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, text=True, capture_output=True, cwd=cwd)


def workspace_roots(staged: list[str]) -> list[Path]:
    """The workspaces the staged files belong to, nearest first.

    Only these are formatted: running every workspace in the repo would rewrite
    unrelated crates and drag them into the commit.
    """

    roots: list[Path] = []
    for name in staged:
        directory = Path(name).parent.resolve()
        for candidate in [directory, *directory.parents]:
            manifest = candidate / "Cargo.toml"
            if manifest.is_file() and "[workspace]" in manifest.read_text(errors="ignore"):
                if candidate not in roots:
                    roots.append(candidate)
                break
    return roots


def main() -> int:
    if run(["git", "rev-parse", "--git-dir"]).returncode != 0:
        return 0

    staged = run(["git", "diff", "--cached", "--name-only", "--diff-filter=ACMR"])
    rust = [line for line in staged.stdout.splitlines() if line.endswith(".rs")]
    if not rust:
        return 0

    driver = "soldr" if shutil.which("soldr") else "cargo"
    if not shutil.which(driver):
        print("pre-commit: no soldr or cargo on PATH; skipping rustfmt", file=sys.stderr)
        return 0

    # `--all` formats one workspace. This repo also carries standalone crates
    # with their own [workspace] (dylints/*, rust/*/fuzz), and `--all` at the
    # root silently skips them, so each root is formatted where it lives.
    for root in workspace_roots(rust):
        fmt = [driver, "cargo", "fmt", "--all"] if driver == "soldr" else ["cargo", "fmt", "--all"]
        result = run(fmt, cwd=root)
        if result.returncode != 0:
            print(f"{driver} fmt failed in {root}:", file=sys.stderr)
            print(result.stderr.strip() or result.stdout.strip(), file=sys.stderr)
            return 1

    changed = run(["git", "diff", "--name-only", "--diff-filter=ACMR"])
    touched = [line for line in changed.stdout.splitlines() if line.endswith(".rs")]
    if not touched:
        return 0

    subprocess.run(["git", "add", "--", *touched], check=False)
    print(
        "pre-commit: rustfmt rewrote " + ", ".join(touched) +
        "\n  the fix is staged; review it and commit again.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
