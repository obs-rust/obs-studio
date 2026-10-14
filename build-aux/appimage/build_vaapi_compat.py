#!/usr/bin/env -S uv run --script
"""Build the pinned Ubuntu libavutil with an optional host VA mapping API.

Install the source package's architecture-dependent build dependencies first:
    apt-get build-dep --arch-only ffmpeg=7:8.0.1-3ubuntu2
Then run this script with a new work directory. The result is libavutil.so.60
in that directory. The Ubuntu recipe and every configured hardware backend are
preserved; only the mandatory dependency on vaMapBuffer2 becomes optional.
"""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess

HERE = Path(__file__).resolve().parent
VERSION = "7:8.0.1-3ubuntu2"
ARCHIVES = {
    "ffmpeg_8.0.1-3ubuntu2.dsc": "70ac30ebbb7e89fc4ebc4438457fab7c1deea5b62986577bfb1c866fee72436e",
    "ffmpeg_8.0.1.orig.tar.xz": "05ee0b03119b45c0bdb4df654b96802e909e0a752f72e4fe3794f487229e5a41",
    "ffmpeg_8.0.1.orig.tar.xz.asc": "54ab4020bb4eb280444da74d7aabc282acf6e8500cf6feaefd9821373fa9e72c",
    "ffmpeg_8.0.1-3ubuntu2.debian.tar.xz": "a32bd75677bca735105b04a0ebd190a3481e0d8c2fcacd7d9f17524e240ee879",
}


def run(command: list[str], **kwargs) -> None:
    print("+", " ".join(command), flush=True)
    subprocess.run(command, check=True, **kwargs)


def exported_abi(library: Path) -> dict[str, tuple[str, int | None]]:
    output = subprocess.check_output(
        ["readelf", "--dyn-syms", "--wide", str(library)], text=True
    )
    symbols = {}
    for line in output.splitlines():
        fields = line.split()
        if (
            len(fields) < 8
            or fields[4] not in ("GLOBAL", "WEAK")
            or fields[6] in ("UND", "ABS")
        ):
            continue
        if fields[3] in ("FUNC", "OBJECT", "TLS"):
            # Function bodies change; exported data layouts must not.
            symbols[fields[7]] = (
                fields[3],
                int(fields[2], 0) if fields[3] != "FUNC" else None,
            )
    return symbols


def audit_library(original: Path, rebuilt: Path, features: Path) -> None:
    old, new = exported_abi(original), exported_abi(rebuilt)
    if not old or old != new:
        raise RuntimeError(
            f"libavutil ABI changed: missing={sorted(old.keys() - new.keys())}, "
            f"added={sorted(new.keys() - old.keys())}, "
            f"layouts={[name for name in old.keys() & new.keys() if old[name] != new[name]]}"
        )
    before = subprocess.check_output([str(features), str(original)], text=True)
    after = subprocess.check_output([str(features), str(rebuilt)], text=True)
    if before != after:
        raise RuntimeError(f"libavutil version/backends changed:\n{before}\n{after}")
    undefined = subprocess.check_output(
        ["readelf", "--dyn-syms", "--wide", str(rebuilt)], text=True
    )
    if any("UND" in line and "vaMapBuffer2" in line for line in undefined.splitlines()):
        raise RuntimeError("libavutil still requires the newer host VA mapping API")
    print(
        f"Preserved {len(old)} exported ABI symbols and actual registered backends:\n{after}",
        flush=True,
    )


def build(work: Path, original: Path, archives: Path | None = None) -> Path:
    installed = subprocess.check_output(
        ["dpkg-query", "-W", "-f=${Version}", "libavutil60"], text=True
    ).strip()
    if installed != VERSION:
        raise RuntimeError(
            f"update the source pin for installed libavutil {installed!r}; expected {VERSION}"
        )
    work.mkdir(parents=True, exist_ok=False)
    if archives is None:
        archives = work / "archives"
        archives.mkdir()
        run(["apt-get", "source", "--download-only", f"ffmpeg={VERSION}"], cwd=archives)
    for filename, digest in ARCHIVES.items():
        actual = hashlib.sha256((archives / filename).read_bytes()).hexdigest()
        if actual != digest:
            raise RuntimeError(f"source checksum mismatch: {filename}")
    source = work / "ffmpeg"
    run(["dpkg-source", "-x", str(archives / "ffmpeg_8.0.1-3ubuntu2.dsc"), str(source)])
    shutil.copyfile(
        HERE / "vaapi-runtime-compat.h", source / "libavutil/vaapi-runtime-compat.h"
    )
    with (HERE / "ffmpeg-vaapi-runtime-compat.patch").open("rb") as patch:
        run(["patch", "-p1", "--fuzz=0"], cwd=source, stdin=patch)
    run(
        ["make", "-f", "debian/rules", "configure_standard"],
        cwd=source,
        env={**os.environ, "DEB_BUILD_PROFILES": "pkg.ffmpeg.noextra"},
    )
    run(
        [
            "make",
            "-C",
            str(source / "debian/standard"),
            "-j2",
            "libavutil/libavutil.so.60",
        ]
    )
    rebuilt = work / "libavutil.so.60"
    shutil.copyfile(source / "debian/standard/libavutil/libavutil.so.60", rebuilt)
    includes = subprocess.check_output(
        ["pkg-config", "--cflags", "libavutil"], text=True
    ).split()
    features = work / "avutil-features"
    run(["cc", *includes, str(HERE / "avutil-features.c"), "-ldl", "-o", str(features)])
    audit_library(original, rebuilt, features)
    return rebuilt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("work", type=Path)
    parser.add_argument(
        "--archives", type=Path, help="already downloaded pinned source archives"
    )
    parser.add_argument("--original", type=Path)
    args = parser.parse_args()
    original = args.original
    if original is None:
        directory = subprocess.check_output(
            ["pkg-config", "--variable=libdir", "libavutil"], text=True
        ).strip()
        original = Path(directory) / "libavutil.so.60"
    result = build(
        args.work.resolve(),
        original.resolve(),
        args.archives.resolve() if args.archives else None,
    )
    print(f"Validated compatible libavutil: {result}")


if __name__ == "__main__":
    main()
