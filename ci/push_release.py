#!/usr/bin/env -S uv run --no-project --script
"""`push.yaml`'s release steps are one Python call each (#962, GEN-005).

`.github/workflows/push.yaml` carried five multi-line `run:` blocks -- the two
`deploy-documentation` shell steps and the three `create-release` ones -- so
`ci-lint precheck` reported a GEN-005 finding for each. Each step is now one
call:

    uv run --no-project --script ci/push_release.py commit-hash
    uv run --no-project --script ci/push_release.py set-redirects --project <name>
    uv run --no-project --script ci/push_release.py check-tag
    uv run --no-project --script ci/push_release.py rename-artifacts --version <v>
    uv run --no-project --script ci/push_release.py checksums

Each keeps its step's behaviour exactly:

- `commit-hash` writes `commitHash` (`GITHUB_SHA`'s first nine characters) to
  `$GITHUB_OUTPUT`.
- `set-redirects` appends the two `/previous/...` lines to `docs/_redirects`;
  the second names `--project`, the step's `vars.CF_PAGES_PROJECT`.
- `check-tag` translates the step's `shopt -s extglob` `case` to two anchored
  regular expressions: `<digits>.<digits>.<digits>` and the same with a
  `-beta`/`-rc` suffix and optional trailing digits. Everything else writes
  only `validTag=false`, as the `*)` arm did.
- `rename-artifacts` performs the twelve `mv -v` moves, globbing each artifact
  directory and failing on zero or several matches the way `mv` did.
- `checksums` writes `CHECKSUMS.txt`: the `### Checksums` header, then one
  sha256 line per `*.deb`, `*.ddeb`, `*.dmg`, `*.tar.xz`, `*.tar.gz`, `*.exe`
  and `*.zip` in the workspace root, in glob (lexical) order.

`GITHUB_SHA`, `GITHUB_REF_NAME`, `GITHUB_WORKSPACE` and `GITHUB_OUTPUT` arrive
from the runner; nothing here re-implements an authentication path.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import sys

REDIRECTS = (
    '/previous/27.2 https://obsproject.com/docs/27.2 302',
    '/previous/:major.:minor https://:major-:minor.{project}.pages.dev 302',
)
RELEASE_TAG = re.compile(r'\d+\.\d+\.\d+')
PRERELEASE_TAG = re.compile(r'\d+\.\d+\.\d+-(?:beta|rc)\d*')
CHECKSUM_PATTERNS = (
    '*.deb',
    '*.ddeb',
    '*.dmg',
    '*.tar.xz',
    '*.tar.gz',
    '*.exe',
    '*.zip',
)
# (artifact directory, artifact glob, release name) -- the twelve `mv` pairs,
# with `{commit}` and `{version}` standing in for the step's variables.
ARTIFACTS = (
    (
        'obs-studio-macos-arm64-{commit}',
        'obs-studio-*-macos-apple.dmg',
        'OBS-Studio-{version}-macOS-Apple.dmg',
    ),
    (
        'obs-studio-macos-arm64-{commit}-dSYMs',
        'obs-studio-*-macos-apple-dSYMs.tar.xz',
        'OBS-Studio-{version}-macOS-Apple-dSYMs.tar.xz',
    ),
    (
        'obs-studio-macos-x86_64-{commit}',
        'obs-studio-*-macos-intel.dmg',
        'OBS-Studio-{version}-macOS-Intel.dmg',
    ),
    (
        'obs-studio-macos-x86_64-{commit}-dSYMs',
        'obs-studio-*-macos-intel-dSYMs.tar.xz',
        'OBS-Studio-{version}-macOS-Intel-dSYMs.tar.xz',
    ),
    (
        'obs-studio-ubuntu-26.04-x86_64-{commit}',
        'obs-studio-*-x86_64-ubuntu-gnu.deb',
        'OBS-Studio-{version}-Ubuntu-26.04-x86_64.deb',
    ),
    (
        'obs-studio-ubuntu-26.04-x86_64-{commit}-dbgsym',
        'obs-studio-*-x86_64-ubuntu-gnu-dbgsym.ddeb',
        'OBS-Studio-{version}-Ubuntu-26.04-x86_64-dbsym.ddeb',
    ),
    (
        'obs-studio-ubuntu-26.04-sources-{commit}',
        'obs-studio-*-sources.tar.gz',
        'OBS-Studio-{version}-Sources.tar.gz',
    ),
    (
        'obs-studio-windows-x64-{version}-signed',
        'OBS-Studio-*.zip',
        'OBS-Studio-{version}-Windows-x64.zip',
    ),
    (
        'obs-studio-windows-x64-{version}-installer',
        'OBS-Studio-*.exe',
        'OBS-Studio-{version}-Windows-x64-Installer.exe',
    ),
    (
        'obs-studio-windows-x64-{version}-pdbs',
        'OBS-Studio-*-pdbs.zip',
        'OBS-Studio-{version}-Windows-x64-PDBs.zip',
    ),
    (
        'obs-studio-windows-arm64-{version}-signed',
        'OBS-Studio-*.zip',
        'OBS-Studio-{version}-Windows-arm64.zip',
    ),
    (
        'obs-studio-windows-arm64-{version}-pdbs',
        'OBS-Studio-*-pdbs.zip',
        'OBS-Studio-{version}-Windows-arm64-PDBs.zip',
    ),
)


class Failure(Exception):
    """A step's non-zero exit, carrying its status."""

    def __init__(self, message: str, status: int = 1) -> None:
        super().__init__(message)
        self.status = status


def fail(message: str) -> None:
    """Print one runner error annotation."""

    print(f'::error::{message}', flush=True)


def workspace() -> Path:
    value = os.environ.get('GITHUB_WORKSPACE', '')
    if not value:
        raise Failure('GITHUB_WORKSPACE is not set; this step must run in a workflow')
    return Path(value)


def variable(name: str) -> str:
    value = os.environ.get(name, '')
    if not value:
        raise Failure(f'{name} is not set; this step must run in a workflow')
    return value


def append(sink: str, line: str) -> None:
    """Append one line to a GitHub file such as `$GITHUB_OUTPUT`."""

    path = variable(sink)
    with Path(path).open('a', encoding='utf-8') as handle:
        handle.write(f'{line}\n')


def set_output(name: str, value: str) -> None:
    append('GITHUB_OUTPUT', f'{name}={value}')


def short_sha() -> str:
    """The first nine characters of `GITHUB_SHA`, as `${GITHUB_SHA:0:9}`."""

    return variable('GITHUB_SHA')[:9]


def commit_hash() -> int:
    set_output('commitHash', short_sha())
    return 0


def set_redirects(project: str) -> int:
    if not project:
        raise Failure('--project is empty; the step reads vars.CF_PAGES_PROJECT')
    target = workspace() / 'docs' / '_redirects'
    if not target.parent.is_dir():
        raise Failure(f'{target.parent} does not exist; the docs artifact is missing')
    with target.open('a', encoding='utf-8') as handle:
        for line in REDIRECTS:
            handle.write(line.format(project=project) + '\n')
    return 0


def check_tag() -> int:
    tag = variable('GITHUB_REF_NAME')
    if PRERELEASE_TAG.fullmatch(tag):
        set_output('validTag', 'true')
        set_output('prerelease', 'true')
        set_output('version', tag)
    elif RELEASE_TAG.fullmatch(tag):
        set_output('validTag', 'true')
        set_output('prerelease', 'false')
        set_output('version', tag)
    else:
        set_output('validTag', 'false')
    return 0


def rename_artifacts(version: str) -> int:
    if not version:
        raise Failure('--version is empty; the step reads steps.check.outputs.version')
    root = workspace()
    commit = short_sha()
    print('::group::Renaming Artifacts', flush=True)
    for directory_template, pattern, target_template in ARTIFACTS:
        directory = root / directory_template.format(commit=commit, version=version)
        target = root / target_template.format(version=version)
        matches = sorted(directory.glob(pattern))
        if not matches:
            raise Failure(f'no file matches {pattern} in {directory.name}')
        if len(matches) > 1:
            raise Failure(
                f'more than one file matches {pattern} in {directory.name}:'
                f' {", ".join(match.name for match in matches)}'
            )
        source = matches[0]
        shutil.move(str(source), str(target))
        print(
            f"renamed '{source.relative_to(root)}' -> '{target.relative_to(root)}'",
            flush=True,
        )
    print('::endgroup::', flush=True)
    return 0


def checksums() -> int:
    root = workspace()
    artifacts = sorted(
        {
            path
            for pattern in CHECKSUM_PATTERNS
            for path in root.glob(pattern)
            if path.is_file()
        },
        key=lambda path: path.name,
    )
    lines = ['### Checksums']
    for path in artifacts:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f'    {path.name}: {digest}')
    (root / 'CHECKSUMS.txt').write_text('\n'.join(lines) + '\n', encoding='utf-8')
    return 0


def parser() -> argparse.ArgumentParser:
    arguments = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    subcommands = arguments.add_subparsers(dest='command', required=True)

    subcommands.add_parser(
        'commit-hash', help='record the short commit hash for the artifact names'
    )
    redirects_parser = subcommands.add_parser(
        'set-redirects', help='append the previous-release redirects to the docs'
    )
    redirects_parser.add_argument(
        '--project', default='', help='the Cloudflare Pages project (vars.CF_PAGES_PROJECT)'
    )
    subcommands.add_parser(
        'check-tag', help='classify the pushed tag as a release or a prerelease'
    )
    rename_parser = subcommands.add_parser(
        'rename-artifacts', help='move the downloaded artifacts to their release names'
    )
    rename_parser.add_argument(
        '--version', default='', help='the release version (steps.check.outputs.version)'
    )
    subcommands.add_parser('checksums', help='write CHECKSUMS.txt for the release')
    return arguments


def main(argv: list[str] | None = None) -> int:
    arguments = parser().parse_args(argv)
    try:
        if arguments.command == 'commit-hash':
            return commit_hash()
        if arguments.command == 'set-redirects':
            return set_redirects(arguments.project)
        if arguments.command == 'check-tag':
            return check_tag()
        if arguments.command == 'rename-artifacts':
            return rename_artifacts(arguments.version)
        return checksums()
    except Failure as failure:
        fail(str(failure))
        return failure.status


if __name__ == '__main__':
    sys.exit(main())
