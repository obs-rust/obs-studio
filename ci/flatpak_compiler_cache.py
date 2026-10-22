#!/usr/bin/env -S uv run --script
"""Explicit experimental compiler-cache artifact; never a production cache writer."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import stat
import subprocess
import tarfile
import tempfile
import time

MAX_PAYLOAD_BYTES = 2 * 1024 ** 3
MAX_FILES = 100_000
BUILDER_WRAPPERS = frozenset('bin/' + name for name in ('cc', 'c++', 'gcc', 'g++'))


def builder_wrapper(path, root):
    # flatpak-builder 1.4.9 regenerates these four sandbox launchers. They are
    # scaffolding, not compiler results, and must never enter the archive.
    return (path.relative_to(root).as_posix() in BUILDER_WRAPPERS
            and path.is_symlink() and str(path.readlink()) == '/usr/bin/ccache')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def context(manifest):
    recipe = json.loads(manifest.read_text())
    sdk = recipe['sdk'] + '//' + recipe['runtime-version']
    def query(command):
        return subprocess.check_output(command, text=True, timeout=60).strip()
    return {
        'source': query(['git', 'rev-parse', 'HEAD']),
        'manifest': digest(manifest),
        'sdk': query(['flatpak', 'info', '--show-commit', sdk]),
        'compiler': query(['flatpak', 'run', '--command=sh', sdk, '-ec',
                           'sha256sum /usr/bin/cc /usr/bin/c++; ccache --version']),
        'architecture': query(['uname', '-m']),
    }


def pack(source, artifact, profile):
    start = time.monotonic()
    source, artifact = Path(source), Path(artifact)
    if not source.is_dir() or source.is_symlink():
        raise ValueError('compiler cache must be an ordinary directory')
    files, size, omitted = [], 0, []
    for entry in sorted(source.rglob('*')):
        mode = entry.lstat().st_mode
        if builder_wrapper(entry, source):
            omitted.append(entry.relative_to(source).as_posix())
            continue
        if stat.S_ISDIR(mode):
            continue
        if not stat.S_ISREG(mode) or entry.relative_to(source).parts[0] == 'bin':
            raise ValueError('compiler cache contains an unsupported link/non-regular file or wrapper entry: '
                             + entry.relative_to(source).as_posix())
        size += entry.stat().st_size
        files.append(entry)
        if size > MAX_PAYLOAD_BYTES or len(files) > MAX_FILES:
            raise ValueError('compiler artifact exceeds experimental size/file limit')
    if not files:
        raise ValueError('compiler cache is empty')
    artifact.mkdir(parents=True, exist_ok=False)
    archive = artifact / 'compiler-cache.tar'
    # Ccache results are already compressed; retain real archive bytes and avoid
    # hiding another compression pass outside the producer clock.
    with tarfile.open(archive, 'w') as output:
        for entry in files:
            member = tarfile.TarInfo(entry.relative_to(source).as_posix())
            member.size = entry.stat().st_size
            member.mode = 0o600
            with entry.open('rb') as stream:
                output.addfile(member, stream)
    report = {'format': 1, 'profile': profile, 'sha256': digest(archive),
              'archive_bytes': archive.stat().st_size, 'payload_bytes': size,
              'files': len(files), 'omitted_builder_wrappers': omitted,
              'pack_seconds': time.monotonic() - start}
    (artifact / 'manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def restore(artifact, destination, profile):
    start = time.monotonic()
    artifact, destination = Path(artifact), Path(destination)
    report = json.loads((artifact / 'manifest.json').read_text())
    if report.get('format') != 1 or report.get('profile') != profile:
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
                    or not path.parts or path.parts[0] == 'bin'
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
                                     or any(p.is_symlink() and not builder_wrapper(p, destination)
                                            for p in destination.rglob('*'))):
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['pack', 'restore', 'reset', 'verify', 'force'])
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--artifact', type=Path, required=True)
    args = parser.parse_args()
    if args.operation == 'force':
        recipe = json.loads(args.manifest.read_text())
        if recipe['modules'][-1]['name'] != 'obs':
            raise ValueError('expected final obs application module')
        recipe['modules'][-1].setdefault('post-install', []).append("printf '%s\\n' OBS_COMPILER_CONSUMER_EXECUTED")
        args.manifest.write_text(json.dumps(recipe, indent=2) + '\n')
        result = {'forced': 'application post-install checksum only; source and compiler configuration preserved'}
    elif args.operation in ('reset', 'verify'):
        recipe = json.loads(args.manifest.read_text())
        command = ['flatpak', 'run', '--command=ccache', '--filesystem=' + str(args.cache.resolve()),
                   '--env=CCACHE_DIR=' + str(args.cache.resolve()), recipe['sdk'] + '//' + recipe['runtime-version']]
        if args.operation == 'reset':
            result = {'reset': subprocess.check_output([*command, '--zero-stats'], text=True, timeout=60).strip()}
        else:
            stats = dict(line.split() for line in subprocess.check_output([*command, '--print-stats'], text=True, timeout=60).splitlines()
                         if len(line.split()) == 2)
            hits = int(stats.get('direct_cache_hit', 0)) + int(stats.get('preprocessed_cache_hit', 0))
            misses = int(stats.get('cache_miss', 0))
            result = {'hits': hits, 'misses': misses, 'stats': stats, 'status': 'success'}
            if hits < 100 or hits / max(1, hits + misses) < 0.8:
                result['status'] = 'failed'
            (args.artifact / 'consumer-result.json').write_text(json.dumps(result, indent=2) + '\n')
            if result['status'] != 'success':
                raise ValueError(f'insufficient cross-runner compiler reuse: {hits} hits, {misses} misses')
    else:
        profile = context(args.manifest)
        result = (pack(args.cache, args.artifact, profile) if args.operation == 'pack'
                  else restore(args.artifact, args.cache, profile))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
