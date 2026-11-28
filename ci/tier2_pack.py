#!/usr/bin/env -S uv run --script
"""Pack Tier 2 with setup-soldr's (or the caller's) zstd environment."""
from pathlib import Path
import subprocess
import sys


def pack(build: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    archive = output / 'tier2-tree.tar.zst'
    # No explicit level or thread count: tar's zstd process inherits
    # ZSTD_CLEVEL/ZSTD_NBTHREADS, exported by setup-soldr when it selects a
    # policy. An unset environment preserves the hosted zstd defaults.
    subprocess.run(['tar', '--zstd', '-cf', str(archive), '-C', str(build),
                    '--exclude=*.o', '--exclude=*.a', '--exclude=*.rlib', '--exclude=*.rmeta',
                    '--exclude=tree/cargo', 'tree'], check=True)
    print(f'== packed {archive.stat().st_size} bytes')


if __name__ == '__main__':
    pack(Path(sys.argv[1]), Path(sys.argv[2]))
