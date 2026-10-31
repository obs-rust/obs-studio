#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""User quotes for the README, from obs-rust/obs-studio's own issue and PR comments.

Three steps, so a person or an agent judges every quote and nothing is
keyword-filtered:

    uv run tools/user_quotes.py collect --out candidates.json
    #   ... the reviewer writes verdicts.json: {"<comment url>": ["approved" | "rejected", "reason"]}
    uv run tools/user_quotes.py record candidates.json verdicts.json
    uv run tools/user_quotes.py render            # rewrite the README section
    uv run tools/user_quotes.py render --check    # fail if the README is stale

`collect` takes every issue and PR comment by a GitHub `User` account that is
not a maintainer (OWNER, MEMBER, COLLABORATOR) and not on the contributors
leaderboard, removes code blocks, `<details>` logs, HTML comments and quoted
replies, and leaves out comments already in tools/user_quotes.json, so a
rejected quote stays rejected. It reads only comments updated since the last
recorded scan. The candidates file carries RULE, the judging rule.

`record` takes the quote text from the candidates file, never from the
verdicts, and checks it against the live comment, so a quote is always the
comment's own words. Every candidate needs a verdict.

The repo's CI has no model access, so the reviewer is the coordinator agent
(obs-coordinator skill, "User quotes") or a person. Standard library and `gh`.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO = "obs-rust/obs-studio"
ROOT = Path(__file__).resolve().parent.parent
DATA = Path(__file__).resolve().parent / "user_quotes.json"
README = ROOT / "README.md"
START = "<!-- user-quotes:start -->"
END = "<!-- user-quotes:end -->"
SHOWN = 8
MAX_CHARS = 600

MAINTAINERS = {"OWNER", "MEMBER", "COLLABORATOR"}
# Agent and bot accounts that are typed "User" (the leaderboard's list, tools/leaderboard/generate.py).
BOT_RE = re.compile(r"(\[bot\]$|^(claude|copilot|dependabot|github-actions|coderabbit)|anthropic)", re.I)
BLOCKS = re.compile(r"```.*?(?:```|\Z)|~~~.*?(?:~~~|\Z)|<details>.*?(?:</details>|\Z)|<!--.*?(?:-->|\Z)",
                    re.S | re.I)
QUOTED = re.compile(r"^[ \t]*>.*$", re.M)

RULE = """\
Approve a comment only when every one of these holds; otherwise reject it, with a short reason.
1. Taken as a whole, it is positive about OBS Rust or a fix in it. A comment that turns ("works, but ...",
   "except ...") or leaves a complaint standing is rejected; never quote the positive half.
2. The author used it: they ran a build, a release or a fix and say how it went. Thanks for a reply,
   a question or a feature request alone is not a quote.
3. No hostility, sarcasm, or framing about AI or about people (upstream OBS included).
4. It reads correctly word for word, exactly as shown: nothing cut, added or reworded. Reject it when
   removing the code blocks or quoted replies left it unclear, or it only makes sense inside its thread.
5. Nothing personal beyond the GitHub login: no names, e-mail addresses, home paths or other identifiers.
"""



def clean(body: str | None) -> str:
    raise NotImplementedError


def is_candidate(comment: dict, excluded: set[str]) -> bool:
    raise NotImplementedError


def candidates(comments: list[dict], excluded: set[str], known: set[str],
               max_chars: int = MAX_CHARS) -> list[dict]:
    raise NotImplementedError


def record(data: dict, found: dict, verdicts: dict, live: dict[str, str]) -> dict:
    raise NotImplementedError


def section(data: dict, shown: int = SHOWN) -> str:
    raise NotImplementedError


def render(readme: str, data: dict, shown: int = SHOWN) -> str:
    raise NotImplementedError


def load(path: Path) -> dict:
    raise NotImplementedError


def dump(data: dict) -> str:
    raise NotImplementedError


def main() -> int:
    raise NotImplementedError


if __name__ == "__main__":
    raise SystemExit(main())
