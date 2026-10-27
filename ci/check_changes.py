#!/usr/bin/env -S uv run --script
"""Resolve the files a step should look at, against the right git reference (#962).

Ports the shell step of `.github/actions/check-changes/action.yaml` into Python so
the composite action makes one call instead of running a 50-line shell block
(GEN-005's shell budget). The base-reference resolution is unchanged:

- an explicit `baseRef` is used as given; when it is not a commit in this
  checkout, a `::warning::` is printed and (with `useFallback: true`, the
  default) the comparison falls back to `HEAD~1`;
- with no `baseRef`, `HEAD~1` is the starting point, then the event decides:
  `pull_request` compares against `origin/<base branch>`, `push` against
  `github.event.before` (unless the push was forced), and every other event
  (`workflow_dispatch`, `schedule`, the merge queue) against the merge base with
  the default branch, so every commit the branch adds is checked (#13);
- a `push` whose `before` is not in the checkout compares against the empty
  tree (`4b825dc642cb6eb9a060e54bf8d69288fbee4904`).

`changedFiles` is the same compact JSON list the shell wrote (`["a","b"]`), and
`hasChangedFiles` the same `true`/`false`; both go to `$GITHUB_OUTPUT`.

Two deliberate differences from the shell, both in the callers' favour:

- the glob is a git pathspec handed to `git diff` as one argument, so a file
  name with a space stays one entry (the shell word-split it);
- the one extglob call site, `!(cmake*)` in `push.yaml`, becomes git's own
  exclusion pathspec `:(exclude,top)cmake*` -- the same "everything except the
  top-level cmake* entries" the shell expanded to, without needing bash.

    uv run --no-project --script ci/check_changes.py --check-glob 'plugins/rtmp-services/data/*.json'

Standard library only: the callers install nothing before this step.
"""

from __future__ import annotations

import argparse
import json
import os
from collections.abc import Mapping, Sequence
import subprocess
from pathlib import Path

DESCRIPTION = 'Resolve the files changed against a base git reference.'

# `git hash-object -t tree /dev/null`: the empty tree, the last-resort ref that
# is always valid, used when a push has no usable `before` commit.
EMPTY_TREE = '4b825dc642cb6eb9a060e54bf8d69288fbee4904'


def git(*arguments: str, cwd: Path) -> subprocess.CompletedProcess[str]:
    """Run git, capturing both streams."""
    return subprocess.run(
        ['git', *arguments],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
    )


def ref_exists(ref: str, cwd: Path) -> bool:
    """True when `ref` names an object in this checkout (`git cat-file -e`)."""
    if not ref:
        return False
    return git('cat-file', '-e', ref, cwd=cwd).returncode == 0


def merge_base(revision: str, cwd: Path) -> str | None:
    """The merge base of `HEAD` and `revision`, or None when there is none."""
    result = git('merge-base', 'HEAD', revision, cwd=cwd)
    found = result.stdout.strip()
    return found if result.returncode == 0 and found else None


def head_commit(cwd: Path) -> str | None:
    """The commit `HEAD` points at."""
    result = git('rev-parse', 'HEAD', cwd=cwd)
    found = result.stdout.strip()
    return found if result.returncode == 0 else None


def resolve_base_ref(environment: Mapping[str, str], cwd: Path) -> str:
    """The reference to diff against, following the shell's event rules."""
    base_ref = environment.get('GIT_BASE_REF', '')
    if base_ref:
        if ref_exists(base_ref, cwd):
            return base_ref
        print(f'::warning::Provided base reference {base_ref} is invalid')
        if environment.get('USE_FALLBACK', '') == 'true':
            return 'HEAD~1'
        return base_ref

    before = environment.get('GITHUB_REF_BEFORE', '')
    if not ref_exists(before, cwd):
        before = EMPTY_TREE

    event = environment.get('GITHUB_EVENT_NAME', '')
    if event == 'pull_request':
        return f'origin/{environment.get("GITHUB_BASE_REF", "")}'
    if event == 'push':
        if environment.get('GITHUB_EVENT_FORCED', '') != 'true':
            return before
        return 'HEAD~1'

    default_branch = environment.get('DEFAULT_BRANCH', '')
    if default_branch:
        found = merge_base(f'origin/{default_branch}', cwd)
        if found is not None and found != head_commit(cwd):
            return found
    return 'HEAD~1'


def pathspec(check_glob: str) -> list[str]:
    """The `git diff` pathspec for the action's `checkGlob` input.

    Empty means every path. An extglob negation (`!(cmake*)`) becomes git's
    exclusion form so it keeps excluding the same top-level entries without a
    shell to expand it.
    """
    if not check_glob:
        return []
    if check_glob.startswith('!(') and check_glob.endswith(')'):
        return [f':(exclude,top){check_glob[2:-1]}']
    return [check_glob]


def changed_files(
    base_ref: str,
    revision: str,
    *,
    diff_filter: str,
    check_glob: str,
    cwd: Path,
) -> list[str]:
    """The changed paths between `base_ref` and `revision`, one per entry."""
    paths = pathspec(check_glob)
    arguments = [
        'diff',
        '--name-only',
        f'--diff-filter={diff_filter}',
        base_ref,
        revision,
        '--',
        *paths,
    ]
    result = git(*arguments, cwd=cwd)
    if result.returncode != 0:
        raise SystemExit(
            f'git {" ".join(arguments)}: {result.stderr.strip() or "failed"}'
        )
    return [line for line in result.stdout.splitlines() if line]


def check(
    *,
    check_glob: str,
    cwd: Path,
    environment: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """The action's step outputs for this checkout."""
    settings = os.environ if environment is None else environment
    base_ref = resolve_base_ref(settings, cwd)
    files = changed_files(
        base_ref,
        settings.get('GIT_REF', 'HEAD') or 'HEAD',
        diff_filter=settings.get('DIFF_FILTER', ''),
        check_glob=check_glob,
        cwd=cwd,
    )
    return {
        'hasChangedFiles': 'true' if files else 'false',
        'changedFiles': json.dumps(files, separators=(',', ':')),
    }


def write_outputs(outputs: Mapping[str, str], path: str | None) -> None:
    """Append `name=value` lines to the step's `$GITHUB_OUTPUT` file."""
    if not path:
        return
    with open(path, 'a', encoding='utf-8') as handle:
        for name, value in outputs.items():
            handle.write(f'{name}={value}\n')


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=DESCRIPTION)
    parser.add_argument(
        '--check-glob',
        default='',
        help="the action's checkGlob input: a git pathspec, empty for every path",
    )
    parser.add_argument(
        '--repository',
        default='.',
        help='checkout to run git in (default: the working directory)',
    )
    arguments = parser.parse_args(argv)
    outputs = check(
        check_glob=arguments.check_glob,
        cwd=Path(arguments.repository),
    )
    write_outputs(outputs, os.environ.get('GITHUB_OUTPUT'))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
