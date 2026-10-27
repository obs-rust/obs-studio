#!/usr/bin/env -S uv run --script
"""Install the pinned zccache release (static musl build) into a directory (#962):

    uv run --no-project rust/tools/linux-validate/install_zccache.py [DEST_DIR]

DEST_DIR defaults to ~/.local/bin. It is the compiler launcher of every Linux
CMake build here: Tier 2 (the Dockerfile next to this script), the Ubuntu
build, the AppImage and the sanitizers. soldr 0.9.38 embeds zccache 1.15.5 for
rustc; this standalone launcher gets the 1.15.5 PCH header-mtime replay fix
independently of soldr.

The release archive's SHA-256 is checked before the binary is installed, and
the version is pinned; a DEST_DIR/zccache that already reports it is left
alone. An unsupported machine is an error, and a download or digest mismatch
fails without installing anything.

Ported off the 38-line shell script in #962; the two Dockerfiles,
setup_zccache, build-project.yaml and sanitize.sh run this file.
"""

from __future__ import annotations

import hashlib
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

ZCCACHE_VERSION = "1.15.5"
RELEASE_URL = "https://github.com/zackees/zccache/releases/download"
# `uname -m` names of the Linux runner arches, mapped to the release arch.
MACHINE_ARCH = {
    "x86_64": "x86_64",
    "amd64": "x86_64",
    "aarch64": "aarch64",
    "arm64": "aarch64",
}
# The pinned SHA-256 of each static-musl release archive, one per arch.
SHA256 = {
    "x86_64": "39906c92b0287e2aac51ae9169b368d36eabc3af363c3c7bd29dc3783975e579",
    "aarch64": "a266bf8319d008708da870f49843bfc3a3ce2bb078be2c1448358584701b215a",
}
# `curl -fsSL --retry 3` tries the download once and then three more times.
DOWNLOAD_ATTEMPTS = 4


def arch_for(machine: str) -> str:
    """The release arch of a `uname -m` machine name; fail on an unknown one."""
    try:
        return MACHINE_ARCH[machine]
    except KeyError:
        raise SystemExit(f"install_zccache.py: unsupported arch {machine}") from None


def sha256_of(path: Path) -> str:
    """The SHA-256 of a file, as `sha256sum` prints it."""
    digest = hashlib.sha256()
    with open(path, "rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def installed(dest: Path) -> bool:
    """True when dest/zccache is executable and reports exactly the pinned version."""
    binary = dest / "zccache"
    if not (binary.is_file() and os.access(binary, os.X_OK)):
        return False
    result = subprocess.run([str(binary), "--version"], capture_output=True, text=True)
    return f"zccache {ZCCACHE_VERSION}" in result.stdout.splitlines()


def download(url: str, path: Path) -> None:
    """Fetch url into path, retrying a transient failure."""
    last: Exception | None = None
    for _ in range(DOWNLOAD_ATTEMPTS):
        try:
            with urllib.request.urlopen(url) as response, open(path, "wb") as out:
                shutil.copyfileobj(response, out)
            return
        except OSError as error:
            last = error
    raise SystemExit(f"install_zccache.py: could not download {url}: {last}")


def install(dest: Path, arch: str, work: Path) -> None:
    """Download the pinned release for arch, check its digest and install it."""
    name = f"zccache-v{ZCCACHE_VERSION}-{arch}-unknown-linux-musl"
    archive = work / f"{name}.tar.gz"
    download(f"{RELEASE_URL}/{ZCCACHE_VERSION}/{name}.tar.gz", archive)
    digest = sha256_of(archive)
    if digest != SHA256[arch]:
        print(f"install_zccache.py: {archive.name}: FAILED", file=sys.stderr)
        print(f"expected {SHA256[arch]}", file=sys.stderr)
        print(f"     got {digest}", file=sys.stderr)
        raise SystemExit(1)
    with tarfile.open(archive) as tar:
        tar.extractall(work, filter="data")
    dest.mkdir(parents=True, exist_ok=True)
    target = dest / "zccache"
    shutil.copyfile(work / name / "zccache", target)
    target.chmod(0o755)


def main(argv: list[str]) -> int:
    dest = Path(argv[1]) if len(argv) > 1 else Path.home() / ".local" / "bin"
    if installed(dest):
        return 0
    arch = arch_for(platform.machine())
    with tempfile.TemporaryDirectory() as tmp:
        install(dest, arch, Path(tmp))
    result = subprocess.run([str(dest / "zccache"), "--version"], capture_output=True, text=True)
    sys.stdout.write(result.stdout)
    return result.returncode


if __name__ == "__main__":
    sys.exit(main(sys.argv))
