#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Publish and pull a build cache as an obs-rust/obs-studio-libs release asset.

The Actions cache is the only cross-job store today, and `ci.toml` bounds it at
10 GB with default-branch-only writes, so the six app-build families left it in
Phase A of #1801 ("the release-asset transport (#782) can bring them back
outside this budget"). Nothing implemented that transport: `fetch_libs_asset.py`
can only *read* a pinned release asset, and nothing here could write one.

This is the writer, plus the reader that discovers the pin. The naming follows
the libs releases already in use:

    openssl-3.5.9-linux-x86_64-glibc2.28-release.tar.zst   (existing)

    cache-<family>-<platform>-<identity>.tar.zst           (this tool)
    cache-<family>-<platform>-<identity>.manifest.json     (the pin)

The manifest exists because `fetch_libs_asset.fetch` deliberately requires a
sha256, while a cache changes on every master push. The consumer downloads the
small manifest first, reads the digest and size, then fetches the blob through
the same verified path the Qt and FFmpeg bundles already use. One code path for
"big verified archive from a release" instead of a second one.

    uv run ci/precache.py push  FAMILY PLATFORM IDENTITY DIR [--tag TAG]
    uv run ci/precache.py pull  FAMILY PLATFORM DIR [--tag TAG] [--identity ID]
    uv run ci/precache.py plan  FAMILY PLATFORM IDENTITY

The store is one rolling prerelease (`ci-cache`) in the libs repository. `push`
creates it on first use and marks it prerelease so it never becomes the
repository's "Latest" release (OpenSSL holds that). `pull` without
`--identity` takes the newest published manifest for the family and platform,
by release-asset timestamp -- not the lexicographically largest name, which an
identity like `rustc1.98.1-...` vs `rustc1.101.0-...` would order wrong. It
exits `NO_CACHE` (3) when nothing was ever published, so a consumer can tell
"no cache exists" from "restored".
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile

# `uv run ci/precache.py` puts ci/ on sys.path; import the sibling by name.
from fetch_libs_asset import STAMP as FETCH_STAMP, fetch, sha256_of

REPOSITORY = 'obs-rust/obs-studio-libs'
DEFAULT_TAG = 'ci-cache'
# A restore that found nothing published is not an error, but it must be
# distinguishable from a restore that worked: callers branch on this.
NO_CACHE = 3
RELEASE_TITLE = 'CI build caches'
RELEASE_NOTES = (
    'Rolling store for build caches published by `ci/precache.py`.\n\n'
    'Assets are `cache-<family>-<platform>-<identity>.tar.zst` plus a '
    '`*.manifest.json` carrying the sha256 the consumer verifies. This release '
    'is a transport, not a product: safe to delete once a family moves back '
    'into the Actions cache or a different store.'
)
# Everything the cache needs to be reproducible and inert: no owner names, no
# build-host paths, no absolute uids. mtimes stay as they are on purpose --
# cargo and zccache compare them, so normalising them would invalidate a cache
# that is perfectly good.
NORMALISED = {'uid': 0, 'gid': 0, 'uname': '', 'gname': ''}


def asset_name(family: str, platform: str, identity: str) -> str:
    return f'cache-{family}-{platform}-{identity}.tar.zst'


def manifest_name(family: str, platform: str, identity: str) -> str:
    return f'cache-{family}-{platform}-{identity}.manifest.json'


def pack(directory: Path, archive: Path) -> tuple[str, int]:
    """Write DIRECTORY as a zstd tar; return (sha256, entry count).

    Sorted members and zeroed ownership make two packs of the same tree
    byte-identical, so re-pushing an unchanged cache is a no-op upstream.
    """
    from compression import zstd

    entries = sorted(p for p in directory.rglob('*'))
    with archive.open('wb') as raw, zstd.open(raw, 'wb') as compressed:
        with tarfile.open(fileobj=compressed, mode='w|') as tar:
            for path in entries:
                info = tar.gettarinfo(str(path), arcname=path.relative_to(directory).as_posix())
                for field, value in NORMALISED.items():
                    setattr(info, field, value)
                if info.isreg():
                    with path.open('rb') as handle:
                        tar.addfile(info, handle)
                else:
                    tar.addfile(info)
    return sha256_of(archive), len(entries)


def manifest(family: str, platform: str, identity: str, sha: str, size: int, entries: int) -> str:
    return json.dumps({
        'family': family,
        'platform': platform,
        'identity': identity,
        'asset': asset_name(family, platform, identity),
        'sha256': sha,
        'bytes': size,
        'entries': entries,
    }, indent=2, sort_keys=True) + '\n'


def parse_manifest(text: str) -> dict:
    data = json.loads(text)
    for field in ('asset', 'sha256', 'identity'):
        if not data.get(field):
            raise ValueError(f'manifest is missing {field}')
    return data


def gh(*args: str) -> None:
    print(f'+ gh {" ".join(args)}', flush=True)
    subprocess.run(['gh', *args], check=True)


def release_assets(tag: str) -> list[dict]:
    """Every asset on TAG, or [] when the release does not exist yet."""
    result = subprocess.run(
        ['gh', 'api', f'repos/{REPOSITORY}/releases/tags/{tag}'],
        capture_output=True, text=True)
    if result.returncode != 0:
        return []
    return json.loads(result.stdout).get('assets') or []


def ensure_release(tag: str) -> None:
    """Create the rolling store release if this is the first push.

    `gh release upload` fails outright when the release does not exist, so
    without this the very first cache of a family could never be published --
    and nothing else in the pipeline creates this tag. Prerelease keeps it out
    of the libs repository's "Latest" slot, which the dependency releases
    (OpenSSL, curl, ...) own.
    """
    if release_assets(tag):
        return
    gh('release', 'create', tag, '--repo', REPOSITORY, '--title', RELEASE_TITLE,
       '--notes', RELEASE_NOTES, '--prerelease', '--latest=false')


def select_manifest(assets: list[dict], family: str, platform: str,
                    identity: str | None = None) -> str:
    """Name of the manifest to restore: an exact pin, else the newest.

    Timestamp order, not name order: identities embed toolchain and soldr
    versions, so `rustc1.9.10` sorts before `rustc1.98.1` while being older.
    """
    prefix = f'cache-{family}-{platform}-'
    manifests = [a for a in assets
                 if a['name'].startswith(prefix) and a['name'].endswith('.manifest.json')]
    if identity is not None:
        wanted = manifest_name(family, platform, identity)
        return wanted if any(a['name'] == wanted for a in manifests) else ''
    if not manifests:
        return ''
    return max(manifests, key=lambda a: a.get('updated_at') or a.get('created_at') or '')['name']


def push(args: argparse.Namespace) -> int:
    source = Path(args.directory).resolve()
    if not source.is_dir():
        raise SystemExit(f'{source} is not a directory')
    with tempfile.TemporaryDirectory() as tmp:
        archive = Path(tmp) / asset_name(args.family, args.platform, args.identity)
        sha, entries = pack(source, archive)
        sidecar = Path(tmp) / manifest_name(args.family, args.platform, args.identity)
        sidecar.write_text(manifest(args.family, args.platform, args.identity,
                                    sha, archive.stat().st_size, entries))
        print(f'{archive.name}: {entries} entries, {archive.stat().st_size} B, {sha}', flush=True)
        ensure_release(args.tag)
        gh('release', 'upload', args.tag, str(archive), str(sidecar),
           '--repo', REPOSITORY, '--clobber')
    return 0


def merge_tree(staged: Path, destination: Path) -> None:
    """Move a staged extraction into DESTINATION, dropping fetch's own stamp.

    `.asset-sha256` is `fetch_libs_asset`'s "already downloaded" marker, not
    cache content. Left inside a restored compiler cache it injects a foreign
    file into the tree, and -- worse -- makes the next pull into that same
    directory skip as a hit, so a newer identity silently never lands.
    """
    for entry in sorted(staged.iterdir()):
        if entry.name == FETCH_STAMP:
            continue
        shutil.move(str(entry), str(destination / entry.name))


def pull(args: argparse.Namespace) -> int:
    destination = Path(args.directory).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    selected = select_manifest(release_assets(args.tag), args.family, args.platform,
                               args.identity)
    if not selected:
        wanted = (f'{args.family}/{args.platform} identity {args.identity}'
                  if args.identity else f'{args.family}/{args.platform}')
        print(f'no {wanted} cache published under {args.tag}', flush=True)
        return NO_CACHE
    with tempfile.TemporaryDirectory() as tmp:
        gh('release', 'download', args.tag, '--repo', REPOSITORY,
           '--pattern', selected, '--dir', tmp, '--clobber')
        data = parse_manifest((Path(tmp) / selected).read_text())
        if args.identity is not None and data['identity'] != args.identity:
            raise SystemExit(f'{selected} carries identity {data["identity"]}, not {args.identity}')
        print(f'pulling {data["asset"]} ({data["bytes"]} B), identity {data["identity"]}', flush=True)
        staged = Path(tmp) / 'extract'
        staged.mkdir()
        fetch(args.tag, data['asset'], data['sha256'], staged)
        merge_tree(staged, destination)
    return 0


def plan(args: argparse.Namespace) -> int:
    print(asset_name(args.family, args.platform, args.identity))
    print(manifest_name(args.family, args.platform, args.identity))
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    stages = parser.add_subparsers(dest='stage', required=True)
    for name, handler in (('push', push), ('pull', pull)):
        sub = stages.add_parser(name)
        sub.add_argument('family')
        sub.add_argument('platform')
        # The docstring's order: FAMILY PLATFORM IDENTITY DIR for push, and an
        # optional pinned identity for pull before the destination.
        if name == 'push':
            sub.add_argument('identity')
        else:
            sub.add_argument('--identity', default=None,
                             help='restore exactly this identity instead of the newest')
        sub.add_argument('directory')
        sub.add_argument('--tag', default=DEFAULT_TAG)
        sub.set_defaults(handler=handler)
    plan_parser = stages.add_parser('plan')
    plan_parser.add_argument('family')
    plan_parser.add_argument('platform')
    plan_parser.add_argument('identity')
    plan_parser.set_defaults(handler=plan)
    args = parser.parse_args()
    return args.handler(args)


if __name__ == '__main__':
    sys.exit(main())
