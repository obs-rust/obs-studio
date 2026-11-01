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

`collect` takes every issue and PR comment, inline PR review comments
included, by a GitHub `User` account that is not a maintainer (OWNER, MEMBER,
COLLABORATOR) and not on the contributors leaderboard. It removes code blocks
(fenced, indented and `<pre>`), `<details>` logs, HTML comments and quoted
replies, and leaves out comments already in tools/user_quotes.json, so a
rejected quote stays rejected. It reads only comments updated since the last
recorded scan. The candidates file carries RULE, the judging rule.

`record` takes the quote text from the candidates file, never from the
verdicts, and checks it against the live comment, so a quote is always the
comment's own words. Every candidate needs a verdict.

`render` puts every approved quote in the README, newest first, with no cap:
past the newest SHOWN, the older ones go in a collapsed <details> block, and
none is ever dropped for space.

tools/user_quotes.json is read from master by other tools (read-only), so its
format stays stable. It is one JSON object:

    {"scanned_through": "<ISO 8601 UTC time of the last collect>",
     "quotes": [<entry>, ...]}             # oldest first, one entry per line

and every entry has the same five fields, in this order:

    text    the quote, word for word ("" when rejected)
    login   the author's GitHub login ("" when rejected)
    url     the comment's link: .../issues/N#issuecomment-ID, .../pull/N#issuecomment-ID,
            or .../pull/N#discussion_rID for an inline review comment
    date    the day the comment was posted, YYYY-MM-DD
    status  "approved" or "rejected"

plus `issue` (the issue or PR number) and, on rejected entries, `reason`. A
reader that wants the quotes keeps the entries whose status is "approved".
Rejected entries keep no text or login, so the repo names nobody whose
comment was turned down (some asked not to be mentioned); the url alone keeps
them rejected.

To strike an approved quote, set its status to "rejected", empty its text and
login, add a reason, and run `render`; to look at a rejected comment again,
delete its line and run `collect --all`.

The repo's CI has no model access, so the reviewer is the coordinator agent
(obs-coordinator skill, section 4a) or a person. Standard library and `gh`.
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
FIELDS = ("text", "login", "url", "date", "status")  # every entry, in this order (the header)
SHOWN = 8  # shown in full; older approved quotes fold into <details>, never dropped

MAINTAINERS = {"OWNER", "MEMBER", "COLLABORATOR"}
# Agent and bot accounts that are typed "User" (the leaderboard's list, tools/leaderboard/generate.py).
BOT_RE = re.compile(r"(\[bot\]$|^(claude|copilot|dependabot|github-actions|coderabbit)|anthropic)", re.I)
BLOCKS = re.compile(r"```.*?(?:```|\Z)|~~~.*?(?:~~~|\Z)|<details\b[^>]*>.*?(?:</details>|\Z)"
                    r"|<pre\b[^>]*>.*?(?:</pre>|\Z)|<!--.*?(?:-->|\Z)", re.S | re.I)
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


LIST_ITEM = re.compile(r"^( *)([-*+]|\d{1,9}[.)])( {1,4})(?=\S)")
# Blocks that end a list even with no blank line before them: a heading, a
# thematic break or a block quote.
ENDS_LIST = re.compile(r"#{1,6}(?:\s|$)|>|([-*_])(?:[ \t]*\1){2,}[ \t]*$")


def unindent_code(text: str) -> str:
    """Drop Markdown indented code blocks, as CommonMark reads them.

    A line is code when it is indented 4 or more columns past the enclosing
    list item's content (or the margin, outside a list) and starts after a
    blank line, since indented code cannot interrupt a paragraph. A
    paragraph indented under a list item is that item's content and stays.
    """
    out, in_code, blank, item = [], False, True, None  # item: content column of the open list item
    for line in text.expandtabs(4).splitlines():
        if not line.strip():
            blank = True
            out.append(line)
            continue
        width = len(line) - len(line.lstrip(" "))
        ends = ENDS_LIST.match(line, width)
        if item is not None and width < item and (blank or ends):
            item = None  # left of the item's content, after a blank line or as one of those blocks
        base = item or 0
        if width < base + 4 and not ends and (m := LIST_ITEM.match(line)):  # "- - -" is a rule
            item, in_code, blank = len(m.group(0)), False, False
            out.append(line)
            continue
        in_code = width >= base + 4 and (in_code or blank)
        blank = False
        if not in_code:
            out.append(line)
    return "\n".join(out)


def clean(body: str | None) -> str:
    """The comment as written, minus code and log blocks, HTML comments and quoted replies."""
    return " ".join(QUOTED.sub(" ", unindent_code(BLOCKS.sub(" ", body or ""))).split())


def is_candidate(comment: dict, excluded: set[str]) -> bool:
    user = comment.get("user") or {}
    login = user.get("login") or ""
    return (user.get("type") == "User" and not BOT_RE.search(login)
            and comment.get("author_association") not in MAINTAINERS
            and login.lower() not in excluded)


def candidates(comments: list[dict], excluded: set[str], known: set[str],
               max_chars: int = 0) -> list[dict]:
    """Candidates from raw REST comments, oldest first; `excluded` holds lowercase logins.

    `max_chars` (0: no limit) is only for a quick look; the seed and the weekly refresh use none.
    """
    excluded = {e.lower() for e in excluded}
    out = []
    for c in comments:
        if c["html_url"] in known or not is_candidate(c, excluded):
            continue
        text = clean(c.get("body"))
        if not text or (max_chars and len(text) > max_chars):
            continue
        out.append({"url": c["html_url"], "login": c["user"]["login"], "date": c["created_at"][:10],
                    "issue": int((c.get("issue_url") or c["pull_request_url"]).rsplit("/", 1)[1]), "text": text})
    return sorted(out, key=lambda q: (q["date"], q["url"]))


def record(data: dict, found: dict, verdicts: dict, live: dict[str, str]) -> dict:
    """Merge verdicts on a candidates file into the data. `live` maps url -> current cleaned text."""
    known = {q["url"] for q in data["quotes"]}
    pending = {c["url"]: c for c in found["candidates"]}
    if missing := sorted(set(pending) - set(verdicts)):
        raise ValueError(f"no verdict for {len(missing)} candidate(s): {missing[0]} ...")
    if extra := sorted(set(verdicts) - set(pending)):
        raise ValueError(f"verdict for a comment that is not a candidate: {extra[0]}")
    quotes = list(data["quotes"])
    for url, c in pending.items():
        if url in known:
            continue  # judged before: the earlier verdict stands
        status, reason = verdicts[url]
        entry = {"text": "", "login": "", "url": url, "date": c["date"], "status": status, "issue": c["issue"]}
        if status == "approved":
            if live.get(url) != c["text"]:
                raise ValueError(f"{url}: the comment changed since collect; collect again")
            entry |= {"text": c["text"], "login": c["login"]}
        elif status == "rejected":
            if not reason:
                raise ValueError(f"{url}: a rejection needs a reason")
            entry["reason"] = reason  # no text or login: the url alone keeps it rejected
        else:
            raise ValueError(f"{url}: status must be approved or rejected, not {status!r}")
        quotes.append(entry)
    quotes.sort(key=lambda q: (q["date"], q["url"]))
    return {"scanned_through": max(data.get("scanned_through") or "", found["scanned_through"]),
            "quotes": quotes}


def section(data: dict, shown: int = SHOWN) -> str:
    approved = [q for q in data["quotes"] if q["status"] == "approved"]
    approved.sort(key=lambda q: (q["date"], q["url"]), reverse=True)
    quote = lambda q: [f"> “{q['text']}”", ">",
                       f"> — [{q['login']}](https://github.com/{q['login']}), "
                       f"[#{q['issue']}]({q['url']}), {q['date']}", ""]
    lines = [START,
             "<!-- Generated by tools/user_quotes.py from tools/user_quotes.json; edit those, not this. -->", ""]
    for q in approved[:shown]:
        lines += quote(q)
    if older := approved[shown:]:
        lines += ["<details>", f"<summary>{len(older)} older quote{'s' * (len(older) != 1)}</summary>", ""]
        for q in older:
            lines += quote(q)
        lines += ["</details>", ""]
    if not approved:
        lines += ["No quotes yet.", ""]
    lines.append(END)
    return "\n".join(lines)


def render(readme: str, data: dict, shown: int = SHOWN) -> str:
    start, end = readme.find(START), readme.find(END)
    if start < 0 or end < start:
        raise ValueError(f"README needs {START} ... {END}")
    return readme[:start] + section(data, shown) + readme[end + len(END):]


def load(path: Path) -> dict:
    return json.loads(path.read_text()) if path.exists() else {"scanned_through": "", "quotes": []}


def dump(data: dict) -> str:
    """One quote per line, so a diff shows exactly which verdicts changed."""
    rows = ",\n".join("    " + json.dumps(q, ensure_ascii=False) for q in data["quotes"])
    quotes = f"[\n{rows}\n  ]" if rows else "[]"
    return f'{{\n  "scanned_through": {json.dumps(data["scanned_through"])},\n  "quotes": {quotes}\n}}\n'


def gh_lines(path: str) -> list[dict]:
    r = subprocess.run(["gh", "api", "--paginate", path, "--jq", ".[]"], capture_output=True, text=True)
    if r.returncode:
        raise SystemExit(f"gh api {path}: {r.stderr.strip()}")
    return [json.loads(line) for line in r.stdout.splitlines() if line.strip()]


def contributors() -> set[str]:
    r = subprocess.run(["gh", "api", "-H", "Accept: application/vnd.github.raw",
                        f"repos/{REPO}/contents/leaderboard.json?ref=leaderboard"],
                       capture_output=True, text=True)
    if r.returncode:
        raise SystemExit(f"leaderboard.json: {r.stderr.strip()}")
    return {c["login"].lower() for c in json.loads(r.stdout)["contributors"]}


def fetch_comments(since: str) -> list[dict]:
    """Issue and PR conversation comments, then inline PR review comments (a separate API)."""
    query = f"per_page=100&since={since}" if since else "per_page=100"
    return (gh_lines(f"repos/{REPO}/issues/comments?{query}")
            + gh_lines(f"repos/{REPO}/pulls/comments?{query}"))


def live_text(url: str) -> str:
    kind, cid = ("pulls", url.rsplit("discussion_r", 1)[1]) if "#discussion_r" in url \
        else ("issues", url.rsplit("issuecomment-", 1)[1])
    r = subprocess.run(["gh", "api", f"repos/{REPO}/{kind}/comments/{cid}", "--jq", ".body"],
                       capture_output=True, text=True)
    if r.returncode:
        raise SystemExit(f"{url}: {r.stderr.strip()}")
    return clean(r.stdout)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("collect", help="write new candidates for review")
    c.add_argument("--out", type=Path, required=True)
    c.add_argument("--max-chars", type=int, default=0, help="skip longer comments (default: no limit)")
    c.add_argument("--all", action="store_true", help="scan every comment, not only those since the last scan")
    r = sub.add_parser("record", help="merge verdicts into tools/user_quotes.json")
    r.add_argument("candidates", type=Path)
    r.add_argument("verdicts", type=Path)
    p = sub.add_parser("render", help="rewrite the README section")
    p.add_argument("--check", action="store_true", help="fail if the README is out of date")
    args = ap.parse_args()

    data = load(DATA)
    if args.cmd == "collect":
        now = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        comments = fetch_comments("" if args.all else data["scanned_through"])
        found = candidates(comments, contributors(), {q["url"] for q in data["quotes"]}, args.max_chars)
        args.out.write_text(json.dumps({"rule": RULE, "scanned_through": now, "candidates": found},
                                       indent=2, ensure_ascii=False) + "\n")
        print(f"{len(comments)} comments, {len(found)} new candidates -> {args.out}")
        return 0
    if args.cmd == "record":
        found = json.loads(args.candidates.read_text())
        verdicts = json.loads(args.verdicts.read_text())
        live = {u: live_text(u) for u, (s, _) in verdicts.items() if s == "approved"}
        data = record(data, found, verdicts, live)
        DATA.write_text(dump(data))
        print(f"{len(found['candidates'])} judged -> {DATA.relative_to(ROOT)}")
    text = README.read_text()
    out = render(text, data)
    if args.cmd == "render" and args.check:
        if out != text:
            print("README.md user quotes are stale: run uv run tools/user_quotes.py render", file=sys.stderr)
            return 1
        return 0
    README.write_text(out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
