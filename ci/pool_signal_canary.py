#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# ///
"""Fail loudly when the pool signal is readable but useless.

`forge-ci/ci.yml/actions/pool-select` decides between the Dell pool and the
hosted fallback from `https://signal.forge-ci.com/v1/pool`. When an org's
`dispatcher` section is `null` the action adds no room and falls back to
hosted — deliberately, because a selector must never fail a merge. The cost is
that an outage is indistinguishable from "the pool was full": on 2026-11-24 a
~30 minute flap on `obs-rust` silently turned a merge-queue Tier 2 build into
the 14m48s slow path while the payload still said `state: ok, error: null`
(forge-ci/gh-runner#90).

This probe is the missing alarm. It fetches the same document the selector
reads and reports every reason the document cannot justify routing, so a flap
shows up as a red scheduled run instead of a slow build nobody attributes.

    uv run ci/pool_signal_canary.py [--org ORG ...] [--max-age SECONDS]
"""
from __future__ import annotations

import argparse
import json
import sys
import urllib.request

URL = 'https://signal.forge-ci.com/v1/pool'
# The orgs whose lanes this repository can route to. Anything else in the
# payload is reported for context but does not fail the probe.
DEFAULT_ORGS = ('obs-rust',)
# pool-select treats a dispatcher report older than this as no room.
DEFAULT_MAX_AGE = 180
LOWER_IS_WORSE = 'floor-0 lanes need dispatcher room; idle runners are normally 0'


def fetch(url: str = URL, timeout: int = 15) -> dict:
    with urllib.request.urlopen(url, timeout=timeout) as response:
        return json.loads(response.read(1_000_000))


def online_runners(entry: dict) -> int:
    """Runners the org can offer right now, whatever their labels."""
    return sum(int(group.get('online') or 0) for group in entry.get('runners') or []
               if isinstance(group, dict))


def lanes(entry: dict) -> list[dict]:
    dispatcher = entry.get('dispatcher')
    hosts = dispatcher.get('hosts') if isinstance(dispatcher, dict) else None
    return [host for host in hosts or [] if isinstance(host, dict)]


def evaluate(payload: dict, org: str, max_age: int = DEFAULT_MAX_AGE) -> list[str]:
    """Every reason ORG's entry cannot justify routing to the pool.

    An empty list means the document is actionable: a dispatcher reported, at
    least one lane is fresh, and its spare room is a non-negative integer.
    """
    problems: list[str] = []
    entry = (payload.get('orgs') or {}).get(org)
    if not isinstance(entry, dict):
        return [f'{org}: missing from the payload']
    if entry.get('state') not in (None, 'ok'):
        problems.append(f'{org}: state is {entry.get("state")!r}, error {entry.get("error")!r}')
    dispatcher = entry.get('dispatcher')
    if dispatcher is None:
        # The exact flap: runners are visible, the dispatcher is not. A
        # selector reads that as "no room" and silently stays hosted.
        online = online_runners(entry)
        detail = f' while {online} runner(s) report online' if online else ''
        problems.append(f'{org}: dispatcher is null{detail} — pool-select can add no room')
        return problems
    hosts = lanes(entry)
    if not hosts:
        problems.append(f'{org}: dispatcher reports no lanes')
        return problems
    fresh = 0
    for host in hosts:
        name = host.get('host', '<unnamed>')
        spare, age = host.get('spare'), host.get('age_s')
        if not isinstance(host.get('labels'), list) or not host['labels']:
            problems.append(f'{org}/{name}: lane has no labels')
            continue
        if type(spare) is not int or spare < 0:
            problems.append(f'{org}/{name}: spare is {spare!r}, not a non-negative integer')
            continue
        if not isinstance(age, (int, float)) or isinstance(age, bool) or age > max_age:
            problems.append(f'{org}/{name}: report is {age!r}s old, older than {max_age}s')
            continue
        fresh += 1
    if hosts and not fresh:
        problems.append(f'{org}: no lane is both fresh and well-formed — {LOWER_IS_WORSE}')
    return problems


def summarise(payload: dict, orgs: tuple[str, ...], max_age: int) -> tuple[list[str], list[str]]:
    """One human line per org, then the problems."""
    lines = [f'signal generated_at {payload.get("generated_at", "?")}']
    for org in orgs:
        entry = (payload.get('orgs') or {}).get(org)
        if not isinstance(entry, dict):
            lines.append(f'  {org:10} missing')
            continue
        found = lanes(entry)
        detail = 'dispatcher=null' if entry.get('dispatcher') is None else ', '.join(
            f'{host.get("host")} spare={host.get("spare")} age={host.get("age_s")}s' for host in found) or 'no lanes'
        lines.append(f'  {org:10} runners={online_runners(entry):2}  {detail}')
    problems = [problem for org in orgs for problem in evaluate(payload, org, max_age)]
    lines.extend(f'FAIL {problem}' for problem in problems)
    lines.append('signal is actionable' if not problems else f'{len(problems)} problem(s)')
    return lines, problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--org', action='append', dest='orgs', default=None)
    parser.add_argument('--max-age', type=int, default=DEFAULT_MAX_AGE)
    args = parser.parse_args()
    orgs = tuple(args.orgs or DEFAULT_ORGS)
    try:
        payload = fetch()
    except Exception as error:
        print(f'FAIL cannot read {URL}: {type(error).__name__}: {error}')
        return 1
    lines, problems = summarise(payload, orgs, args.max_age)
    print('\n'.join(lines))
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
