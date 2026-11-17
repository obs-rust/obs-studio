#!/usr/bin/env -S uv run --script
"""Pack Tier 2 for a local handoff or GitHub, preserving hosted compression."""
import os
from pathlib import Path
import subprocess
import sys

from tier2_kept_tree import handoff_is_local


def prepare_upload(output: Path) -> None:
    """A failed local handoff must still upload level-one multithreaded zstd."""
    archive = output / 'tier2-tree.tar.zst'
    with archive.open('rb') as stream:
        if stream.read(4) == bytes.fromhex('28b52ffd'):
            return
    compressed = output / 'tier2-upload.tmp'
    subprocess.run(['zstd', '-1', '-T0', '-f', str(archive), '-o', str(compressed)], check=True)
    os.replace(compressed, archive)


def pack(build: Path, output: Path, marker: Path = Path('/var/cache/obs-runner/.persistent-cache')) -> None:
    compression = ['--zstd']
    if os.environ.get('OBS_TIER2_PERSISTENT'):
        compression = ['-I', 'zstd -1 -T0']
        if os.environ.get('OBS_TIER2_HANDOFF') and handoff_is_local(os.environ.get('MQ_RUNNER_LABELS', ''), marker):
            compression = []
    output.mkdir(parents=True, exist_ok=True)
    # Keep the established artifact name; tar -xf detects the stream's magic,
    # accepting both plain Dell tar and existing hosted zstd packs.
    archive = output / 'tier2-tree.tar.zst'
    subprocess.run(['tar', *compression, '-cf', str(archive), '-C', str(build),
                    '--exclude=*.o', '--exclude=*.a', '--exclude=*.rlib', '--exclude=*.rmeta',
                    '--exclude=tree/cargo', 'tree'], check=True)
    print(f'== packed {archive.stat().st_size} bytes ({" ".join(compression) or "plain tar"})')


if __name__ == '__main__':
    if sys.argv[1] == '--upload':
        prepare_upload(Path(sys.argv[2]))
    else:
        pack(Path(sys.argv[1]), Path(sys.argv[2]))
