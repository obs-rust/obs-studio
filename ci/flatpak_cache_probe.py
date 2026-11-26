#!/usr/bin/env -S uv run --script
"""Opt-in source-fetch and proxy-compression measurements; never prune state."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time


ARCHIVE_SCRIPT = '''root=$1
shift
tar --posix --pax-option=delete=atime,delete=ctime -C "$root" -cf - "$@" | zstd -q -T0 --long=30 | wc -c
'''


def run(command, seconds=180, quiet=False):
    bounded = ['timeout', '--kill-after=10s', str(seconds) + 's', *map(str, command)]
    print('+ ' + shlex.join(bounded), file=sys.stderr)
    start = time.monotonic()
    result = subprocess.run(bounded, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    elapsed = time.monotonic() - start
    if not quiet or result.returncode:
        print(result.stdout, end='', file=sys.stderr)
    print(result.stderr, end='', file=sys.stderr)
    result.check_returncode()
    return result.stdout, elapsed


def measure(state, manifest, arch, builder='flatpak-builder', temp_root=None):
    state = Path(state).resolve(strict=True)
    manifest = Path(manifest).resolve(strict=True)
    downloads = state / 'downloads'
    if not downloads.is_dir():
        raise ValueError('measurement requires the actual populated downloads directory')
    scratch = Path(temp_root or tempfile.gettempdir()).resolve(strict=True)
    if scratch == state or state in scratch.parents:
        raise ValueError('temporary state must be outside the original cache')
    if downloads.is_symlink() or any(path.is_symlink() for path in downloads.rglob('*')):
        raise ValueError('isolated download measurement requires sources without symbolic links')
    version, _ = run([builder, '--version'], seconds=30, quiet=True)
    zstd_version, _ = run(['zstd', '--version'], seconds=30, quiet=True)
    apparent, _ = run(['du', '--apparent-size', '--block-size=1', '--summarize', '--', downloads], quiet=True)
    result = {
        'status': 'success', 'architecture': arch, 'manifest': str(manifest),
        'builder_version': version.strip(), 'zstd_version': zstd_version.strip(),
        'downloads_apparent_bytes': int(apparent.split(maxsplit=1)[0]),
        'compression': {},
        'limits': 'Canonical tar+zstd proxy sizes, not Actions-cache archive/transport measurements. '
                  'One paired source-fetch observation, not a build, safe-pruning decision or representative speedup. '
                  'Download commands use isolated states; original state is read for copying/compression only.',
    }
    with tempfile.TemporaryDirectory(prefix='flatpak-cache-probe-', dir=scratch) as directory:
        root = Path(directory)
        for condition in ('warm', 'fresh'):
            isolated = root / condition
            isolated.mkdir()
            if condition == 'warm':
                shutil.copytree(downloads, isolated / 'downloads')
            _, elapsed = run([
                builder, '--download-only', '--disable-rofiles-fuse', '--force-clean',
                '--arch=' + arch, '--state-dir', isolated,
                root / (condition + '-build'), manifest,
            ])
            result[condition + '_download_seconds'] = elapsed

        for condition, exclusions in (
            ('full', []),
            ('without_downloads', ['downloads']),
            # Exclude both dependency ccache and the isolated OBS zccache
            # store nested in the same builder-provided mount.
            ('without_downloads_or_compiler', ['downloads', 'ccache']),
        ):
            options = ['--exclude=' + state.name + '/' + name for name in exclusions]
            output, elapsed = run([
                'bash', '-o', 'pipefail', '-c', ARCHIVE_SCRIPT,
                'flatpak-cache-probe', state.parent, *options, state.name,
            ], seconds=120, quiet=True)
            result['compression'][condition] = {'bytes': int(output.strip()), 'seconds': elapsed}
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state', type=Path, default=Path('.flatpak-builder'))
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--arch', default='x86_64')
    parser.add_argument('--temp-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = measure(args.state, args.manifest, args.arch, temp_root=args.temp_root)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        args.output.write_text(json.dumps({'status': 'failed', 'error': str(error)}, indent=2) + '\n')
        raise
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
