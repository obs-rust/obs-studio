#!/usr/bin/env -S uv run --script
"""Welcome a first-time contributor, once per person (#2506).

    uv run ci/welcome.py --event "$GITHUB_EVENT_PATH" --repo "$GITHUB_REPOSITORY"
    uv run ci/welcome.py --event payload.json --dry-run

The trigger was #2483: a first-time contributor filed an issue, sent a fix
with a failing test first, and then waited for a maintainer to approve the CI
run of their fork. Nothing in the project had said "welcome", and nothing had
answered the one question a first fork PR actually raises ("why is CI
waiting?").

The decision from the issue:

- **B, first PR of an account** (`pull_request_target: opened`) and a first
  **issue** of an account (`issues: opened`) get the same `first-pr` message,
  so nobody is welcomed twice for one thing.
- **C, first merged PR of an account** (`pull_request_target: closed`,
  merged) gets the `first-merge` message.
- **D** (CI red on a first-timer's PR) is deliberately not implemented: it is
  the only event that can repeat for one person, so it is left for a later
  PR on its own.

Rules the code holds:

- Never for a member, owner or collaborator, and never for a bot.
- Once per person per message, keyed on a hidden marker in the posted comment
  itself, written by this repository's own automation. A contributor quoting
  the marker cannot suppress their own welcome.
- Fail closed. A search error, a missing token or an unreadable template
  prints why and posts nothing: a welcome bot that misfires on a stranger's
  thread is worse than one that stays quiet.

The text lives in `.github/welcome/*.md` (reviewed like code); this file only
decides and posts. ci/test_welcome.py pins every rule here.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TEMPLATES = ROOT / ".github" / "welcome"
REPO = "obs-rust/obs-studio"
API = "https://api.github.com"

# The automation comments as github-actions[bot]; only its own comments count
# for the once-only marker.
BOT_AUTHORS = frozenset({"github-actions[bot]"})
# Maintainers: never welcomed, whatever they do here. Their association is
# one of these, so the two gates below exclude them on their own.
STAFF = frozenset({"OWNER", "MEMBER", "COLLABORATOR"})
# A first PR or issue of an account. FIRST_TIMER is "never contributed
# anywhere", FIRST_TIME_CONTRIBUTOR is "never contributed here". Anything
# else - CONTRIBUTOR, STAFF, NONE - has contributed or has a relationship.
FIRST_TIME = frozenset({"FIRST_TIMER", "FIRST_TIME_CONTRIBUTOR"})

FIRST_PR = "first-pr"
FIRST_MERGE = "first-merge"
TEMPLATE_FILES = {FIRST_PR: "first-pr.md", FIRST_MERGE: "first-merge.md"}

# The author mention is the one placeholder the bot fills; anything else in
# {{...}} is an optional line a maintainer would write by hand.
AUTHOR_PLACEHOLDER = "{{author}}"
OPTIONAL_PLACEHOLDER = re.compile(r"[ \t]*\{\{[^}]*\}\}")


@dataclass(frozen=True)
class Post:
    """A comment to post on one thread."""

    number: int
    marker: str
    template: Path
    body: str


def is_bot(login: str) -> bool:
    return login.endswith("[bot]") or login in BOT_AUTHORS


def event_target(payload: dict) -> tuple[str, dict] | tuple[None, None]:
    """The event name and the object it is about, or (None, None)."""
    for key, name in (("pull_request", "pr"), ("issue", "issue")):
        target = payload.get(key)
        if isinstance(target, dict):
            return name, target
    return None, None


def render(marker: str, author: str) -> str:
    """The template for this message, with the mention and the marker."""
    text = (TEMPLATES / TEMPLATE_FILES[marker]).read_text(encoding="utf-8").strip()
    if AUTHOR_PLACEHOLDER not in text:
        raise ValueError(
            f"{TEMPLATE_FILES[marker]} has no {AUTHOR_PLACEHOLDER} placeholder"
        )
    text = text.replace(AUTHOR_PLACEHOLDER, f"@{author}")
    # An optional line a maintainer would fill in by hand (C's "one line on
    # what the fix does") is dropped rather than posted raw: the message reads
    # complete without it, and a maintainer can edit the comment to name it.
    text = OPTIONAL_PLACEHOLDER.sub("", text).strip()
    return f"{text}\n\n<!-- welcome:{marker} -->"


def marker_for(
    action: str | None,
    kind: str | None,
    target: dict,
    author: str,
    merged_pr_count: Callable[[str], int],
) -> str | None:
    """Which message this event calls for, or None."""
    association = target.get("author_association") or "NONE"
    if action == "opened" and kind in ("pr", "issue"):
        return FIRST_PR if association in FIRST_TIME else None
    if action == "closed" and kind == "pr":
        # CONTRIBUTOR is deliberately allowed here: GitHub assigns it to an
        # author as soon as they have a contribution, so a first-time
        # contributor's association can already read CONTRIBUTOR at the
        # moment their only PR merges. The merged-PR count below is the real
        # "is this their first" test; only a staff relation is a hard skip.
        if not target.get("merged") or association in STAFF:
            return None
        try:
            # The merged PR that triggers this counts toward the total, so a
            # first merge is exactly 1. Anything else (an older merged PR, an
            # API error, a missing token) means no welcome.
            return FIRST_MERGE if merged_pr_count(author) == 1 else None
        except Exception as exc:  # fail closed: no count, no welcome
            print(f"welcome: not welcoming @{author}: {exc}", file=sys.stderr)
            return None
    return None


def decide(
    payload: dict,
    *,
    already_posted: Callable[[int, str], bool],
    merged_pr_count: Callable[[str], int],
) -> Post | None:
    """Which comment to post for this event, or None.

    `already_posted(number, marker)` and `merged_pr_count(author)` are
    injected, so every rule here is a unit test with no network.
    """
    if (payload.get("repository") or {}).get("full_name") != REPO:
        return None
    kind, target = event_target(payload)
    if target is None:
        return None
    author = (target.get("user") or {}).get("login") or ""
    if is_bot(author):
        return None
    marker = marker_for(payload.get("action"), kind, target, author, merged_pr_count)
    number = target.get("number")
    if marker is None or not isinstance(number, int) or already_posted(number, marker):
        return None
    return Post(
        number=number,
        marker=marker,
        template=TEMPLATES / TEMPLATE_FILES[marker],
        body=render(marker, author),
    )


def api(path: str, token: str, *, method: str = "GET", body: dict | None = None):
    request = urllib.request.Request(
        f"{API}{path}" if path.startswith("/") else path,
        data=json.dumps(body).encode() if body is not None else None,
        method=method,
        headers={
            "Authorization": f"Bearer {token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "obs-rust-welcome-bot",
        },
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        text = response.read().decode("utf-8")
    return json.loads(text) if text else {}


def posted_marker(number: int, marker: str, repo: str, token: str, fetch=api) -> bool:
    """True when this automation already posted `marker` on this thread."""
    comments = fetch(f"/repos/{repo}/issues/{number}/comments?per_page=100", token)
    if not isinstance(comments, list):
        raise ValueError("unexpected comments payload")
    needle = f"<!-- welcome:{marker} -->"
    # One page is enough: the bot posts at most one comment per thread and
    # does it early, so the marker is inside the first hundred comments of
    # any thread it could still be missing from.
    return any(
        (comment.get("user") or {}).get("login") in BOT_AUTHORS
        and needle in (comment.get("body") or "")
        for comment in comments
    )


def merged_pr_count(author: str, repo: str, token: str, fetch=api) -> int:
    query = f"repo:{repo} is:pr is:merged author:{author}"
    result = fetch(f"/search/issues?q={urllib.parse.quote(query)}", token)
    return int(result["total_count"])


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        description="Welcome a first-time contributor (#2506)."
    )
    parser.add_argument(
        "--event",
        default=os.environ.get("GITHUB_EVENT_PATH", ""),
        help="the event payload JSON",
    )
    parser.add_argument("--repo", default=os.environ.get("GITHUB_REPOSITORY", REPO))
    parser.add_argument(
        "--dry-run", action="store_true", help="print the comment instead of posting it"
    )
    args = parser.parse_args(argv)

    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN", "")
    if not token:
        print("welcome: no GH_TOKEN/GITHUB_TOKEN; posting nothing")
        return 0
    if not args.event or not Path(args.event).is_file():
        print("welcome: no event payload; posting nothing")
        return 0

    payload = json.loads(Path(args.event).read_text(encoding="utf-8"))
    try:
        post = decide(
            payload,
            already_posted=lambda number, marker: posted_marker(
                number, marker, args.repo, token
            ),
            merged_pr_count=lambda author: merged_pr_count(author, args.repo, token),
        )
    except Exception as exc:  # a bot must never redden CI
        print(f"welcome: could not decide ({exc}); posting nothing", file=sys.stderr)
        return 0

    if post is None:
        print("welcome: nothing to post")
        return 0
    if args.dry_run:
        print(f"welcome: would post {post.marker} on #{post.number}\n\n{post.body}")
        return 0
    api(
        f"/repos/{args.repo}/issues/{post.number}/comments",
        token,
        method="POST",
        body={"body": post.body},
    )
    print(f"welcome: posted {post.marker} on #{post.number}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
