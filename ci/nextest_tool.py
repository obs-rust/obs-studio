"""Pinned nextest installer used only when selected Rust tests actually run.

Linux uses a static upstream binary. Other hosts keep Cargo's existing runner.
No remote cache is published; the local archive is verified before every use.
"""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import platform
import shutil
import tarfile
import tempfile
from urllib.request import urlopen

import host_platform as host

VERSION = '0.9.143'
ARCHIVES = {
    'x86_64': ('x86_64-unknown-linux-musl', '6d891c18105ec2d33f6e441a4f92b7ccab47ba263e22055c8bb66884243f3389'),
    'aarch64': ('aarch64-unknown-linux-musl', '0560ce0ce017f368c54b5db86588370fc03b470985490fa81200e62a657dda05'),
}


def unpack_checked(archive: Path, output: Path, digest: str) -> None:
    with archive.open('rb') as stream:
        actual = hashlib.file_digest(stream, 'sha256').hexdigest()
    if actual != digest:
        raise ValueError(f'nextest archive checksum mismatch: {archive}')
    with tarfile.open(archive, 'r:gz') as source:
        member = source.getmember('cargo-nextest')
        if not member.isfile() or member.size > 64 * 1024 * 1024:
            raise ValueError('nextest archive must contain a regular executable')
        stream = source.extractfile(member)
        if stream is None:
            raise ValueError('nextest executable is missing')
        data = stream.read()
    if output.is_file() and output.read_bytes() == data:
        output.chmod(0o755)
        return
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as tmp:
        staged = Path(tmp) / 'cargo-nextest'
        staged.write_bytes(data)
        staged.chmod(0o755)
        staged.replace(output)


def ensure_nextest() -> Path | None:
    if not host.is_linux() or platform.machine() not in ARCHIVES:
        return None
    target, digest = ARCHIVES[platform.machine()]
    root = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'obs-rust' / 'nextest' / VERSION / target
    root.mkdir(parents=True, exist_ok=True)
    archive = root / 'nextest.tar.gz'
    if not archive.is_file():
        name = f'cargo-nextest-{VERSION}-{target}.tar.gz'
        url = f'https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-{VERSION}/{name}'
        print(f'+ download pinned nextest {VERSION} ({target})', flush=True)
        with tempfile.TemporaryDirectory(dir=root) as tmp:
            staged = Path(tmp) / name
            with urlopen(url, timeout=30) as response, staged.open('wb') as destination:
                shutil.copyfileobj(response, destination)
            # Validate before publishing either cache file or executable.
            unpack_checked(staged, root / 'cargo-nextest', digest)
            staged.replace(archive)
    output = root / 'cargo-nextest'
    unpack_checked(archive, output, digest)
    return output
