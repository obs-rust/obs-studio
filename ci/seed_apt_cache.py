#!/usr/bin/env -S uv run --script
"""Fill the shared Tier 2 apt archive without waiting on a build to finish.

`Install Tier 2 Packages` downloads roughly 339 MB inside a ten-minute step.
On a slow link it never finishes, and until 5522 the failure also threw the
partial archive away, so every run restarted at `Get:1`. 5522 keeps what a
failed run downloaded, but the archive still only grows when some run gets
lucky: on a link where the download needs eleven minutes, no build ever hands
over a complete set.

This job has no build to lose. It restores the archive, downloads the package
list into it in bounded attempts, and stops when the archive stops growing or
its budget runs out. The workflow then saves the result under the content key
with the .deb count appended, which is the same key shape the build jobs
restore by prefix, so the next build starts where this one finished.

Exit status is 0 whenever at least one package arrived: a partial seed is
still progress, and the save step runs either way.
"""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
PACKAGES = ROOT / 'rust' / 'tools' / 'linux-validate' / 'apt-packages.txt'

# Four times the ~11 minutes a slow link needs, so a single run usually
# finishes; a run that runs out still leaves a larger archive behind.
DEFAULT_BUDGET_SECONDS = 2700
DEFAULT_ATTEMPTS = 12
UPDATE_BOUND = 180
INSTALL_BOUND = 600
IDLE_SLEEP = 15


def selected_packages(path: Path = PACKAGES) -> list[str]:
    """The package names the Tier 2 image installs, comments removed."""
    names = []
    for line in path.read_text().splitlines():
        name = line.split('#', 1)[0].strip()
        if name:
            names.append(name)
    return names


def archive_count(directory: Path) -> int:
    if not directory.is_dir():
        return 0
    return sum(1 for entry in directory.iterdir() if entry.suffix == '.deb' and entry.is_file())


def bounded(command: list[str], limit: int) -> int:
    """Run one command under the remaining budget; a timeout is not fatal."""
    if limit <= 0:
        return 124
    print(f'+ {" ".join(command)} (bound {limit}s)', flush=True)
    try:
        return subprocess.run(command, timeout=limit, check=False).returncode
    except subprocess.TimeoutExpired:
        print(f'  stopped at the {limit}s bound; keeping what arrived', flush=True)
        return 124


def seed(directory: Path, packages: list[str], budget: int, attempts: int) -> int:
    started = time.monotonic()
    expected = len(packages)
    best = archive_count(directory)
    print(f'archive starts at {best}/{expected} packages', flush=True)
    for attempt in range(1, attempts + 1):
        remaining = budget - int(time.monotonic() - started)
        if remaining < UPDATE_BOUND:
            print(f'attempt {attempt}: {remaining}s left, stopping', flush=True)
            break
        bounded(['sudo', 'apt-get', 'update', '-qq'], min(UPDATE_BOUND, remaining))
        remaining = budget - int(time.monotonic() - started)
        bounded(['sudo', 'apt-get', 'install', '--download-only', '-y',
                 '--no-install-recommends', '--fix-missing',
                 '-o', f'Dir::Cache::archives={directory}', *packages],
                min(INSTALL_BOUND, remaining))
        count = archive_count(directory)
        print(f'attempt {attempt}: {count}/{expected} packages (+{count - best})', flush=True)
        if count >= expected:
            return count
        if count == best:
            time.sleep(IDLE_SLEEP)
        best = count
    return archive_count(directory)


def main() -> None:
    raw = os.environ.get('TIER2_APT_ARCHIVES', '')
    if not raw:
        raise SystemExit('TIER2_APT_ARCHIVES must point at the shared archive directory')
    directory = Path(raw)
    directory.mkdir(parents=True, exist_ok=True)
    budget = int(os.environ.get('SEED_BUDGET_SECONDS', DEFAULT_BUDGET_SECONDS))
    attempts = int(os.environ.get('SEED_ATTEMPTS', DEFAULT_ATTEMPTS))
    count = seed(directory, selected_packages(), budget, attempts)
    print(f'seeded {count} packages into {directory}', flush=True)
    sys.exit(0 if count else 1)


if __name__ == '__main__':
    main()
