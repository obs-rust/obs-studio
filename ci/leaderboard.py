#!/usr/bin/env -S uv run --script
"""The leaderboard workflow's three multi-line steps, as one module (#962).

    uv run --no-project ci/leaderboard.py generate --repo <owner/repo> --data data
    uv run --no-project ci/leaderboard.py render --data data
    uv run --no-project ci/leaderboard.py commit --data data

GEN-005 gives a workflow `run:` step a one-line shell budget. These are the
three steps of `.github/workflows/leaderboard.yml`'s `update` job that were over
it, in order, with the command lines they replaced:

- `Generate` (`steps[3]`): overlay `tools/leaderboard/site/*` onto the `data`
  checkout, drop the `.nojekyll` marker GitHub Pages needs, then run
  `tools/leaderboard/generate.py` into `data/leaderboard.json`.
- `Render JPEG` (`steps[5]`): install headless Chromium for playwright, then
  render the dashboard, the conversion card and the minimal README card.
- `Commit to leaderboard branch` (`steps[6]`, which ran with
  `working-directory: data`): commit whatever the two steps above wrote, if
  anything, and push it to the orphan `leaderboard` branch. "no changes" is
  success, exactly as the shell's `&& { echo "no changes"; exit 0; }` was.

Each child ran under `bash -e`, so the first non-zero status is the step's
status and nothing after it runs; every subcommand keeps that.

`--data` is the `data` checkout of the `leaderboard` branch (the workflow's
`actions/checkout` with `ref: leaderboard`), and `--site` exists so the test can
overlay a scratch directory instead of the real page assets.
"""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
TOOLS = ROOT / "tools" / "leaderboard"
SITE = TOOLS / "site"
GENERATE = "tools/leaderboard/generate.py"
RENDER = "tools/leaderboard/render.py"

PYTHON = "3.12"
PLAYWRIGHT = "playwright==1.49.1"
BRANCH = "leaderboard"
# The address GitHub's own actions use for the bot's commits.
BOT_NAME = "github-actions[bot]"
BOT_EMAIL = "41898282+github-actions[bot]@users.noreply.github.com"


def run(argv: list[str], cwd: Path = ROOT) -> int:
    """Run one child with the runner's stdio inherited; its status is the answer."""
    return subprocess.run(argv, cwd=cwd, check=False).returncode


def uv(*args: str) -> list[str]:
    return ["uv", *args]


def generate(repo: str, data: Path, site: Path = SITE) -> int:
    """`Generate`: the site overlay, `.nojekyll`, one generate.py run."""
    data.mkdir(parents=True, exist_ok=True)
    for item in sorted(site.iterdir()):
        # `cp site/* data/` copied the regular files; a directory there would
        # have failed the step ("omitting directory"), so skip it the same way.
        if item.is_file():
            shutil.copy2(item, data / item.name)
    (data / ".nojekyll").touch()
    return run(
        uv(
            "run",
            "--no-project",
            "--python",
            PYTHON,
            GENERATE,
            "--repo",
            repo,
            "--out",
            str(data / "leaderboard.json"),
        )
    )


def render(data: Path) -> int:
    """`Render JPEG`: install Chromium, then the three renders in order."""
    with_playwright = uv(
        "run", "--no-project", "--python", PYTHON, "--with", PLAYWRIGHT, RENDER
    )
    steps = [
        uv(
            "run",
            "--python",
            PYTHON,
            "--with",
            PLAYWRIGHT,
            "playwright",
            "install",
            "--with-deps",
            "chromium",
        ),
        # No page/out arguments: render.py defaults to index.html -> leaderboard.jpg.
        [*with_playwright, str(data)],
        [*with_playwright, str(data), "conversion.html", "conversion.jpg"],
        [
            *with_playwright,
            str(data),
            "conversion-minimal.html",
            "conversion-minimal.jpg",
            "640x100",
        ],
    ]
    for argv in steps:
        code = run(argv)
        if code:
            return code
    return 0


def git(data: Path, *args: str) -> int:
    return run(["git", *args], cwd=data)


def commit_message() -> str:
    """`leaderboard: update <UTC date>T<UTC hour>:<UTC minute>Z`, as `date -u +%FT%H:%MZ`."""
    return "leaderboard: update " + datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%MZ")


def commit(data: Path) -> int:
    """`Commit to leaderboard branch`: config, add, nothing-to-do, commit, push."""
    for argv in (("config", "user.name", BOT_NAME), ("config", "user.email", BOT_EMAIL), ("add", "-A")):
        code = git(data, *argv)
        if code:
            return code
    # `git diff --cached --quiet` is 0 when nothing is staged, 1 when something is.
    staged = git(data, "diff", "--cached", "--quiet")
    if staged == 0:
        print("no changes")
        return 0
    if staged != 1:
        return staged
    code = git(data, "commit", "-m", commit_message())
    if code:
        return code
    return git(data, "push", "origin", f"HEAD:{BRANCH}")


def parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(description="The leaderboard workflow's three steps (#962).")
    sub = ap.add_subparsers(dest="command", required=True)

    gen = sub.add_parser("generate", help="overlay the site, then generate the JSON")
    gen.add_argument("--repo", required=True, help="owner/repo the leaderboard measures")
    gen.add_argument("--data", required=True, type=Path, help="the leaderboard branch checkout")
    gen.add_argument("--site", type=Path, default=SITE, help="page assets to overlay (tests)")

    ren = sub.add_parser("render", help="install Chromium and render the three pages")
    ren.add_argument("--data", required=True, type=Path)

    com = sub.add_parser("commit", help="commit and push the data checkout")
    com.add_argument("--data", required=True, type=Path)
    return ap


def main(argv: list[str]) -> int:
    args = parser().parse_args(argv)
    if args.command == "generate":
        return generate(args.repo, args.data, args.site)
    if args.command == "render":
        return render(args.data)
    return commit(args.data)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
