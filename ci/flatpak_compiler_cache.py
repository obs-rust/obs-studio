#!/usr/bin/env -S uv run --script
"""Explicit experimental compiler-cache artifact; never a production cache writer."""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import stat
import subprocess
import tarfile
import tempfile
import time

MAX_PAYLOAD_BYTES = 2 * 1024 ** 3
MAX_FILES = 100_000
CACHE_SUBDIR = Path('ccache/obs-zccache')
FORMAT = 2
DURABLE_FILES = frozenset(('index.bin', 'metadata.bin', 'compiler_hash.bin',
                           'system_includes.bin', 'depgraph/depgraph.bin'))


def durable(path):
    version, *parts = path.parts
    if not version.startswith('v') or not version[1:].replace('.', '').isdigit() or not parts:
        return False
    return ('/'.join(parts) in DURABLE_FILES or
            (parts[0] == 'artifacts' and parts[-1] != '.store.lock'))


def read_session(cache):
    stats = json.loads((cache / 'session.json').read_text())
    keys = ('hits', 'misses', 'compilations', 'non_cacheable', 'errors')
    if stats.get('status') != 'ok' or any(type(stats.get(key)) is not int or stats[key] < 0 for key in keys):
        raise ValueError('invalid completed Flatpak zccache session')
    if not stats['compilations']:
        raise ValueError('Flatpak zccache session observed zero compilations')
    return stats


def require_reuse(stats):
    hits, misses = stats['hits'], stats['misses']
    return hits >= 100 and hits / max(1, hits + misses) >= 0.8


def verify_package(app):
    if not app.is_dir():
        raise ValueError(f'missing Flatpak application directory: {app}')
    for path in app.rglob('*'):
        if path.name in {'.flatpak-zccache', '.flatpak-zccache-state', 'obs-zccache', 'zccache', 'zccache-fp', 'zccache-daemon'}:
            raise ValueError(f'build-only zccache leaked into Flatpak application: {path.relative_to(app)}')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def context(manifest):
    recipe = json.loads(manifest.read_text())
    sdk = recipe['sdk'] + '//' + recipe['runtime-version']
    def query(command):
        return subprocess.check_output(command, text=True, timeout=60).strip()
    architecture = query(['uname', '-m'])
    obs = next(module for module in recipe['modules'] if isinstance(module, dict) and module.get('name') == 'obs')
    source = next(source for source in obs['sources']
                  if source.get('dest') == '.flatpak-zccache' and source.get('only-arches') == [architecture])
    # Journal configuration affects observability, not the compiler cache identity.
    env = obs['build-options']['env']
    env.pop('OBS_FLATPAK_ZCCACHE_JOURNAL', None)
    manifest_identity = hashlib.sha256(json.dumps(recipe, sort_keys=True).encode()).hexdigest()
    return {
        'zccache': source['sha256'],
        'source': query(['git', 'rev-parse', 'HEAD']),
        'manifest': manifest_identity,
        'sdk': query(['flatpak', 'info', '--show-commit', sdk]),
        'compiler': query(['flatpak', 'run', '--command=sh', sdk, '-ec',
                           'sha256sum /usr/bin/cc /usr/bin/c++']),
        'architecture': architecture,
    }


def pack(source, artifact, profile):
    start = time.monotonic()
    source, artifact = Path(source), Path(artifact)
    if not source.is_dir() or source.is_symlink():
        raise ValueError('compiler cache must be an ordinary directory')
    files, size, omitted = [], 0, []
    for entry in sorted(source.rglob('*')):
        mode = entry.lstat().st_mode
        relative = entry.relative_to(source)
        if stat.S_ISDIR(mode):
            continue
        # The stopped daemon can leave its socket inode in the mount.
        if stat.S_ISSOCK(mode) and entry.name == f'daemon-{relative.parts[0]}.sock' and len(relative.parts) == 2:
            omitted.append(relative.as_posix())
            continue
        if not stat.S_ISREG(mode) or relative.parts[0] == 'bin':
            raise ValueError('compiler cache contains an unsupported link/non-regular file or wrapper entry: '
                             + relative.as_posix())
        if not durable(relative):
            continue
        size += entry.stat().st_size
        files.append(entry)
        if size > MAX_PAYLOAD_BYTES or len(files) > MAX_FILES:
            raise ValueError('compiler artifact exceeds experimental size/file limit')
    if not files:
        raise ValueError('compiler cache is empty')
    artifact.mkdir(parents=True, exist_ok=False)
    archive = artifact / 'compiler-cache.tar'
    # Retain real archive bytes and avoid
    # hiding another compression pass outside the producer clock.
    with tarfile.open(archive, 'w') as output:
        for entry in files:
            member = tarfile.TarInfo(entry.relative_to(source).as_posix())
            member.size = entry.stat().st_size
            member.mode = 0o600
            with entry.open('rb') as stream:
                output.addfile(member, stream)
    report = {'format': FORMAT, 'profile': profile, 'sha256': digest(archive),
              'archive_bytes': archive.stat().st_size, 'payload_bytes': size,
              'files': len(files), 'omitted_daemon_sockets': omitted,
              'pack_seconds': time.monotonic() - start}
    (artifact / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def restore(artifact, destination, profile):  # noqa: C901
    start = time.monotonic()
    artifact, destination = Path(artifact), Path(destination)
    report = json.loads((artifact / 'manifest.json').read_text())
    if report.get('format') != FORMAT or report.get('profile') != profile:
        producer = report.get('profile', {})
        mismatch = {'producer': producer, 'consumer': profile,
                    'different_fields': sorted(key for key in set(producer) | set(profile)
                                               if producer.get(key) != profile.get(key))}
        (artifact / 'profile-mismatch.json').write_text(json.dumps(mismatch, indent=2) + '\n')
        raise ValueError('compiler artifact profile mismatch: ' + ', '.join(mismatch['different_fields']))
    archive = artifact / 'compiler-cache.tar'
    if archive.is_symlink() or not archive.is_file():
        raise ValueError('compiler artifact must be an ordinary archive')
    if archive.stat().st_size != report['archive_bytes'] or archive.stat().st_size > MAX_PAYLOAD_BYTES + MAX_FILES * 2048:
        raise ValueError('compiler archive size mismatch or limit exceeded')
    if digest(archive) != report['sha256']:
        raise ValueError('compiler artifact digest mismatch')
    # Validate the complete member inventory before creating or overlaying the
    # destination. Only ordinary cache files are accepted, never archive links.
    with tarfile.open(archive, 'r:') as source:
        members = []
        for member in source:
            if len(members) >= MAX_FILES:
                raise ValueError('compiler archive file limit exceeded')
            members.append(member)
        total, names = 0, set()
        if len(members) > MAX_FILES:
            raise ValueError('compiler archive file limit exceeded')
        for member in members:
            path = PurePosixPath(member.name)
            if (not member.isfile() or path.is_absolute() or '..' in path.parts
                    or not path.parts or not durable(path) or member.name != path.as_posix()
                    or member.name in names or '\\' in member.name):
                raise ValueError('unsafe compiler archive member')
            names.add(member.name)
            total += member.size
            if total > MAX_PAYLOAD_BYTES:
                raise ValueError('compiler archive payload size limit exceeded')
        if total != report['payload_bytes'] or len(members) != report['files']:
            raise ValueError('compiler archive inventory mismatch')
        if any(parent.is_symlink() for parent in [destination, *destination.parents]):
            raise ValueError('compiler destination contains links')
        if destination.exists() and (not destination.is_dir()
                                     or any(p.is_symlink() for p in destination.rglob('*'))):
            raise ValueError('compiler destination contains links or is not a directory')
        destination.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='flatpak-compiler-transfer-', dir=destination.parent) as directory:
            staged = Path(directory)
            for member in members:
                target = staged / member.name
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as incoming, target.open('xb') as outgoing:
                    shutil.copyfileobj(incoming, outgoing)
            shutil.copytree(staged, destination, dirs_exist_ok=True)
    return dict(report, restore_seconds=time.monotonic() - start)


def main():  # noqa: C901
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['pack', 'restore', 'reset', 'report', 'verify', 'force'])
    parser.add_argument('--manifest', type=Path)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--artifact', type=Path)
    parser.add_argument('--app', type=Path)
    # Set only when the caller knows this build had a warm cache to reuse
    # (#5788); a cold build keeps the zero-hit warning instead of failing.
    parser.add_argument('--expect-reuse', action='store_true',
                        help='report: fail when the build reused nothing although reuse was expected')
    args = parser.parse_args()
    if args.operation in ('force', 'pack', 'restore') and args.manifest is None:
        parser.error(f'{args.operation} requires --manifest')
    if args.operation in ('pack', 'restore', 'verify') and args.artifact is None:
        parser.error(f'{args.operation} requires --artifact')
    if args.operation == 'force':
        recipe = json.loads(args.manifest.read_text())
        if recipe['modules'][-1]['name'] != 'obs':
            raise ValueError('expected final obs application module')
        recipe['modules'][-1].setdefault('post-install', []).append("printf '%s\\n' OBS_COMPILER_CONSUMER_EXECUTED")
        args.manifest.write_text(json.dumps(recipe, indent=2) + '\n')
        result = {'forced': 'application post-install checksum only; source and compiler configuration preserved'}
    elif args.operation == 'reset':
        (args.cache / 'session.json').unlink(missing_ok=True)
        result = {'reset': 'previous session report removed'}
    elif args.operation in ('report', 'verify'):
        if args.app is not None:
            verify_package(args.app)
        stats = read_session(args.cache)
        hits, misses = stats['hits'], stats['misses']
        if args.operation == 'report':
            summary = (f"Flatpak Zccache Stats (this build): warm_hits={hits} cold_misses={misses} "
                       f"uncacheable={stats['non_cacheable']} compilations={stats['compilations']}")
            print(summary)
            if output := os.environ.get('GITHUB_STEP_SUMMARY'):
                with Path(output).open('a', encoding='utf-8') as stream:
                    print(summary, file=stream)
            if not hits:
                if args.expect_reuse:
                    print('::error::Flatpak zccache reported no hits although compiler reuse was expected')
                    raise SystemExit(1)
                print('::warning::Flatpak zccache reported no hits for this build')
            return
        result = {'hits': hits, 'misses': misses, 'stats': stats,
                  'status': 'success' if require_reuse(stats) else 'failed'}
        (args.artifact / 'consumer-result.json').write_text(json.dumps(result, indent=2) + '\n')
        if result['status'] != 'success':
            raise ValueError(f'insufficient cross-runner compiler reuse: {hits} hits, {misses} misses')
    elif args.operation == 'pack':
        read_session(args.cache)
        result = pack(args.cache / 'state', args.artifact, context(args.manifest))
    else:
        result = restore(args.artifact, args.cache / 'state', context(args.manifest))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
