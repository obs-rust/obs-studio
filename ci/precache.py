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
    uv run ci/precache.py pull  FAMILY PLATFORM DIR [--tag TAG]
    uv run ci/precache.py plan  FAMILY PLATFORM IDENTITY
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile

# `uv run ci/precache.py` puts ci/ on sys.path; import the sibling by name.
from fetch_libs_asset import fetch, sha256_of

REPOSITORY = 'obs-rust/obs-studio-libs'
DEFAULT_TAG = 'ci-cache'
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
        gh('release', 'upload', args.tag, str(archive), str(sidecar),
           '--repo', REPOSITORY, '--clobber')
    return 0


def pull(args: argparse.Namespace) -> int:
    destination = Path(args.directory).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    pattern = f'cache-{args.family}-{args.platform}-*.manifest.json'
    with tempfile.TemporaryDirectory() as tmp:
        gh('release', 'download', args.tag, '--repo', REPOSITORY,
           '--pattern', pattern, '--dir', tmp, '--clobber')
        candidates = sorted(Path(tmp).glob(pattern))
        if not candidates:
            print(f'no {args.family}/{args.platform} cache published under {args.tag}', flush=True)
            return 0
        data = parse_manifest(candidates[-1].read_text())
        print(f'pulling {data["asset"]} ({data["bytes"]} B), identity {data["identity"]}', flush=True)
        fetch(args.tag, data['asset'], data['sha256'], destination)
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
        sub.add_argument('directory')
        sub.add_argument('--tag', default=DEFAULT_TAG)
        if name == 'push':
            sub.add_argument('identity')
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
