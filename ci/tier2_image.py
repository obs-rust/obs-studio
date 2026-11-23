#!/usr/bin/env -S uv run --script
"""Content identity, public GHCR selection and master-only Tier 2 image publication.

The package must be PUBLIC: anonymous probes and Docker pulls need no credentials,
including on forks. Missing/private manifests and probe errors select stock Ubuntu.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
CONTEXT = Path('rust/tools/linux-validate')
DOCKERFILE = CONTEXT / 'tier2-image/Dockerfile'
BOOTSTRAP = CONTEXT / 'tier2-image/install-git-packages.txt'
INPUTS = (DOCKERFILE, CONTEXT / 'apt-packages.txt', BOOTSTRAP)
REPOSITORY = 'ghcr.io/obs-rust/tier2-linux'
DIGEST = re.compile(r'sha256:[0-9a-f]{64}')
ACCEPT = ', '.join(('application/vnd.oci.image.manifest.v1+json',
                    'application/vnd.oci.image.index.v1+json',
                    'application/vnd.docker.distribution.manifest.v2+json',
                    'application/vnd.docker.distribution.manifest.list.v2+json'))


def base_image(root: Path) -> str:
    match = re.search(r'^FROM (ubuntu:26\.04@sha256:[0-9a-f]{64})$',
                      (root / DOCKERFILE).read_text(), re.M)
    if match is None:
        raise ValueError('Tier 2 Dockerfile must pin the stock Ubuntu base digest')
    return match[1]


def identity(root: Path) -> str:
    # JSON framing includes names and exact bytes, preventing ambiguous boundaries.
    inputs = {'base': base_image(root),
              'files': {path.as_posix(): (root / path).read_bytes().hex() for path in INPUTS}}
    digest = hashlib.sha256(json.dumps(inputs, sort_keys=True, separators=(',', ':')).encode()).hexdigest()
    return REPOSITORY + ':' + digest


def manifest_digest(tag: str, *, token: str | None = None) -> str | None:
    """Bounded anonymous Registry v2 probe. Only a manifest 404 means absent.

    Other errors propagate: readers fail open; writers must not overwrite a tag
    whose existence they could not establish. The public read uses no job token.
    """
    repository, reference = tag.removeprefix('ghcr.io/').split(':', 1)
    query = urllib.parse.urlencode({'service': 'ghcr.io', 'scope': f'repository:{repository}:pull'})
    headers = {}
    if token:
        credentials = (os.environ['GITHUB_ACTOR'] + ':' + token).encode()
        headers['Authorization'] = 'Basic ' + base64.b64encode(credentials).decode()
    request = urllib.request.Request('https://ghcr.io/token?' + query, headers=headers)
    with urllib.request.urlopen(request, timeout=5) as response:
        token = json.loads(response.read(65536))['token']
    request = urllib.request.Request(f'https://ghcr.io/v2/{repository}/manifests/{reference}',
                                    headers={'Authorization': 'Bearer ' + token, 'Accept': ACCEPT})
    try:
        with urllib.request.urlopen(request, timeout=5) as response:
            body = response.read(1048577)
            digest = response.headers.get('Docker-Content-Digest', '')
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise
    if len(body) > 1048576 or not DIGEST.fullmatch(digest) or \
            digest != 'sha256:' + hashlib.sha256(body).hexdigest():
        raise ValueError('GHCR manifest digest mismatch or oversized response')
    return digest


def select_image(labels: str, root: Path) -> str:
    stock = base_image(root)
    if not labels:
        return stock
    try:
        digest = manifest_digest(identity(root))
        if isinstance(digest, str) and DIGEST.fullmatch(digest):
            return REPOSITORY + '@' + digest
    except Exception as error:  # Any registry/protocol failure must retain stock.
        print(f'Tier 2 image probe unavailable ({type(error).__name__}); using stock Ubuntu', flush=True)
    return stock


def publish(root: Path) -> None:
    # Defense in depth: never grant PRs, merge groups, tags or forks a writer.
    if (os.environ.get('GITHUB_EVENT_NAME'), os.environ.get('GITHUB_REF'),
            os.environ.get('GITHUB_REPOSITORY')) != ('push', 'refs/heads/master', 'obs-rust/obs-studio'):
        raise ValueError('Tier 2 image publication requires an obs-rust/obs-studio master push')
    tag = identity(root)
    if manifest_digest(tag, token=os.environ.get('GITHUB_TOKEN')):
        print(f'{tag} already exists; nothing to build')
        return
    subprocess.run(['docker', 'login', 'ghcr.io', '--username', os.environ['GITHUB_ACTOR'],
                    '--password-stdin'], input=os.environ['GITHUB_TOKEN'], text=True, check=True)
    subprocess.run(['docker', 'build', '--platform', 'linux/amd64', '--tag', tag,
                    '--file', str(root / DOCKERFILE), str(root / CONTEXT)], check=True)
    subprocess.run(['docker', 'push', tag], check=True)
    print(f'Published {tag}')


def verify(root: Path) -> None:
    """Fail when the published image is not anonymously readable.

    The probe inside select_image() falls back to stock Ubuntu on any error, so
    a private package degrades silently: every Linux Tier 2 job re-downloads
    its 339 MB of .debs and nobody learns why. Publishing is the one moment we
    can tell the difference, so assert the documented invariant here instead of
    leaving it to whoever remembers to flip the setting.
    """
    tag = identity(root)
    try:
        digest = manifest_digest(tag)
    except urllib.error.HTTPError as error:
        raise SystemExit(
            f'{tag} is not anonymously readable (HTTP {error.code}): the package is '
            f'private. Anonymous probes and Docker pulls need no credentials, so make '
            f'https://github.com/orgs/obs-rust/packages/container/tier2-linux/settings '
            f'public; until then every Tier 2 job silently rebuilds from stock Ubuntu.')
    if digest is None:
        raise SystemExit(f'{tag} is missing: the publish step should have created it')
    print(f'{tag} is publicly readable ({digest})')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('identity', 'select', 'publish', 'verify'))
    args = parser.parse_args()
    if args.command == 'publish':
        publish(ROOT)
    elif args.command == 'verify':
        verify(ROOT)
    elif args.command == 'identity':
        print(identity(ROOT))
    else:
        image = select_image(os.environ.get('MQ_RUNNER_LABELS', ''), ROOT)
        output = f'image={image}\nprebaked={str(image != base_image(ROOT)).lower()}\n'
        print(output, end='')
        if os.environ.get('GITHUB_OUTPUT'):
            with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
                stream.write(output)


if __name__ == '__main__':
    main()
