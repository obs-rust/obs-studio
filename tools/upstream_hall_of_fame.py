#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Build docs/hall-of-fame.md from upstream obsproject/obs-studio issues.

One cheap pass over the GitHub search API (via `gh api`), no per-issue LLM
calls. An issue qualifies when users clearly cared (many comments or
reactions) and upstream shut it down anyway: closed as "not planned",
and/or locked. Score = comments + 2 * reactions, with a bonus for locking.

    uv run tools/upstream_hall_of_fame.py              # write the markdown
    uv run tools/upstream_hall_of_fame.py --dry-run    # print the table only
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

REPO = "obsproject/obs-studio"
OUT = Path(__file__).resolve().parent.parent / "docs" / "hall-of-fame.md"
QUOTES = Path(__file__).resolve().parent / "upstream_hall_of_fame_quotes.json"

# Each query is sorted server side, so a few pages cover the high-engagement tail.
QUERIES = [
    f'repo:{REPO} is:issue is:closed reason:"not planned" sort:comments-desc',
    f'repo:{REPO} is:issue is:closed reason:"not planned" sort:reactions-desc',
    f"repo:{REPO} is:issue is:locked sort:comments-desc",
    f"repo:{REPO} is:issue is:locked sort:reactions-desc",
]


def search(query: str, pages: int) -> list[dict]:
    items: list[dict] = []
    for page in range(1, pages + 1):
        batch = json.loads(gh(["-X", "GET", "search/issues", "-f", f"q={query}",  # ci-lint: allow GHAPI-001 bounded search page walk; the sleep is gh()'s 429 backoff, not a state poll
                               "-f", "per_page=100", "-f", f"page={page}"]))["items"]
        items += batch
        if len(batch) < 100:
            break
    return items


def gh(args: list[str]) -> str:
    """`gh api`, waiting out the search API's 30-requests-a-minute limit."""
    for attempt in range(5):
        r = subprocess.run(["gh", "api", *args], capture_output=True, text=True)
        if r.returncode == 0:
            return r.stdout
        if "rate limit" not in (r.stdout + r.stderr).lower():
            # Search stops at 1000 results; a page past the end is not an error worth failing on.
            if "only the first 1000" in (r.stdout + r.stderr).lower():
                return '{"items": []}'
            raise SystemExit(r.stderr or r.stdout)
        time.sleep(30 * (attempt + 1))
    raise SystemExit("gh api: still rate limited")


def candidates(pages: int, cache: Path | None) -> dict[int, dict]:
    if cache and cache.exists():
        return {i["number"]: i for i in json.loads(cache.read_text())}
    seen: dict[int, dict] = {}
    for q in QUERIES:
        for issue in search(q, pages):
            seen[issue["number"]] = issue
    if cache:
        cache.write_text(json.dumps(list(seen.values())))
    return seen


def digest(issue: dict, keep: int) -> str:
    """Body excerpt plus maintainer comments and the last few comments, trimmed."""
    n = issue["number"]
    comments = json.loads(gh(["--paginate", "--slurp",
                              f"repos/{REPO}/issues/{n}/comments?per_page=100"]))
    comments = [c for page in comments for c in page]
    staff = {"OWNER", "MEMBER", "COLLABORATOR"}
    picked = [c for c in comments if c["author_association"] in staff]
    picked += [c for c in comments[-keep:] if c not in picked]
    picked.sort(key=lambda c: c["created_at"])
    clip = lambda t, k: " ".join((t or "").split())[:k]
    out = [f"### #{n} {issue['title']}",
           f"{issue['comments']} comments, {issue['reactions']['total_count']} reactions, "
           f"{verdict(issue)}, closed {issue.get('closed_at')}",
           f"BODY: {clip(issue['body'], 500)}"]
    for c in picked:
        tag = "STAFF" if c["author_association"] in staff else "user"
        out.append(f"- [{tag} {c['user']['login']} {c['created_at'][:10]} "
                   f"+{c['reactions']['total_count']}] {clip(c['body'], 400)}")
    return "\n".join(out) + "\n"


def score(issue: dict) -> int:
    reactions = issue["reactions"]["total_count"]
    return issue["comments"] + 2 * reactions + (25 if issue["locked"] else 0)


def qualifies(issue: dict, min_comments: int, min_reactions: int) -> bool:
    shut_down = issue.get("state_reason") == "not_planned" or issue["locked"]
    engaged = (issue["comments"] >= min_comments
               or issue["reactions"]["total_count"] >= min_reactions)
    return shut_down and engaged


def verdict(issue: dict) -> str:
    parts = []
    if issue.get("state_reason") == "not_planned":
        parts.append("closed not planned")
    elif issue["state"] == "closed":
        parts.append("closed")
    if issue["locked"]:
        reason = issue.get("active_lock_reason") or "no reason"
        parts.append(f"locked ({reason})")
    return ", ".join(parts)


def table(rows: list[dict]) -> list[str]:
    noted = any("note" in i for i in rows)
    lines = ["| # | Issue | Comments | 👍 | Outcome | Closed |" + (" Why |" if noted else ""),
             "|---|---|---:|---:|---|---|" + ("---|" if noted else "")]
    for i in rows:
        title = i["title"].replace("|", "\\|")
        closed = (i.get("closed_at") or "")[:10] or "open"
        why = f" {i['note'].replace('|', '/')} |" if noted else ""
        lines.append(f"| {i['number']} | [{title}]({i['html_url']}) | {i['comments']} "
                     f"| {i['reactions']['+1']} | {verdict(i)} | {closed} |{why}")
    return lines


def badge(label: str, value: object, color: str) -> str:
    esc = lambda t: str(t).replace("-", "--").replace("_", "__").replace(" ", "_")
    return (f"![{label}: {value}](https://img.shields.io/badge/"
            f"{esc(label)}-{esc(value)}-{color}?style=flat-square)")


def card(rank: int, i: dict) -> list[str]:
    medal = {1: "🥇", 2: "🥈", 3: "🥉"}.get(rank, "🏆")
    closed = (i.get("closed_at") or "")[:10]
    badges = [badge("comments", i["comments"], "blue")]
    if i.get("state_reason") == "not_planned":
        badges.append(badge("closed", "not planned", "lightgrey"))
    if i["locked"]:
        badges.append(badge("locked", i.get("active_lock_reason") or "yes", "red"))
    if closed == "2024-10-14":
        badges.append(badge("stale sweep", closed, "orange"))
    return [f"### {medal} [#{i['number']} · {i['title']}]({i['html_url']})", "",
            " ".join(badges), "", f"> {i['note']}", ""]


def quote_block(q: dict) -> list[str]:
    return [f"> **“{q['q']}”**", ">",
            f"> — an OBS maintainer, {q['context']} · [#{q['n']}, {q['date']}]({q['url']})", ""]


def render_hall(rows: list[dict]) -> str:
    quotes = json.loads(QUOTES.read_text()) if QUOTES.exists() else []
    by_size = lambda rs: sorted(rs, key=lambda i: i["comments"], reverse=True)
    yes = by_size([i for i in rows if i["tier"] == "yes"])
    maybe = by_size([i for i in rows if i["tier"] == "borderline"])
    sweep = sum((i.get("closed_at") or "").startswith("2024-10-14") for i in yes)
    locked = sum(i["locked"] for i in yes)
    voices = sum(i["comments"] for i in yes)
    out = [
        '<div align="center">', "",
        "# 🏆 OBS Project User Abuse Hall of Fame", "",
        "**Bugs they confirmed, then closed. Features you relied on, then removed.",
        "Threads where users showed up in numbers, then got locked out.**", "",
        badge("inducted", len(yes), "crimson") + " " + badge("locked", locked, "red")
        + " " + badge("closed in one afternoon", sweep, "orange")
        + " " + badge("comments", voices, "blue"), "",
        "</div>", "", "---", "",
        "## The scoreboard", "",
        "| | Issue | 💬 | Fate |", "|:-:|---|--:|---|",
    ]
    for rank, i in enumerate(yes, 1):
        medal = {1: "🥇", 2: "🥈", 3: "🥉"}.get(rank, str(rank))
        title = i["title"].replace("|", "\\|")
        out.append(f"| {medal} | [{title}]({i['html_url']}) | {i['comments']} | {verdict(i)} |")
    out += ["", f"> [!WARNING]", f"> **October 14, 2024:** {sweep} of these were closed as stale",
            "> on the same afternoon, including bugs the maintainers had reproduced themselves.", "",
            "## In their own words", "",
            "Verbatim, from maintainers' comments on the threads. Each one links to the comment.", ""]
    for q in quotes:
        out += quote_block(q)
    out += ["## The inductees", ""]
    for rank, i in enumerate(yes, 1):
        out += card(rank, i)
    out += ["## Honorable mentions", "",
            "<details>", f"<summary><b>{len(maybe)} borderline cases</b>: defensible on the merits,",
            "handled curtly or closed without a fix.</summary>", ""]
    for i in maybe:
        out.append(f"- **[#{i['number']} · {i['title']}]({i['html_url']})** "
                   f"({i['comments']} comments). {i['note']}")
    out += ["", "</details>", "", "---", "",
            "## How an issue gets in", "",
            "1. `tools/upstream_hall_of_fame.py` searches `obsproject/obs-studio` for",
            "   issues closed as *not planned* or locked, sorted by comments and reactions.",
            "2. It keeps those with at least 15 comments or 8 reactions.",
            "3. A person reads every remaining thread. Issues traced to a third-party",
            "   driver, plugin or distro are thrown out. The verdicts live in",
            "   `tools/upstream_hall_of_fame_verdicts.json`.", "",
            "Every claim links to the thread. Read it and judge for yourself.", "",
            "**Hit one of these? [OBS-Studio-Rust](../README.md) is where we fix them.**", ""]
    return "\n".join(out)


def render(rows: list[dict]) -> str:
    if any("tier" in i for i in rows):
        return render_hall(rows)
    lines = [
        "# Upstream OBS Studio: Hall of Fame",
        "",
        "Upstream `obsproject/obs-studio` issues where many users replied or",
        "upvoted and the issue was still closed as *not planned* and/or locked.",
        "Generated by `tools/upstream_hall_of_fame.py`; selection is mechanical",
        "(engagement plus how the issue was shut down), so read each thread",
        "before quoting it.",
        "",
        *table(rows),
    ]
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pages", type=int, default=2, help="pages of 100 per query")
    ap.add_argument("--min-comments", type=int, default=15)
    ap.add_argument("--min-reactions", type=int, default=8)
    ap.add_argument("--top", type=int, default=50)
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--cache", type=Path, help="reuse/save the raw search results")
    ap.add_argument("--digest", type=Path, help="write a compact per-issue digest for review")
    ap.add_argument("--keep", type=int, default=6, help="last N comments in the digest")
    ap.add_argument("--verdicts", type=Path,
                    help="JSON {number: [yes|borderline, note]} from reading the threads")
    args = ap.parse_args()

    seen = candidates(args.pages, args.cache)
    rows = sorted(
        (i for i in seen.values()
         if qualifies(i, args.min_comments, args.min_reactions)),
        key=score, reverse=True,
    )[: args.top]

    notes = json.loads(args.verdicts.read_text()) if args.verdicts else None
    if notes is not None:
        rows = [i for i in rows if str(i["number"]) in notes]
        for i in rows:
            i["tier"], i["note"] = notes[str(i["number"])]
    if args.digest:
        args.digest.write_text("\n".join(digest(i, args.keep) for i in rows))
        print(f"digest of {len(rows)} issues -> {args.digest}")
        return 0
    md = render(rows)
    if args.dry_run:
        sys.stdout.write(md)
    else:
        OUT.write_text(md, encoding="utf-8")
        print(f"{len(seen)} candidates, {len(rows)} listed -> {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
