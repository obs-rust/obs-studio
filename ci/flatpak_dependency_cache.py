#!/usr/bin/env -S uv run --script
"""Experimental OSTree cache-only archive; no production cache publication."""

import argparse
import configparser
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shlex
import shutil
import stat
import subprocess
import tarfile
import tempfile
import time

MAX_BYTES = 8 * 1024 ** 3
MAX_ARCHIVE_BYTES = 2 * 1024 ** 3
MAX_FILES = 100_000
OBJECT = re.compile(r'objects/[0-9a-f]{2}/[0-9a-f]{62}\.file')


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def fsck(path):
    subprocess.run(['ostree', '--repo=' + str(path), 'fsck'], check=True, timeout=180)


def validate_member(member):
    path = PurePosixPath(member.name)
    if (path.is_absolute() or '..' in path.parts or not path.parts
            or '\\' in member.name or path.as_posix() != member.name
            or any(part.endswith('.file') for part in path.parts[:-1])):
        raise ValueError('unsafe dependency archive path')
    if member.issym():
        if not OBJECT.fullmatch(member.name):
            raise ValueError('structural link in dependency cache')
    elif not (member.isfile() or member.isdir()):
        raise ValueError('unsafe dependency archive link or file type')


def pack(source, artifact, profile):
    start = time.monotonic()
    source, artifact = Path(source), Path(artifact)
    if not source.is_dir() or source.is_symlink():
        raise ValueError('dependency cache must be an ordinary directory')
    members, size, symbolic = [], 0, 0
    for entry in sorted(source.rglob('*')):
        member = tarfile.TarInfo(entry.relative_to(source).as_posix())
        mode = entry.lstat().st_mode
        if stat.S_ISLNK(mode):
            member.type, member.linkname = tarfile.SYMTYPE, str(entry.readlink())
            symbolic += 1
        elif stat.S_ISDIR(mode):
            member.type, member.mode = tarfile.DIRTYPE, 0o700
        elif stat.S_ISREG(mode):
            member.size, member.mode = entry.stat().st_size, stat.S_IMODE(mode)
            size += member.size
        else:
            raise ValueError('unsupported dependency cache file type')
        validate_member(member)
        members.append((entry, member))
        if size > MAX_BYTES or len(members) > MAX_FILES:
            raise ValueError('dependency cache exceeds experimental limits')
    if not members:
        raise ValueError('dependency cache is empty')
    configuration = configparser.ConfigParser()
    configuration.read(source / 'config')
    if configuration.get('core', 'mode', fallback='unknown') != 'bare-user-only':
        raise ValueError('unsupported dependency repository mode; only bare-user-only preserves metadata')
    fsck(source)
    artifact.mkdir(parents=True, exist_ok=False)
    archive = artifact / 'dependency-cache.tar.zst'
    with subprocess.Popen(['timeout', '300s', 'zstd', '-q', '-T0', '-o', str(archive)], stdin=subprocess.PIPE) as process:
        try:
            with tarfile.open(fileobj=process.stdin, mode='w|') as output:
                for entry, member in members:
                    if member.isfile():
                        with entry.open('rb') as data:
                            output.addfile(member, data)
                    else:
                        output.addfile(member)
            process.stdin.close()
            if process.wait(timeout=310):
                raise ValueError('dependency archive compression failed')
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
    if archive.stat().st_size > MAX_ARCHIVE_BYTES:
        raise ValueError('dependency archive exceeds experimental byte limit')
    report = {'format': 1, 'profile': profile, 'sha256': digest(archive),
              'archive_bytes': archive.stat().st_size, 'payload_bytes': size,
              'members': len(members), 'symbolic_objects': symbolic,
              'pack_seconds': time.monotonic() - start,
              'projection': 'cache only; may include application stage objects'}
    (artifact / 'dependency-manifest.json').write_text(json.dumps(report, indent=2) + '\n')
    return report


def read_archive(archive, visit):
    with subprocess.Popen(['timeout', '300s', 'zstd', '-q', '-dc', str(archive)], stdout=subprocess.PIPE) as process:
        try:
            with tarfile.open(fileobj=process.stdout, mode='r|') as source:
                for member in source:
                    visit(member, source)
            # Consume trailing decompressor output so its exit status is checked.
            while process.stdout.read(1024 * 1024):
                pass
            if process.wait(timeout=310):
                raise ValueError('dependency archive decompression failed')
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()


def restore(artifact, destination, profile):
    start = time.monotonic()
    artifact, destination = Path(artifact), Path(destination)
    report = json.loads((artifact / 'dependency-manifest.json').read_text())
    if report.get('format') != 1 or report.get('profile') != profile:
        raise ValueError('dependency artifact profile mismatch')
    archive = artifact / 'dependency-cache.tar.zst'
    if (archive.is_symlink() or not archive.is_file()
            or archive.stat().st_size != report['archive_bytes']
            or archive.stat().st_size > MAX_ARCHIVE_BYTES or digest(archive) != report['sha256']):
        raise ValueError('dependency archive digest or size mismatch')
    if destination.exists() or any(p.is_symlink() for p in [destination, *destination.parents]):
        raise ValueError('dependency destination must be absent and contain no structural links')
    names, total, symbolic = set(), 0, 0
    def inspect(member, source):
        nonlocal total, symbolic
        validate_member(member)
        if member.name in names:
            raise ValueError('duplicate dependency archive path')
        names.add(member.name)
        total += member.size
        symbolic += member.issym()
        if total > MAX_BYTES or len(names) > MAX_FILES:
            raise ValueError('dependency archive exceeds experimental limits')
    read_archive(archive, inspect)
    if (len(names) != report['members'] or total != report['payload_bytes']
            or symbolic != report['symbolic_objects']):
        raise ValueError('dependency archive inventory mismatch')
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='flatpak-dependency-', dir=destination.parent) as directory:
        staged = Path(directory) / 'cache'
        staged.mkdir()
        def extract(member, source):
            target = staged / member.name
            target.parent.mkdir(parents=True, exist_ok=True)
            if member.isdir():
                target.mkdir(exist_ok=True)
            elif member.issym():
                target.symlink_to(member.linkname)
            else:
                with source.extractfile(member) as incoming, target.open('xb') as outgoing:
                    shutil.copyfileobj(incoming, outgoing)
                target.chmod(member.mode & 0o777)
        read_archive(archive, extract)
        fsck(staged)
        staged.rename(destination)
    return dict(report, restore_seconds=time.monotonic() - start)


def main():
    from flatpak_compiler_cache import context
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['pack', 'restore', 'instrument', 'verify'])
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--cache', type=Path, required=True)
    parser.add_argument('--artifact', type=Path, required=True)
    args = parser.parse_args()
    if args.operation == 'instrument':
        builder = shutil.which('flatpak-builder')
        if not builder:
            raise ValueError('flatpak-builder missing')
        args.artifact.mkdir(parents=True, exist_ok=True)
        tools = args.artifact / 'bin'
        tools.mkdir()
        wrapper = tools / 'flatpak-builder'
        log = args.artifact / 'builder.log'
        wrapper.write_text('#!/bin/bash\nset -o pipefail\n' + shlex.quote(builder)
                           + ' "$@" 2>&1 | tee ' + shlex.quote(str(log.resolve())) + '\n')
        wrapper.chmod(0o700)
        print(tools.resolve())
    elif args.operation == 'verify':
        recipe = json.loads(args.manifest.read_text())
        log = (args.artifact / 'builder.log').read_text()
        dependencies = [entry['name'] for entry in recipe['modules'][:-1]]
        missing = [name for name in dependencies if 'Cache hit for ' + name + ', skipping build' not in log]
        result = {'missing_dependencies': missing, 'dependency_count': len(dependencies),
                  'application_rebuilt': ('OBS_COMPILER_CONSUMER_EXECUTED' in log.splitlines()
                                          and 'Cache hit for obs, skipping build' not in log)}
        result['status'] = 'success' if not missing and result['application_rebuilt'] else 'failed'
        (args.artifact / 'dependency-result.json').write_text(json.dumps(result, indent=2) + '\n')
        if result['status'] != 'success':
            raise ValueError('dependency reuse or application rebuild missing')
        print(json.dumps(result, indent=2))
    else:
        profile = context(args.manifest)
        result = (pack(args.cache, args.artifact, profile) if args.operation == 'pack'
                  else restore(args.artifact, args.cache, profile))
        # Keep consumer measurements separate from the immutable producer manifest.
        if args.operation == 'restore':
            (args.artifact / 'dependency-restore.json').write_text(json.dumps(result, indent=2) + '\n')
        print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
