#!/usr/bin/env -S uv run --script
"""Execute complete Tier 2 suites in a verified runtime on disposable CI hosts.

The prepared image is an optimization of package installation only. A closure
that differs in up to MAX_DELTA packages (an Ubuntu update since the image was
made) installs those exact versions on top of it; a larger difference or an
unavailable release falls back to the pinned stock base. Corrupt
bytes, image identities and inventory mismatches fail instead of falling back.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

BASE = 'ubuntu:26.04@sha256:f144425ff09be612d6d9ad965196e9cdc23dae1f42110a8a11a3e9a8198759f7'
SOURCE = '/__w/obs-studio/obs-studio'
BUILD = '/__w/_temp/tier2'
COST = '/__w/_temp/tier2-cost'
# One zlib update made every job install the whole closure for 75 s (#1592).
MAX_DELTA = 16
ASSET_CACHE_DIR = Path('/var/cache/obs-runner/assets')
ASSET_CACHE_MARKER = Path('/var/cache/obs-runner/.persistent-cache')
spec = importlib.util.spec_from_file_location('runtime_selection', Path(__file__).with_name('runtime-selection.py'))
selection = importlib.util.module_from_spec(spec)
spec.loader.exec_module(selection)


def command(*args: str, capture_stderr: bool = False) -> str:
    return subprocess.check_output(args, text=True,
                                   stderr=subprocess.STDOUT if capture_stderr else None).strip()


def verify_archive(path: Path, descriptor: dict) -> None:
    with path.open('rb') as stream:
        digest = hashlib.file_digest(stream, 'sha256').hexdigest()
    if path.stat().st_size != descriptor['compressed_bytes'] or digest != descriptor['archive_sha256']:
        raise ValueError('Prepared archive size or checksum mismatch')


def prepared_image(descriptor: dict, requirements: str, root: Path) -> str | None:
    if descriptor['base'] != BASE or descriptor['platform'] != 'linux/amd64':
        raise ValueError('Prepared runtime base or platform mismatch')
    changed = selection.delta(requirements, descriptor['inventory'])
    if len(changed) > MAX_DELTA:
        print(f'Runtime selection: stock ({len(changed)} packages differ, more than {MAX_DELTA})', flush=True)
        return None
    return download_prepared(descriptor, root)


def asset_cache_directory() -> Path | None:
    """Use only the marker-gated persistent Dell mount."""
    try:
        if ASSET_CACHE_MARKER.is_file() and ASSET_CACHE_DIR.is_dir():
            return ASSET_CACHE_DIR
    except OSError as error:
        print(f'Prepared runtime asset cache: unavailable ({error})', flush=True)
    return None


def cached_archive(descriptor: dict, archive: Path) -> tuple[Path | None, bool]:
    """Copy and verify optional persistent bytes before using them in this job."""
    directory = asset_cache_directory()
    if directory is None:
        return None, False
    digest = descriptor['archive_sha256']
    if not re.fullmatch(r'[0-9a-f]{64}', digest):
        raise ValueError('Invalid prepared archive checksum')
    cached = directory / digest
    try:
        # Verify the job's copy, so concurrent cache replacement cannot change it.
        shutil.copyfile(cached, archive)
        verify_archive(archive, descriptor)
        os.utime(cached, None)
    except (FileNotFoundError, ValueError) as error:
        print(f'Prepared runtime asset cache: miss ({error})', flush=True)
        return cached, False
    except OSError as error:
        print(f'Prepared runtime asset cache: read failed, using job download ({error})', flush=True)
        return None, False
    print('Prepared runtime asset cache: hit', flush=True)
    return cached, True


def download_archive(descriptor: dict, archive: Path) -> None:
    with urllib.request.urlopen(descriptor['url'], timeout=60) as response, archive.open('wb') as output:
        while block := response.read(1024 * 1024):
            output.write(block)
    verify_archive(archive, descriptor)


def download_cached_archive(descriptor: dict, archive: Path, cached: Path) -> bool:
    """Download to the shared .part first; cache errors retry in the job directory."""
    try:
        with tempfile.NamedTemporaryFile(prefix=f'.{cached.name}.', suffix='.part',
                                         dir=cached.parent, delete=False) as temporary:
            staging = Path(temporary.name)
        try:
            download_archive(descriptor, staging)
            staging.chmod(0o644)
            shutil.copyfile(staging, archive)
            verify_archive(archive, descriptor)
            # Another uid can own an existing entry in the sticky directory.
            # Keep it for host GC; this job uses its verified local copy.
            if not cached.exists():
                os.replace(staging, cached)
        finally:
            staging.unlink(missing_ok=True)
    except OSError as error:
        print(f'Prepared runtime asset cache: write failed, using job download ({error})', flush=True)
        return False
    return True


def download_prepared(descriptor: dict, root: Path) -> str | None:
    image = descriptor['image_id']
    if not re.fullmatch(r'sha256:[0-9a-f]{64}', image):
        raise ValueError('Invalid prepared image identity')
    url = descriptor['url']
    if not url.startswith('https://github.com/obs-rust/obs-studio-libs/releases/download/'):
        raise ValueError('Prepared runtime must use the approved immutable asset route')
    try:
        present = command('docker', 'image', 'inspect', '--format', '{{.Id}}', image) == image
    except (OSError, subprocess.CalledProcessError):
        present = False
    print(f'Prepared runtime image cache: {"hit" if present else "miss"}', flush=True)
    if present:
        return image
    archive = root / 'runtime.tar.zst'
    cached, cache_hit = cached_archive(descriptor, archive)
    if not cache_hit:
        try:
            if cached is None or not download_cached_archive(descriptor, archive, cached):
                download_archive(descriptor, archive)
        except (urllib.error.URLError, TimeoutError):
            print('Runtime selection: stock (prepared asset unavailable)', flush=True)
            return None
    unpacked = root / 'runtime.tar'
    command('zstd', '-d', '-f', str(archive), '-o', str(unpacked))
    command('docker', 'image', 'load', '--input', str(unpacked))
    if command('docker', 'image', 'inspect', '--format', '{{.Id}}', image) != image:
        raise ValueError('Prepared image identity mismatch')
    return image


def part_env(part: str) -> list[str]:
    """The validate.sh environment for a Tier 2 part (#2071, #2072)."""
    if part == 'all':
        # No include/exclude/shard selector: execute the complete registry.
        return ['OBS_CTEST_PART=', 'OBS_CTEST_SHARD=', 'OBS_CTEST_SERIAL_SHARD=',
                'OBS_TIER2_SERIAL_CPUS=']
    if part in ('suite', 'serial'):
        return [f'OBS_CTEST_PART={part}']
    shard = re.fullmatch(r'serial-([1-9][0-9]*)-of-([1-9][0-9]*)', part)
    if not shard or int(shard[1]) > int(shard[2]):
        raise ValueError(f'Invalid Tier 2 part: {part}')
    return ['OBS_CTEST_PART=serial', f'OBS_CTEST_SERIAL_SHARD={shard[1]}/{shard[2]}']


def mounts(source: Path, build: Path, cost: Path, uv: Path, archives: Path) -> list[str]:
    result = []
    for path, target, readonly in (
        (source, SOURCE, True), (build, BUILD, False), (cost, COST, False),
        (uv, '/usr/local/bin/uv', True), (archives, '/var/cache/apt/archives', False),
    ):
        result.extend(('--mount', f'type=bind,src={path.resolve()},dst={target}' + (',readonly' if readonly else '')))
    return result


def install_prepared_delta(ident: str, packages: list[str]) -> None:
    # Normal delta setup is about 45 s (#5237); allow 4x that per apt command.
    # apt.conf(5) delegates per-transport Timeout to apt-transport-http/https(1).
    apt = ('docker', 'exec', ident, 'timeout', '--kill-after=10s', '180s',
           'env', 'LC_ALL=C', 'apt-get', '-o', 'Acquire::Retries=3',
           '-o', 'Acquire::http::Timeout=15', '-o', 'Acquire::https::Timeout=15')
    install = (*apt, 'install', '-y', '--no-install-recommends', *packages)
    try:
        command(*apt, 'update', '-qq', capture_stderr=True)
        try:
            command(*install, capture_stderr=True)
        except subprocess.CalledProcessError as error:
            if error.returncode != 100 or not re.search(
                    r"E: Version '[^'\n]+' for '[^'\n]+' was not found", error.output or ''):
                raise
            # Refresh once for a superseded pin; never substitute newer versions.
            print('Runtime delta pin unavailable; refreshing package indexes once', flush=True)
            command(*apt, 'update', '-qq', capture_stderr=True)
            command(*install, capture_stderr=True)
    except subprocess.CalledProcessError as error:
        raise RuntimeError(f'Runtime delta apt failed (exit {error.returncode}) for '
                           f'{" ".join(packages)}:\n{error.output or ""}') from error


def install_packages(ident: str, variant: str, requirements: str, changed: list[str]) -> None:
    # Prebaked confirms exact installed versions; stock/prepared retain their installs.
    packages = changed
    if variant == 'prebaked':
        packages = requirements.splitlines()
    if variant == 'stock':
        packages = [*requirements.splitlines(), 'git', 'ca-certificates', 'curl', 'sudo', 'zstd']
    elif changed:
        print('Runtime selection: prepared plus ' + ' '.join(changed), flush=True)
    if packages:
        command('docker', 'exec', ident, 'sh', '-c',
                'if test -f /etc/apt/apt.conf.d/docker-clean; then '
                'mv /etc/apt/apt.conf.d/docker-clean /tmp/docker-clean.disabled; fi')
        if variant == 'prepared':
            install_prepared_delta(ident, packages)
            return
        if variant != 'prebaked':
            command('docker', 'exec', ident, 'apt-get', 'update', '-qq')
        command('docker', 'exec', ident, 'apt-get', 'install', '-y', '--no-install-recommends', *packages)


def execute(args: argparse.Namespace) -> None:
    started = time.monotonic()
    requirements = args.packages.read_text()
    selection.records(requirements, '=')
    prebaked = getattr(args, 'prebaked_image', '')
    if prebaked and not re.fullmatch(r'ghcr\.io/obs-rust/tier2-linux@sha256:[0-9a-f]{64}', prebaked):
        raise ValueError('Invalid prebaked Tier 2 digest')
    descriptor = {} if prebaked else json.loads(args.descriptor.read_text())
    args.state.mkdir(parents=True, exist_ok=True)
    args.cost.mkdir(parents=True, exist_ok=True)
    image = prebaked or prepared_image(descriptor, requirements, args.state)
    variant = 'prebaked' if prebaked else ('prepared' if image else 'stock')
    changed = selection.delta(requirements, descriptor['inventory']) if variant == 'prepared' else []
    if prebaked:
        command('docker', 'pull', prebaked)
    if image is None:
        command('docker', 'pull', BASE)
    summary_args = []
    if summary := os.environ.get('GITHUB_STEP_SUMMARY'):
        summary_path = Path(summary).resolve()
        summary_path.touch(exist_ok=True)
        summary_args = ['--mount', f'type=bind,src={summary_path},dst=/tier2-step-summary',
                        '--env', 'GITHUB_STEP_SUMMARY=/tier2-step-summary']
    ident = command('docker', 'create', '--platform', 'linux/amd64', '--cpus', '4',
                    '--env', 'DEBIAN_FRONTEND=noninteractive',
                    *summary_args,
                    *mounts(args.source, args.build, args.cost, args.uv, args.archives),
                    image or BASE, 'sleep', 'infinity')
    if not re.fullmatch(r'[0-9a-f]{64}', ident):
        raise ValueError('Invalid runtime container identity')
    try:
        command('docker', 'start', ident)
        install_packages(ident, variant, requirements, changed)
        inventory = command('docker', 'exec', ident, 'dpkg-query', '-W', '-f=${binary:Package}\t${Version}\n')
        if not selection.satisfies(requirements, inventory):
            raise ValueError('Runtime does not satisfy the current artifact package closure')
        if variant == 'prepared' and not changed and \
                selection.records(inventory, '\t') != selection.records(descriptor['inventory'], '\t'):
            raise ValueError('Prepared runtime inventory differs from the producer')
        report = {'variant': variant, 'setup_seconds': time.monotonic() - started,
                  'image': image or BASE, 'mode': args.mode, 'part': args.part, 'delta': changed,
                  'inventory_verified': True}
        (args.state / 'runtime-measurement.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report), flush=True)
        subprocess.run(['docker', 'exec', '--workdir', SOURCE,
                        '--env', f'OBS_TIER2_SRC={SOURCE}', '--env', f'OBS_TIER2_BUILD={BUILD}',
                        '--env', f'OBS_TIER2_COST_DIR={COST}', '--env', 'OBS_TIER2_STAGE=test',
                        '--env', f'OBS_TIER2_MODES={args.mode}', '--env', 'OBS_CTEST_JOBS=8',
                        *(arg for env in part_env(args.part) for arg in ('--env', env)),
                        ident, 'bash', f'{SOURCE}/rust/tools/linux-validate/validate.sh'], check=True)
    finally:
        command('docker', 'stop', '--timeout', '1', ident)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('descriptor', 'packages', 'source', 'build', 'cost', 'uv', 'archives', 'state'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--prebaked-image', default='', help='Selected public Dell image, pinned by digest')
    parser.add_argument('--mode', choices=('OFF', 'ON'), required=True)
    parser.add_argument('--part', required=True, help='all, suite, serial or serial-<i>-of-<n>')
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('Docker execution requires a disposable CI runner; local Docker work uses bosn')
    execute(args)


if __name__ == '__main__':
    main()
