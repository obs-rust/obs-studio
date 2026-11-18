"""Pinned nextest installer used only when selected Rust tests actually run.

Linux uses a static upstream binary. Other hosts keep Cargo's existing runner.
No remote cache is published; the local archive is verified before every use.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import platform
import random
import shutil
import subprocess
import tarfile
import tempfile
import time
from urllib.error import HTTPError, URLError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener, urlopen

import host_platform as host

VERSION = '0.9.143'
ARCHIVES = {
    'x86_64': ('x86_64-unknown-linux-musl', '6d891c18105ec2d33f6e441a4f92b7ccab47ba263e22055c8bb66884243f3389'),
    'aarch64': ('aarch64-unknown-linux-musl', '0560ce0ce017f368c54b5db86588370fc03b470985490fa81200e62a657dda05'),
}
# Download budget: ATTEMPTS tries, sleeping BACKOFF * 2**n (+ up to 50% jitter)
# between them: ~5+10+20+40 s, about 75-110 s worst case with the 30 s timeouts.
ATTEMPTS = 5
BACKOFF = 5.0
TIMEOUT = 30
# With a workflow token the asset comes through the REST API: GitHub can
# answer anonymous release downloads with 504 for hours while authenticated
# requests work (2026-11-09, #4209). Local runs have no token and keep the
# anonymous URL.
RELEASES_API = 'https://api.github.com/repos/nextest-rs/nextest/releases'


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


def installed_nextest() -> Path | None:
    """A cargo-nextest of exactly VERSION already on PATH or in $CARGO_HOME/bin."""
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo'))
    candidates = [cargo_home / 'bin' / 'cargo-nextest']
    found = shutil.which('cargo-nextest')
    if found:
        candidates.insert(0, Path(found))
    for candidate in candidates:
        if not (candidate.is_file() and os.access(candidate, os.X_OK)):
            continue
        try:
            result = subprocess.run([str(candidate), 'nextest', '--version'], capture_output=True,
                                    text=True, timeout=30, check=False)
        except (OSError, subprocess.TimeoutExpired):
            continue
        words = result.stdout.split()
        if result.returncode == 0 and VERSION in words:
            return candidate
        print(f'+ ignoring {candidate}: not nextest {VERSION} ({result.stdout.strip()!r})', flush=True)
    return None


def transient(error: BaseException) -> bool:
    if isinstance(error, HTTPError):
        return error.code >= 500 or error.code in (408, 429)
    return isinstance(error, (TimeoutError, ConnectionError, URLError))


class SameHostAuthRedirect(HTTPRedirectHandler):
    """Follow redirects, but keep the token on the host it was meant for.

    The API answers an asset request with a redirect to a signed URL on
    another host, which needs no token and must not receive it. urllib
    copies every header to the redirected request otherwise.
    """

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        new = super().redirect_request(req, fp, code, msg, headers, newurl)
        if new is not None and urlsplit(newurl).netloc != urlsplit(req.full_url).netloc:
            new.remove_header('Authorization')
        return new


def authenticated_open(request: Request, timeout: float):
    return build_opener(SameHostAuthRedirect()).open(request, timeout=timeout)


def github_token() -> str | None:
    for name in ('GITHUB_TOKEN', 'GH_TOKEN'):
        value = os.environ.get(name, '').strip()
        if value:
            return value
    return None


def open_asset(name: str, url: str):
    """The release asset: anonymous without a token, else through the REST API."""
    token = github_token()
    if token is None:
        return urlopen(url, timeout=TIMEOUT)
    headers = {'Authorization': f'Bearer {token}', 'X-GitHub-Api-Version': '2022-11-28',
               'User-Agent': f'obs-rust-ci-nextest/{VERSION}'}
    tag = f'{RELEASES_API}/tags/cargo-nextest-{VERSION}'
    with authenticated_open(Request(tag, headers={**headers, 'Accept': 'application/vnd.github+json'}),
                            timeout=TIMEOUT) as response:
        release = json.load(response)
    asset = next((item for item in release.get('assets', []) if item.get('name') == name), None)
    if asset is None:
        raise ValueError(f'nextest release cargo-nextest-{VERSION} has no asset {name}')
    return authenticated_open(Request(asset['url'], headers={**headers, 'Accept': 'application/octet-stream'}),
                              timeout=TIMEOUT)


def download(url: str, staged: Path) -> None:
    """Fetch url into staged, retrying only transient failures with jittered backoff."""
    failures: list[str] = []
    for attempt in range(1, ATTEMPTS + 1):
        try:
            with open_asset(staged.name, url) as response, staged.open('wb') as destination:
                shutil.copyfileobj(response, destination)
            return
        except (HTTPError, URLError, TimeoutError, ConnectionError) as error:
            if isinstance(error, HTTPError):
                error.close()  # an HTTPError owns the open error response
                reason = f'HTTP {error.code}'
            else:
                reason = f'{type(error).__name__}: {error}'
            failures.append(f'attempt {attempt}: {reason}')
            if not transient(error):
                break
            if attempt < ATTEMPTS:
                delay = BACKOFF * 2 ** (attempt - 1) * (1 + random.random() / 2)
                print(f'+ nextest download attempt {attempt}/{ATTEMPTS} failed ({reason}); retry in {delay:.0f} s',
                      flush=True)
                time.sleep(delay)
    source = url if github_token() is None else f'{url} (through the REST API with the job token)'
    raise RuntimeError(
        f'nextest {VERSION} download failed after {len(failures)} of {ATTEMPTS} attempts from {source}: '
        f'{"; ".join(failures)}. Rust tests were NOT run. Recovery: re-run the job (GitHub release CDN '
        f'failures are usually transient), or install the pinned binary with '
        f'`soldr cargo install cargo-nextest --version {VERSION} --locked` so it is found on PATH or in '
        f'$CARGO_HOME/bin, or place the verified archive at {staged.parent.parent / "nextest.tar.gz"}.')


def ensure_nextest() -> Path | None:
    if not host.is_linux() or platform.machine() not in ARCHIVES:
        return None
    installed = installed_nextest()
    if installed is not None:
        print(f'+ using installed nextest {VERSION}: {installed}', flush=True)
        return installed
    target, digest = ARCHIVES[platform.machine()]
    root = Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'obs-rust' / 'nextest' / VERSION / target
    root.mkdir(parents=True, exist_ok=True)
    archive = root / 'nextest.tar.gz'
    if archive.is_file():
        print(f'+ using cached nextest {VERSION} archive: {archive}', flush=True)
    else:
        name = f'cargo-nextest-{VERSION}-{target}.tar.gz'
        url = f'https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-{VERSION}/{name}'
        print(f'+ download pinned nextest {VERSION} ({target})', flush=True)
        with tempfile.TemporaryDirectory(dir=root) as tmp:
            staged = Path(tmp) / name
            download(url, staged)
            # Validate before publishing either cache file or executable.
            # A digest mismatch is fatal: retrying cannot make wrong bytes right.
            unpack_checked(staged, root / 'cargo-nextest', digest)
            staged.replace(archive)
    output = root / 'cargo-nextest'
    unpack_checked(archive, output, digest)
    return output
