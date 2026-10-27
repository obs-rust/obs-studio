#!/usr/bin/env -S uv run --script
"""Retain selected paths when pending master Push builds are coalesced."""
import json
import os
import subprocess

# setup-soldr exports these through GITHUB_ENV; gh then colourises --json.
_COLOUR = ("CLICOLOR_FORCE", "FORCE_COLOR", "GH_FORCE_TTY")


def plain_env(environ=None):
    """The environment a `gh --json` call is parsed from: never coloured."""
    env = {k: v for k, v in (os.environ if environ is None else environ).items() if k not in _COLOUR}
    env["NO_COLOR"] = "1"
    return env


def baseline_covers_coalesced_pushes(sha, run):
    """Old coalesced runs can be green while omitting superseded paths."""
    push = run(['git', 'show', f'{sha}:.github/workflows/push.yaml'],
               capture_output=True, text=True)
    if push.returncode != 0:
        return False
    if 'group: push-build-' not in push.stdout:
        return True  # Before coalescing, successful pushes did not skip predecessors.
    helper = run(['git', 'show', f'{sha}:ci/master_scope_base.py'],
                 capture_output=True, text=True)
    return helper.returncode == 0 and 'def baseline_covers_coalesced_pushes(' in helper.stdout


def scope_base(run=subprocess.run):
    try:
        result = run(
            ['gh', 'run', 'list', '--workflow', 'push.yaml', '--branch', 'master',
             '--event', 'push', '--status', 'success', '--limit', '30',
             '--json', 'headSha'], check=True, capture_output=True, text=True,
            env=plain_env(),
        )
        for item in json.loads(result.stdout):
            sha = item['headSha']
            if len(sha) != 40 or any(c not in '0123456789abcdef' for c in sha):
                continue
            if run(['git', 'merge-base', '--is-ancestor', sha, 'HEAD'],
                   capture_output=True).returncode == 0 and baseline_covers_coalesced_pushes(sha, run):
                return sha
    except (subprocess.CalledProcessError, ValueError, KeyError):
        pass
    # No trustworthy baseline: compare the entire tree. Never silently drop
    # paths because the API is unavailable or recent runs were unsuccessful.
    return 'ALL'


if __name__ == '__main__':
    print(scope_base())
