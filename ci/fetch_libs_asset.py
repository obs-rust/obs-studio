#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Fetch, verify and unpack one obs-rust/obs-studio-libs release asset (#992).

    uv run ci/fetch_libs_asset.py TAG ASSET SHA256 DEST

Skips everything when DEST/.asset-sha256 already holds SHA256 (a cache hit).
The archive is deleted once unpacked; pkg-config files that name the build-time
prefix /opt/obs-deps/<name> are rewritten to DEST so PKG_CONFIG_PATH works.

The zstd layer is decoded here with the standard library (compression.zstd,
Python 3.14) and plain tar is piped to `tar -xf -`. `tar --zstd` made macOS
bsdtar start an external zstd filter, which exited 1 on a valid, sha-verified
Qt SDK ("tar: Child process exited with status 1", run 41439029430).
"""

from __future__ import annotations

import hashlib
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

URL = "https://github.com/obs-rust/obs-studio-libs/releases/download/{tag}/{asset}"
STAMP = ".asset-sha256"


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def rewrite_pkgconfig(dest: Path) -> int:
    n = 0
    for pc in dest.rglob("*.pc"):
        text = pc.read_text()
        new = re.sub(r"/opt/obs-deps/[A-Za-z0-9_.+-]+", str(dest), text)
        if new != text:
            pc.write_text(new)
            n += 1
    return n


def extract_tar_zst(archive: Path, dest: Path) -> None:
    """Unpack ARCHIVE (.tar.zst) into DEST; no zstd program is ever started."""
    from compression import zstd

    argv = ["tar", "-xf", "-", "-C", str(dest)]
    proc = subprocess.Popen(argv, stdin=subprocess.PIPE)
    assert proc.stdin is not None
    try:
        with zstd.open(archive, "rb") as src, proc.stdin:
            shutil.copyfileobj(src, proc.stdin, 1 << 20)
    except (zstd.ZstdError, EOFError) as e:  # EOFError: truncated stream
        proc.kill()
        proc.wait()
        raise SystemExit(f"{archive.name}: zstd decode failed: {e}") from e
    except BrokenPipeError:
        pass  # tar quit early; its exit status below says why
    except BaseException:
        proc.kill()
        proc.wait()
        raise
    if rc := proc.wait():
        raise SystemExit(f"{archive.name}: tar exited {rc} unpacking the decoded stream ({' '.join(argv)})")


def fetch(tag: str, asset: str, sha: str, dest: Path) -> None:
    sha = sha.lower()
    if (dest / STAMP).is_file() and (dest / STAMP).read_text().strip() == sha:
        print(f"{asset}: cache hit ({dest})")
        return
    with tempfile.TemporaryDirectory() as tmp:
        archive = Path(tmp) / asset
        urllib.request.urlretrieve(URL.format(tag=tag, asset=asset), archive)
        got = sha256_of(archive)
        if got != sha:
            raise SystemExit(f"{asset}: sha256 {got} != pinned {sha}")
        dest.mkdir(parents=True, exist_ok=True)
        extract_tar_zst(archive, dest)
    print(f"{asset}: rewrote {rewrite_pkgconfig(dest)} pkg-config files")
    (dest / STAMP).write_text(sha + "\n")


def main() -> int:
    if len(sys.argv) != 5 or not re.fullmatch(r"[0-9a-fA-F]{64}", sys.argv[3]):
        print(__doc__, file=sys.stderr)
        return 2
    fetch(sys.argv[1], sys.argv[2], sys.argv[3], Path(sys.argv[4]).resolve())
    return 0


if __name__ == "__main__":
    sys.exit(main())
