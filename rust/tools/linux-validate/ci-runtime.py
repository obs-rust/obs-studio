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
import datetime
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.parse
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
# The runner's mirror and the fallbacks, one rule for the build container's
# Install Git step and these containers (#5510).
spec = importlib.util.spec_from_file_location(
    'host_apt_mirrors', Path(__file__).resolve().parents[3] / 'ci' / 'host_apt_mirrors.py')
host_apt_mirrors = importlib.util.module_from_spec(spec)
spec.loader.exec_module(host_apt_mirrors)
spec = importlib.util.spec_from_file_location('serial_runner', Path(__file__).with_name('serial-runner.py'))
serial_runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(serial_runner)


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


def cpu_plan(part: str, reservation: str, isolation: str, parent: str = '') -> tuple[list[str], list[str]]:
    environment = [*part_env(part), 'OBS_CTEST_JOBS=8']
    if not reservation and not isolation and not parent:
        return ['--cpus', '4'], environment
    if part != 'all' or not reservation or not isolation:
        raise ValueError('CPU reservation and isolation must both be supplied for the complete suite')
    cpus = set(serial_runner.parse_cpus(reservation))
    isolated = set(serial_runner.parse_cpus(isolation))
    suite = cpus - isolated
    if parent and not re.fullmatch(r'obs_tier2_[a-z0-9_]+\.slice', parent):
        raise ValueError('invalid Tier 2 CPU reservation slice')
    if (not isolated < cpus or len(isolated) < 2 or len(isolated) % 2
            or (not parent and not cpus <= os.sched_getaffinity(0)) or len(suite) < 2):
        raise ValueError('CPU reservation needs complete isolated pairs and separate suite CPUs within runner affinity')
    # On the measured eight-core suite, twelve workers hide I/O and timer
    # waits. The kernel still restricts those workers to the same CPU set.
    workers = 12 if len(suite) == 8 else len(suite)
    parent_args = ['--cgroup-parent', parent] if parent else []
    return [*parent_args, '--cpuset-cpus', serial_runner.format_cpus(sorted(cpus)), '--cpus', str(len(cpus))], [
        'OBS_CTEST_PART=', 'OBS_CTEST_SHARD=', 'OBS_CTEST_SERIAL_SHARD=',
        f'OBS_TIER2_SERIAL_CPUS={serial_runner.format_cpus(sorted(isolated))}',
        f'OBS_TIER2_ISOLATED_SCHEDULE={SOURCE}/rust/tools/linux-validate/isolation-schedule.json',
        f'OBS_CTEST_JOBS={workers}',
    ]


def use_cpu_grant(args: argparse.Namespace, grant: object) -> None:
    if args.part != 'all' or getattr(args, 'reserved_cpus', '') or getattr(args, 'isolated_cpus', '') or grant is None:
        return
    if not isinstance(grant, dict) or grant.keys() != {'parent', 'reserved', 'isolated'} or any(
            not isinstance(value, str) or not value for value in grant.values()):
        print('Invalid Tier 2 CPU grant. Using ordinary CTest scheduling.', flush=True)
        return
    try:
        cpu_plan(args.part, grant['reserved'], grant['isolated'], grant['parent'])
    except ValueError:
        print('Invalid Tier 2 CPU grant. Using ordinary CTest scheduling.', flush=True)
        return
    args.cpu_parent = grant['parent']
    args.reserved_cpus = grant['reserved']
    args.isolated_cpus = grant['isolated']


def mounts(source: Path, build: Path, cost: Path, uv: Path, archives: Path) -> list[str]:
    result = []
    for path, target, readonly in (
        (source, SOURCE, True), (build, BUILD, False), (cost, COST, False),
        (uv, '/usr/local/bin/uv', True), (archives, '/var/cache/apt/archives', False),
    ):
        result.extend(('--mount', f'type=bind,src={path.resolve()},dst={target}' + (',readonly' if readonly else '')))
    return result


# The host's apt mirror list and the containers' main source (#5510).
HOST_MIRRORS = Path('/etc/apt/apt-mirrors.txt')
SOURCES = '/etc/apt/sources.list.d/ubuntu.sources'


# Point the main source (every stanza but the security one) at a mirror, only
# when the sources have the stock security stanza; never fail on it.
USE_MIRROR = ('grep -qx "URIs: http://security.ubuntu.com/ubuntu/" "$0" && '
              'sed -i "/security\\.ubuntu\\.com/!s|^URIs: .*|URIs: $1|" "$0" || '
              'echo "keeping the sources of $0"')


def use_mirror(ident: str, uri: str) -> None:
    """The container's main source points at `uri` (#5510)."""
    print(f'Runtime delta mirror: {uri}', flush=True)
    command('docker', 'exec', ident, 'sh', '-c', USE_MIRROR, SOURCES, uri)


# timeout(1) exits 124 when the bound stops a command, 137 when it has to kill it.
TIMED_OUT = (124, 137)


def install_once(install: tuple[str, ...], recover: tuple[str, ...], next_mirror) -> None:
    """The delta install; a timeout retries it once (#5504). A slow mirror can
    stop apt (run 45113911635). A timeout in the unpack leaves dpkg
    interrupted, which fails every later apt-get until `dpkg --configure -a`.
    The retry fetches the index from the next mirror (#5510); the .debs
    downloaded so far stay in the archive mount. Other errors still fail at
    once (#5249)."""
    try:
        command(*install, capture_stderr=True)
    except subprocess.CalledProcessError as error:
        if error.returncode not in TIMED_OUT:
            raise
        print(f'Runtime delta install timed out (exit {error.returncode}); '
              'recovering dpkg and retrying once', flush=True)
        try:
            command(*recover, capture_stderr=True)
        except subprocess.CalledProcessError as recovery:
            print(f'dpkg --configure -a failed (exit {recovery.returncode}); retrying anyway', flush=True)
        next_mirror()
        command(*install, capture_stderr=True)


# apt's own wording for a pinned version that no index lists.
MISSING_PIN = re.compile(r"E: Version '([^'\n]+)' for '([^'\n]+)' was not found")
ARCHIVES = '/var/cache/apt/archives'


def cached_pins(packages: list[str], output: str, archives: Path | None) -> list[str] | None:
    """`packages` with each pin the index lost swapped for its cached .deb (#5579).

    The build can read a mirror that still lists a version the test
    container's mirror has already superseded (pipewire-pulse 1.2 against a
    phased 1.3, run 45405030842). The build took that .deb from the same
    restored apt cache that is mounted here, so install exactly that version.
    None when a lost pin has no single cached .deb of its exact version.
    """
    def key(name: str, version: str) -> tuple[str, str]:
        # apt reports `pipewire-pulse` for the pin `pipewire-pulse:amd64=...`.
        return name.partition(':')[0], version

    missing = {key(name, version) for version, name in MISSING_PIN.findall(output)}
    pins = {key(*package.partition('=')[::2]): package for package in packages}
    if archives is None or not missing or not missing <= pins.keys():
        return None
    result = []
    for package in packages:
        name, _, version = package.partition('=')
        if key(name, version) not in missing:
            result.append(package)
            continue
        # apt names cached files <package>_<version, ':' as %3a>_<arch>.deb.
        stem = f'{key(name, version)[0]}_{version.replace(":", "%3a")}_'
        found = [path for path in archives.iterdir()
                 if path.name.startswith(stem) and path.name.endswith('.deb') and path.is_file()]
        if len(found) != 1:
            return None
        result.append(f'{ARCHIVES}/{found[0].name}')
    return result


LAUNCHPAD = 'https://api.launchpad.net/1.0/ubuntu/+archive/primary'


def missing_pins(output: str, packages: list[str]) -> list[tuple[str, str, str]]:
    """(name, version, architecture) of each pin apt reports as not found."""
    arches = {}
    for package in packages:
        name, _, version = package.partition('=')
        base, _, arch = name.partition(':')
        arches[base, version] = arch or 'amd64'
    return [(name.partition(':')[0], version, arches.get((name.partition(':')[0], version), 'amd64'))
            for version, name in MISSING_PIN.findall(output)]


def snapshot_before_supersession(pins: list[tuple[str, str, str]]) -> str:
    """The snapshot.ubuntu.com ID one second before the first of `pins` left
    the archive index (#5578). The build resolved every pin against an index
    that still listed it, so at that time each pin was still published and
    none of them had been replaced yet."""
    times = []
    for name, version, arch in pins:
        # Launchpad lists an architecture-independent binary on every architecture.
        arch = 'amd64' if arch == 'all' else arch
        query = urllib.parse.urlencode({'ws.op': 'getPublishedBinaries', 'binary_name': name,
                                        'version': version, 'exact_match': 'true'})
        with urllib.request.urlopen(f'{LAUNCHPAD}?{query}', timeout=30) as reply:
            entries = json.load(reply)['entries']
        gone = [entry['date_superseded'] or entry['date_removed'] for entry in entries
                if entry['distro_arch_series_link'].rstrip('/').endswith('/' + arch)]
        gone = [datetime.datetime.fromisoformat(stamp) for stamp in gone if stamp]
        if not gone:
            raise ValueError(f'Launchpad has no supersession of {name} {version} on {arch}')
        times.append(max(gone))
    before = min(times).replace(microsecond=0) - datetime.timedelta(seconds=1)
    return before.astimezone(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')


def install_prepared_delta(ident: str, packages: list[str], archives: Path | None = None) -> None:
    # Normal delta setup is about 45 s (#5237); allow 4x that per apt command.
    # apt.conf(5) delegates per-transport Timeout to apt-transport-http/https(1).
    bound = ('docker', 'exec', ident, 'timeout', '--kill-after=10s', '180s', 'env', 'LC_ALL=C')
    apt = (*bound, 'apt-get', '-o', 'Acquire::Retries=3',
           '-o', 'Acquire::http::Timeout=15', '-o', 'Acquire::https::Timeout=15')
    install = (*apt, 'install', '-y', '--no-install-recommends', *packages)
    recover = (*bound, 'dpkg', '--configure', '-a')
    mirrors = host_apt_mirrors.container_mirrors(HOST_MIRRORS)

    def next_mirror() -> None:
        use_mirror(ident, mirrors[1 % len(mirrors)])
        command(*apt, 'update', '-qq', capture_stderr=True)

    try:
        use_mirror(ident, mirrors[0])
        command(*apt, 'update', '-qq', capture_stderr=True)
        try:
            install_once(install, recover, next_mirror)
        except subprocess.CalledProcessError as error:
            if error.returncode != 100 or not re.search(
                    r"E: Version '[^'\n]+' for '[^'\n]+' was not found", error.output or ''):
                raise
            # Refresh once for a superseded pin; never substitute newer versions.
            print('Runtime delta pin unavailable; refreshing package indexes once', flush=True)
            command(*apt, 'update', '-qq', capture_stderr=True)
            try:
                install_once(install, recover, next_mirror)
            except subprocess.CalledProcessError as still:
                if still.returncode != 100:
                    raise
                cached = cached_pins(packages, still.output or '', archives)
                if cached is not None:
                    print('Runtime delta pin still unavailable; installing the cached .deb of that exact version: '
                          + ' '.join(c for c in cached if c not in packages), flush=True)
                    install_once((*apt, 'install', '-y', '--no-install-recommends', *cached), recover, next_mirror)
                    return
                # No cached .deb (the build fetched it from a mirror that had
                # not synced yet): take the same pins from the Ubuntu snapshot
                # just before they were superseded (#5578).
                pins = missing_pins(still.output or '', packages)
                if not pins:
                    raise
                try:
                    snapshot = snapshot_before_supersession(pins)
                except (OSError, ValueError) as lookup:
                    print(f'Runtime delta snapshot lookup failed: {lookup}', flush=True)
                    raise still from lookup
                print(f'Runtime delta pins superseded: {pins}; installing them from snapshot {snapshot}', flush=True)
                at = (*apt, '--snapshot', snapshot)
                command(*at, 'update', '-qq', capture_stderr=True)
                command(*at, *install[len(apt):], capture_stderr=True)
    except subprocess.CalledProcessError as error:
        raise RuntimeError(f'Runtime delta apt failed (exit {error.returncode}) for '
                           f'{" ".join(packages)}:\n{error.output or ""}') from error


def install_packages(ident: str, variant: str, requirements: str, changed: list[str],
                     archives: Path | None = None) -> None:
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
            install_prepared_delta(ident, packages, archives)
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
    cpu_args, test_env = cpu_plan(args.part, getattr(args, 'reserved_cpus', ''),
                                  getattr(args, 'isolated_cpus', ''), getattr(args, 'cpu_parent', ''))
    ident = command('docker', 'create', '--platform', 'linux/amd64', *cpu_args,
                    '--env', 'DEBIAN_FRONTEND=noninteractive',
                    *summary_args,
                    *mounts(args.source, args.build, args.cost, args.uv, args.archives),
                    image or BASE, 'sleep', 'infinity')
    if not re.fullmatch(r'[0-9a-f]{64}', ident):
        raise ValueError('Invalid runtime container identity')
    try:
        command('docker', 'start', ident)
        install_packages(ident, variant, requirements, changed, args.archives)
        inventory = command('docker', 'exec', ident, 'dpkg-query', '-W', '-f=${binary:Package}\t${Version}\n')
        if not selection.satisfies(requirements, inventory):
            raise ValueError('Runtime does not satisfy the current artifact package closure')
        if variant == 'prepared' and not changed and \
                selection.records(inventory, '\t') != selection.records(descriptor['inventory'], '\t'):
            raise ValueError('Prepared runtime inventory differs from the producer')
        report = {'variant': variant, 'setup_seconds': time.monotonic() - started,
                  'image': image or BASE, 'mode': args.mode, 'part': args.part, 'delta': changed,
                  'inventory_verified': True}
        if parent := getattr(args, 'cpu_parent', ''):
            report['cpu_plan'] = {'parent': parent, 'reserved': args.reserved_cpus, 'isolated': args.isolated_cpus}
        (args.state / 'runtime-measurement.json').write_text(json.dumps(report, indent=2) + '\n')
        print(json.dumps(report), flush=True)
        subprocess.run(['docker', 'exec', '--workdir', SOURCE,
                        '--env', f'OBS_TIER2_SRC={SOURCE}', '--env', f'OBS_TIER2_BUILD={BUILD}',
                        '--env', f'OBS_TIER2_COST_DIR={COST}', '--env', 'OBS_TIER2_STAGE=test',
                        '--env', f'OBS_TIER2_MODES={args.mode}',
                        *(arg for env in test_env for arg in ('--env', env)),
                        ident, 'bash', f'{SOURCE}/rust/tools/linux-validate/validate.sh'], check=True)
    finally:
        command('docker', 'stop', '--timeout', '1', ident)


def execute_shared(args: argparse.Namespace) -> int:
    """Run under a host lease, or return EX_TEMPFAIL before starting tests."""
    token = os.environ.pop('OBS_TIER2_ARTIFACT_TOKEN', '')
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as lease, tempfile.TemporaryDirectory() as temp:
        try:
            lease.settimeout(5)
            lease.connect(os.environ.get('OBS_TIER2_ARTIFACT_SOCKET', ''))
            # A cold fill includes the GitHub transfer and image materialization.
            lease.settimeout(600)
            request = dict(run=os.environ.get('GITHUB_RUN_ID', ''), head=os.environ.get('GITHUB_SHA', ''), token=token)
            lease.sendall((json.dumps(request) + '\n').encode())
            with lease.makefile('rb') as response:
                reply = json.loads(response.readline(2 << 20))
            if not isinstance(reply, dict):
                raise ValueError('invalid shared Tier 2 lease')
            path = reply.get('path')
            packages = reply.get('packages')
            if (not isinstance(path, str)
                    or not re.fullmatch(r'/run/obs-tier2-artifacts/sessions/tier2-job-[a-z0-9_]+/merged', path)
                    or not isinstance(packages, str) or len(packages) > 1 << 20):
                raise ValueError('shared Tier 2 lease was unavailable')
            args.build = Path(path)
            args.packages = Path(temp) / 'runtime-packages.txt'
            args.packages.write_text(packages)
            if reply.get('cpu_plan') is not None and hasattr(args, 'part'):
                use_cpu_grant(args, reply['cpu_plan'])
        except (OSError, ValueError):
            print('Tier 2 shared artifact unavailable. Using normal artifact delivery.', flush=True)
            return 75
        execute(args)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('descriptor', 'source', 'cost', 'uv', 'archives', 'state'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--packages', type=Path)
    parser.add_argument('--build', type=Path)
    parser.add_argument('--shared-artifact', action='store_true')
    parser.add_argument('--prebaked-image', default='', help='Selected public Dell image, pinned by digest')
    parser.add_argument('--mode', choices=('OFF', 'ON'), required=True)
    parser.add_argument('--part', required=True, help='all, suite, serial or serial-<i>-of-<n>')
    parser.add_argument('--reserved-cpus', default='', help='Physical CPU set reserved for this job by the runner host')
    parser.add_argument('--isolated-cpus', default='', help='Even subset of reserved CPUs for isolated test pairs')
    args = parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS') != 'true':
        parser.error('Docker execution requires a disposable CI runner; local Docker work uses bosn')
    if args.shared_artifact:
        return execute_shared(args)
    if args.packages is None or args.build is None:
        parser.error('--packages and --build are required for normal artifact delivery')
    execute(args)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
