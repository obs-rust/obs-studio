#!/usr/bin/env -S uv run --script
"""Claim bot for obs-rust/obs-studio (docs/rust-port/coordination.md §2-3).

Usage:
  claim_bot.py comment [--dry-run]   handle the issue_comment event in $GITHUB_EVENT_PATH
  claim_bot.py sweep [--dry-run]     expire stale claims

Needs the `gh` CLI, authenticated (GH_TOKEN in CI). Standard library only.

Quiet by design: the only action that notifies anyone is a rejection,
which replies to and mentions the person refused. Everything accepted is
the `claimed` label plus a 🚀 reaction on the command comment; expiry just
removes the label. Bot comments, assignments and @-mentions each sent a
phone notification, and one maintainer's agents make many claims.

Commands, read from the first line of an issue comment:
  Claiming: <item>                  claim the issue
  Releasing <item>                  the claimant gives it up
  Releasing <item> — handing it to @x, Handing <item> over to @x,
  It's yours, @x, /handoff [item] [to] @x
                                     the claimant hands the claim to @x instead
                                     of dropping it (the label stands)
  Taking over stale claim from @x   take a stale claim
  /reassign @y                      maintainers (OWNER/MEMBER/COLLABORATOR) move any claim

A claim goes stale after STALE_HOURS (CRITICAL_STALE_HOURS on issues labeled
`critical`) with no comment from the claimant on the issue and no commit,
comment or review on an open PR they opened that references the issue. The
hourly sweep then removes the label. Issues labeled `claim-locked` are left
alone.

State lives in the visible history: no `claimed` label means no claim;
otherwise the latest command the bot reacted to with REACTION decides (see
current_claim). Older bot comments that name the claimant still count, and
claims made before the bot are read from the latest `Claiming:` comment.
The functions above main() are pure and tested by ci/test_claim_bot.py.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone

REPO = "obs-rust/obs-studio"
BOT = "github-actions[bot]"
BOT_PREFIX = "**Claim bot:**"
CLAIMED = "claimed"
CRITICAL = "critical"
LOCKED = "claim-locked"
STALE_HOURS = 48
CRITICAL_STALE_HOURS = 8
MAINTAINERS = {"OWNER", "MEMBER", "COLLABORATOR"}
REACTION = "rocket"

CLAIM_RE = re.compile(r"^\s*claiming\b", re.I)
RELEASE_RE = re.compile(r"^\s*releasing\b", re.I)
TAKEOVER_RE = re.compile(r"^\s*taking over stale claim from @([A-Za-z0-9-]+)", re.I)
REASSIGN_RE = re.compile(r"^\s*/reassign\s+@([A-Za-z0-9-]+)", re.I)
# A handoff: the holder names who takes the claim, in one comment. This only
# recognises the shape of the line so the event gate routes it here; the
# target itself comes from handoff_target().
HANDOFF_RE = re.compile(
    r"^\s*(?:/?handoff\b|hand(?:ed|ing)\b|pass(?:ed|ing)\b|giv(?:e|ing)\b"
    r"|transfer(?:red|ring)\b|mov(?:e|ing)\b|it'?s\s+yours\b|\byours\b)", re.I)
# The verb pins the intent: `handing it (over) to @x`, `passed the work on to @x`.
# A release that merely names someone (`thanks to @x`) has no verb, so it stays
# a release rather than quietly keeping the claim on the wrong person.
HANDOFF_TO_RE = re.compile(
    r"\b(?:hand(?:ed|ing)|pass(?:ed|ing)|giv(?:e|ing)|transfer(?:red|ring)|mov(?:e|ing))"
    r"(?:\s+\S+){0,3}\s+to\s*@([A-Za-z0-9-]+)", re.I)
# `/handoff @x`, `/handoff <item> to @x`.
HANDOFF_CMD_RE = re.compile(r"^\s*/?handoff\b(?:\s+\S+){0,3}?\s*(?:to\s+)?@([A-Za-z0-9-]+)", re.I)
# `It's yours, @x`.
YOURS_RE = re.compile(r"\bit'?s\s+yours[,;:! ]+@([A-Za-z0-9-]+)", re.I)
# Everything that makes a comment a command, so the event gate and the
# granted-reaction reader cannot drift apart.
COMMANDS = (CLAIM_RE, RELEASE_RE, TAKEOVER_RE, REASSIGN_RE, HANDOFF_RE)
# The bot's own records. Older ones wrote @login; current ones `login`.
BOT_CLAIM_RE = re.compile(re.escape(BOT_PREFIX) + r" (?:claimed by|claim moved to) [@`]([A-Za-z0-9-]+)")
BOT_CLEAR_RE = re.compile(re.escape(BOT_PREFIX) + r" claim (?:released|expired)")


@dataclass
class Comment:
    author: str
    association: str
    body: str
    created: datetime
    id: int = 0
    granted: bool = False  # a Claiming: comment the bot reacted to

    def first_line(self) -> str:
        return self.body.strip().splitlines()[0] if self.body.strip() else ""


@dataclass
class LinkedPR:
    number: int
    author: str
    activity: list[datetime] = field(default_factory=list)


@dataclass
class Issue:
    number: int
    labels: set[str]
    assignees: list[str]
    comments: list[Comment]
    prs: list[LinkedPR] = field(default_factory=list)


@dataclass
class Claim:
    user: str
    since: datetime


def is_bot(login: str) -> bool:
    return login.endswith("[bot]")


def handoff_target(line: str) -> str | None:
    """The `@user` a handoff line names, or None when it names nobody."""
    if m := HANDOFF_CMD_RE.match(line):
        return m.group(1)
    if not (RELEASE_RE.match(line) or HANDOFF_RE.match(line)):
        return None
    for rx in (HANDOFF_TO_RE, YOURS_RE):
        if m := rx.search(line):
            return m.group(1)
    return None


def current_claim(issue: Issue) -> Claim | None:
    """The claimant. No `claimed` label means no claim (release and expiry
    only remove it). Otherwise the latest granted command decides: a granted
    `Claiming:` or takeover gives it to its author, `/reassign @x` to x, a
    granted handoff to the `@x` it names, a plain granted release clears it.
    Older bot comments (@login records) count too; before the bot, the latest
    `Claiming:` comment."""
    if CLAIMED not in issue.labels:
        return None
    claim: Claim | None = None
    seen_record = False
    for c in sorted(issue.comments, key=lambda c: c.created):
        line = c.first_line()
        if c.granted and not is_bot(c.author):
            seen_record = True
            if m := REASSIGN_RE.match(line):
                claim = Claim(m.group(1), c.created)
            elif RELEASE_RE.match(line) or HANDOFF_RE.match(line):
                target = handoff_target(line)
                if target and (claim is None or target.lower() != claim.user.lower()):
                    claim = Claim(target, c.created)      # a handoff moves it
                elif RELEASE_RE.match(line):
                    claim = None                          # a plain release clears it
            else:
                claim = Claim(c.author, c.created)
        elif c.author == BOT and (m := BOT_CLAIM_RE.match(c.body)):
            claim, seen_record = Claim(m.group(1), c.created), True
        elif c.author == BOT and BOT_CLEAR_RE.match(c.body):
            claim, seen_record = None, True
    if seen_record:
        return claim
    human = [c for c in issue.comments if not is_bot(c.author) and CLAIM_RE.match(c.first_line())]
    if not human:
        return None
    last = max(human, key=lambda c: c.created)
    return Claim(last.author, last.created)


def stale_hours(issue: Issue) -> int:
    return CRITICAL_STALE_HOURS if CRITICAL in issue.labels else STALE_HOURS


def last_activity(issue: Issue, claim: Claim) -> datetime:
    """Latest sign of the claimant working: the claim, their issue comments, their PRs."""
    times = [claim.since]
    times += [c.created for c in issue.comments if c.author.lower() == claim.user.lower()]
    for pr in issue.prs:
        if pr.author.lower() == claim.user.lower():
            times += pr.activity
    return max(times)


def is_stale(issue: Issue, claim: Claim, now: datetime) -> bool:
    return now - last_activity(issue, claim) >= timedelta(hours=stale_hours(issue))


def decide_comment(issue: Issue, comment: Comment, now: datetime) -> list[tuple]:
    """Actions for one new comment. Only a rejection comments (and mentions
    the person rejected); every accepted command is the label plus REACTION
    on the command, so nobody is notified. There is no per-account claim
    limit: several agents can share one maintainer's account, and expiry
    handles hoarding."""
    if is_bot(comment.author) or LOCKED in issue.labels:
        return []
    line = comment.first_line()
    claim = current_claim(issue)
    who = comment.author

    if m := REASSIGN_RE.match(line):
        if comment.association not in MAINTAINERS:
            return reject(who, "only maintainers can `/reassign`.")
        return accept(comment)

    if m := TAKEOVER_RE.match(line):
        if claim is None:
            return accept(comment)
        if claim.user.lower() != m.group(1).lower():
            return reject(who, f"the current claim is held by {u(claim.user)}, not {u(m.group(1))}.")
        if not is_stale(issue, claim, now):
            left = timedelta(hours=stale_hours(issue)) - (now - last_activity(issue, claim))
            return reject(who, f"the claim by {u(claim.user)} is still active; "
                          f"it goes stale in {fmt_hours(left)} without progress.")
        return accept(comment)

    if CLAIM_RE.match(line):
        if claim and claim.user.lower() == who.lower():
            return []
        if claim and not is_stale(issue, claim, now):
            return reject(who, f"this is already claimed by {u(claim.user)}. Pick something else or offer "
                          "help here; once the claim goes stale, comment "
                          f"`Taking over stale claim from @{claim.user}`.")
        return accept(comment)

    if (RELEASE_RE.match(line) or HANDOFF_RE.match(line)) and claim \
            and claim.user.lower() == who.lower():
        target = handoff_target(line)
        if target and target.lower() != claim.user.lower():
            return accept(comment)      # the label stands; current_claim moves it
        if RELEASE_RE.match(line):
            return [("label_remove", CLAIMED), ("react", comment.id)]
        return []
    return []


def u(login: str) -> str:
    """A login as code: GitHub sends no notification for it, unlike @login."""
    return f"`{login}`"


def accept(comment: Comment) -> list[tuple]:
    """Label and react: the record of the claim, and silent."""
    return [("label_add", CLAIMED), ("react", comment.id)]


def reject(who: str, why: str) -> list[tuple]:
    """The one action that notifies: the person whose command was refused."""
    return [("comment", f"{BOT_PREFIX} @{who}, {why}")]


def decide_sweep(issue: Issue, now: datetime) -> list[tuple]:
    """Expire a claim that went stale by removing the label, silently."""
    if LOCKED in issue.labels:
        return []
    claim = current_claim(issue)
    if claim is None:
        return []
    if now - last_activity(issue, claim) >= timedelta(hours=stale_hours(issue)):
        return [("label_remove", CLAIMED)]
    return []


def fmt_hours(d: timedelta) -> str:
    hours = max(0.0, d.total_seconds() / 3600)
    return f"{hours:.0f} h" if hours >= 1 else f"{max(1, round(hours * 60))} min"


# ---- GitHub I/O -------------------------------------------------------------


def gh_json(path: str, paginate: bool = True):
    args = ["gh", "api", "-H", "Accept: application/vnd.github+json", path]
    if paginate:
        args[2:2] = ["--paginate", "--slurp"]
    out = json.loads(subprocess.run(args, check=True, capture_output=True, text=True).stdout)
    if paginate:
        return [x for page in out for x in page]
    return out


def ts(s: str) -> datetime:
    return datetime.fromisoformat(s.replace("Z", "+00:00"))


def load_pr(number: int) -> LinkedPR:
    pr = gh_json(f"repos/{REPO}/pulls/{number}", paginate=False)
    activity = [ts(pr["created_at"])]
    for ev in gh_json(f"repos/{REPO}/issues/{number}/timeline?per_page=100"):
        if ev.get("event") == "committed":
            activity.append(ts(ev["committer"]["date"]))
        elif ev.get("event") in ("commented", "reviewed", "ready_for_review", "head_ref_force_pushed"):
            stamp = ev.get("created_at") or ev.get("submitted_at")
            actor = (ev.get("user") or ev.get("actor") or {}).get("login", "")
            if stamp and actor.lower() == pr["user"]["login"].lower():
                activity.append(ts(stamp))
    return LinkedPR(number, pr["user"]["login"], activity)


def load_issue(number: int) -> Issue:
    raw = gh_json(f"repos/{REPO}/issues/{number}", paginate=False)
    comments = []
    for c in gh_json(f"repos/{REPO}/issues/{number}/comments?per_page=100"):
        com = Comment(c["user"]["login"], c["author_association"], c["body"] or "", ts(c["created_at"]), c["id"])
        line = com.first_line()
        command = any(r.match(line) for r in COMMANDS)
        if not is_bot(com.author) and command and c.get("reactions", {}).get(REACTION):
            reactions = gh_json(f"repos/{REPO}/issues/comments/{c['id']}/reactions?content={REACTION}&per_page=100")
            com.granted = any(r["user"]["login"] == BOT for r in reactions)
        comments.append(com)
    prs = []
    for ev in gh_json(f"repos/{REPO}/issues/{number}/timeline?per_page=100"):
        src = (ev.get("source") or {}).get("issue") or {}
        if ev.get("event") == "cross-referenced" and src.get("pull_request") and src.get("state") == "open" \
                and src.get("repository", {}).get("full_name", REPO) == REPO:
            prs.append(load_pr(src["number"]))
    return Issue(number, {l["name"] for l in raw["labels"]}, [a["login"] for a in raw["assignees"]], comments, prs)


def claimed_issue_numbers() -> list[int]:
    items = gh_json(f"repos/{REPO}/issues?state=open&labels={CLAIMED}&per_page=100")
    return [i["number"] for i in items if "pull_request" not in i]


def apply(number: int, actions: list[tuple], dry_run: bool) -> None:
    for kind, arg in actions:
        print(f"#{number}: {kind} {arg if kind != 'comment' else arg[:80]!r}")
        if dry_run:
            continue
        base = ["gh", "issue", "edit", str(number), "--repo", REPO]
        cmd = {
            "label_add": base + ["--add-label", arg],
            "label_remove": base + ["--remove-label", arg],
            "react": ["gh", "api", "-X", "POST", f"repos/{REPO}/issues/comments/{arg}/reactions",
                      "-f", f"content={REACTION}"],
            "comment": ["gh", "issue", "comment", str(number), "--repo", REPO, "--body", arg],
        }[kind]
        subprocess.run(cmd, check=True, stdout=subprocess.DEVNULL)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["comment", "sweep"])
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    now = datetime.now(timezone.utc)

    if args.mode == "comment":
        with open(os.environ["GITHUB_EVENT_PATH"], encoding="utf-8") as f:
            event = json.load(f)
        if "pull_request" in event["issue"]:
            return 0
        c = event["comment"]
        comment = Comment(c["user"]["login"], c["author_association"], c["body"] or "", ts(c["created_at"]), c["id"])
        line = comment.first_line()
        if is_bot(comment.author) or not any(r.match(line) for r in COMMANDS):
            return 0
        number = event["issue"]["number"]
        issue = load_issue(number)
        # The new comment is in the issue's history; decide as of before it.
        issue.comments = [x for x in issue.comments if x.created < comment.created]
        apply(number, decide_comment(issue, comment, now), args.dry_run)
        return 0

    for number in claimed_issue_numbers():
        apply(number, decide_sweep(load_issue(number), now), args.dry_run)
    return 0


if __name__ == "__main__":
    sys.exit(main())
