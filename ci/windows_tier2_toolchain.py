#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Provision the published soldr-toolchain Windows builtins as a private overlay.

The managed LLVM installation is never changed. Like fetch_libs_asset.py, pin
the archive SHA256 before extraction. This is the single-part catalogue unit
compiler-rt/21.1.5/windows-x64/bundle.tar.zst, source-d9139e614bd7-p5.
"""

from __future__ import annotations

import argparse
from compression import zstd
import hashlib
import io
from pathlib import Path
import subprocess
import tarfile
import urllib.request

ARCHIVE_SHA256 = "f5da2d73d77d6d454b02deeb99aa636a6759a4268b80805359a9cc4ef9cfca2f"
LIBRARY_SHA256 = "157aa147988b0f62eba653c91d7b9c71873e62711b1fc2af169dc4c0013b958d"
LIBRARY = "lib/clang/21/lib/windows/clang_rt.builtins-x86_64.lib"
URL = (
    "https://raw.githubusercontent.com/zackees/soldr-toolchain/public-b/sha256/"
    f"{ARCHIVE_SHA256}/0001-{ARCHIVE_SHA256}.part"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def install_builtins(output: Path, library: Path) -> None:
    if not library.exists():
        with urllib.request.urlopen(URL, timeout=120) as response:
            archive = response.read()
        if sha256(archive) != ARCHIVE_SHA256:
            raise ValueError("compiler-rt archive checksum mismatch")
        # The pinned unit has only builtins and its license. Read regular files
        # without extracting archive paths, links or permissions to the host.
        with tarfile.open(fileobj=io.BytesIO(zstd.decompress(archive))) as tar:
            entries = {
                entry.name.removeprefix("./"): entry for entry in tar.getmembers()
            }
            member = entries[f"package/{LIBRARY}"]
            if not member.isfile():
                raise ValueError("compiler-rt builtins is not a regular file")
            payload = tar.extractfile(member).read()
            licenses = {
                entry.name: tar.extractfile(entry).read()
                for entry in tar.getmembers()
                if entry.isfile() and "license" in entry.name.lower()
            }
        if sha256(payload) != LIBRARY_SHA256 or not licenses:
            raise ValueError("compiler-rt builtins checksum or license mismatch")
        library.parent.mkdir(parents=True, exist_ok=True)
        library.write_bytes(payload)
        for index, data in enumerate(licenses.values()):
            (output / f"compiler-rt-license-{index}.txt").write_bytes(data)
    if sha256(library.read_bytes()) != LIBRARY_SHA256:
        raise ValueError("compiler-rt builtins checksum mismatch")


def preserve_resources(managed: Path, resource: Path, library: Path) -> None:
    # Preserve all original resource entries; split lib/ so adding Windows
    # builtins cannot modify the shared managed tree through a directory link.
    for parent, destination in [
        (managed, resource),
        (managed / "lib", resource / "lib"),
        (managed / "lib/windows", resource / "lib/windows"),
    ]:
        if not parent.exists():
            continue
        for child in parent.iterdir():
            if (
                (parent == managed and child.name == "lib")
                or (parent == managed / "lib" and child.name == "windows")
                or child.name == library.name
            ):
                continue
            link = destination / child.name
            if not link.exists():
                link.symlink_to(child, target_is_directory=child.is_dir())


def provision(cc: str, output: Path) -> Path:
    managed = Path(
        subprocess.check_output([cc, "-print-resource-dir"], text=True).strip()
    )
    if not (managed / "include/xmmintrin.h").is_file():
        raise ValueError(f"missing managed clang resource headers: {managed}")
    output = output.resolve()
    resource = output / "resource"
    library = resource / "lib/windows/clang_rt.builtins-x86_64.lib"
    install_builtins(output, library)
    preserve_resources(managed, resource, library)
    config = output / "clang.cfg"
    # Clang expands config options BEFORE the driver's injected xwin -isystem
    # arguments. Ordinary trailing -isystem/-I flags cannot fix that order.
    text = f'-isystem\n"{managed / "include"}"\n-resource-dir\n"{resource}"\n--rtlib=compiler-rt\n'
    if not config.exists() or config.read_text() != text:
        config.write_text(text)
    return config


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cc", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    print(provision(args.cc, args.output))


if __name__ == "__main__":
    main()
