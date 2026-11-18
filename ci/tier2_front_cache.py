#!/usr/bin/env -S uv run --script
"""Deduplicate hosted Tier 2 fixture compiles within one build (Refs #4988).

    uv run --no-project ci/tier2_front_cache.py -- bash rust/tools/linux-validate/validate.sh

The backing zccache keys user depfiles on output/target argv. Hundreds of
private fixtures therefore compile identical C support files again. ccache
rewrites dependency targets on replay; a fresh front cache shares those
compiles without changing the existing remote cache or its publication policy.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
import tempfile
import urllib.request

VERSION = '4.14.3'
NAME = f'ccache-{VERSION}-linux-x86_64-musl-static'
SHA256 = 'd553fac006ed93a37342cee9228cf4e13dd66690fa1a8075c54c7bace9b72ae6'
URL = f'https://github.com/ccache/ccache/releases/download/v{VERSION}/{NAME}.tar.xz'


def install(work: Path) -> Path:
    """Install only the checksum-pinned binary, before executing it."""
    work.mkdir(parents=True, exist_ok=True)
    archive = work / 'ccache.tar.xz'
    urllib.request.urlretrieve(URL, archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != SHA256:
        raise RuntimeError('Tier 2 front cache SHA-256 mismatch')
    binary = work / 'ccache'
    with tarfile.open(archive, 'r:xz') as tar:
        member = tar.getmember(NAME + '/ccache')
        if not member.isfile():
            raise RuntimeError('Tier 2 front cache binary is not a regular file')
        with tar.extractfile(member) as source:
            binary.write_bytes(source.read())
    binary.chmod(0o755)
    return binary


def main(command: list[str]) -> int:
    if command[:1] == ['--']:
        command = command[1:]
    if not command:
        raise SystemExit('usage: tier2_front_cache.py -- <build command>')
    # The existing hosted job is x64. Dell's empty launcher and other explicit
    # overrides keep their original execution, including kept-tree paths.
    if os.environ.get('OBS_TIER2_LAUNCHER', 'zccache') != 'zccache' or platform.machine() not in ('x86_64', 'amd64'):
        return subprocess.call(command)
    with tempfile.TemporaryDirectory(prefix='tier2-front-cache-') as directory:
        work = Path(directory)
        binary = install(work)
        env = dict(os.environ, OBS_TIER2_LAUNCHER=str(binary), CCACHE_PREFIX='zccache',
                   CCACHE_DIR=str(work / 'objects'), CCACHE_MAXSIZE='512M')
        # Job-local defaults only: do not inherit a user's relaxed cache policy
        # or redirects through a global config file/remote storage.
        config = work / 'ccache.conf'
        config.write_text('')
        for key in list(env):
            if key.startswith('CCACHE_') and key not in ('CCACHE_DIR', 'CCACHE_MAXSIZE', 'CCACHE_PREFIX'):
                del env[key]
        env['CCACHE_CONFIGPATH'] = str(config)
        env['PATH'] = str(work) + os.pathsep + env.get('PATH', '')
        print('== Tier 2: fresh ccache front layer, zccache backing cache (512M)', flush=True)
        try:
            return subprocess.call(command, env=env)
        finally:
            # Diagnostics and shutdown cannot hide the validation exit status.
            subprocess.call([str(binary), '--show-stats'], env=env)
            subprocess.call(['zccache', 'stop'], env=env)


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
