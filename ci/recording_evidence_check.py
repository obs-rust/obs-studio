#!/usr/bin/env -S uv run --script
"""Check that a recording evidence run used a hardware renderer and the expected libobs (#1305).

    recording_evidence_check.py --log <OBS log> --maps <maps> --libobs {c,rust}
                                [--prefix <install prefix>] [--json <out>]

<maps> is a copy of /proc/<pid>/maps taken while OBS runs (on macOS, the
output of `vmmap <pid>` or `lsof -p <pid>`: every line that ends in an
absolute path counts). The run fails when the log shows a software
renderer, when the mapped libobs is not the expected backend, or when the
graphics module is missing or comes from outside --prefix. The Rust twin
is told apart from the C libobs by the Rust code linked into it, as in
release-rust.yaml (#1253). Every check fails closed: a missing line, an
unreadable file or an ambiguous map is a failure. On success it prints
the run identity (renderer, library paths and SHA-256) as JSON.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path


class CheckError(Exception):
    pass


ADAPTER = re.compile(r"Loading up (OpenGL|D3D11|Metal) on adapter (.+?)\s*$")
GL_VERSION = re.compile(r"OpenGL loaded successfully, version (.+?)\s*$")
SOFTWARE = re.compile(
    r"llvmpipe|softpipe|swrast|lavapipe|software rasterizer|"
    r"Microsoft Basic Render Driver|Apple Software Renderer",
    re.IGNORECASE,
)
MAPPED_PATH = re.compile(r"\s(/.*?)\s*$")
LIBOBS = re.compile(r"^(libobs\.so(\.\d+)*|libobs(\.\d+)*\.dylib|obs\.dll)$")
GRAPHICS = re.compile(r"^libobs-(opengl|d3d11|metal)(\.so(\.\d+)*|\.dylib|\.dll)?$")
RUST_MARKER = b"/rustc/"


def renderer(log: str) -> dict:
    adapters = []
    version = None
    for line in log.splitlines():
        if m := ADAPTER.search(line):
            if (m[1], m[2]) not in adapters:
                adapters.append((m[1], m[2]))
        elif (m := GL_VERSION.search(line)) and version is None:
            version = m[1]
    if not adapters:
        raise CheckError("no renderer line ('Loading up ... on adapter') in the log")
    if len(adapters) > 1:
        names = "; ".join(f"{api} {name}" for api, name in adapters)
        raise CheckError(f"more than one renderer in the log: {names}")
    api, adapter = adapters[0]
    for text in (adapter, version or ""):
        if SOFTWARE.search(text):
            raise CheckError(f"software renderer: {api} {adapter} ({version or 'no version line'})")
    return {"api": api, "adapter": adapter, "version": version}


def mapped_paths(maps: str) -> list[str]:
    paths = []
    for line in maps.splitlines():
        if (m := MAPPED_PATH.search(line)) and m[1] not in paths:
            paths.append(m[1])
    return paths


def is_libobs(path: Path) -> bool:
    if path.name == "libobs":
        return path.parent.parent.parent.name == "libobs.framework"
    return bool(LIBOBS.match(path.name))


def has_rust(data: bytes) -> bool:
    return RUST_MARKER in data


def identify(path: str) -> dict:
    if path.endswith(" (deleted)"):
        raise CheckError(f"{path}: the mapped file was deleted or replaced while OBS ran")
    try:
        data = Path(path).read_bytes()
    except OSError as e:
        raise CheckError(f"cannot read mapped file {path}: {e.strerror}") from e
    return {"path": path, "sha256": hashlib.sha256(data).hexdigest(), "rust": has_rust(data)}


def one(paths: list[str], what: str) -> str:
    if len(paths) != 1:
        raise CheckError(f"{len(paths)} {what} mapped, expected exactly 1: {paths}")
    return paths[0]


def libraries(maps: str, *, expect: str, prefix: Path | None = None) -> dict:
    paths = mapped_paths(maps)
    stripped = [p.removesuffix(" (deleted)") for p in paths]
    libobs = identify(one([p for p, s in zip(paths, stripped) if is_libobs(Path(s))], "libobs"))
    graphics = identify(
        one([p for p, s in zip(paths, stripped) if GRAPHICS.match(Path(s).name)], "graphics modules")
    )
    backend = "rust" if libobs.pop("rust") else "c"
    graphics.pop("rust")
    libobs["backend"] = backend
    if backend != expect:
        raise CheckError(f"expected the {expect} libobs, mapped {backend}: {libobs['path']}")
    if prefix is not None:
        root = prefix.resolve()
        for lib in (libobs, graphics):
            if not Path(lib["path"]).resolve().is_relative_to(root):
                raise CheckError(f"{lib['path']} is outside the install prefix {prefix}")
    return {"libobs": libobs, "graphics": graphics}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--log", type=Path, required=True, help="OBS log of the run")
    ap.add_argument("--maps", type=Path, required=True, help="copy of /proc/<pid>/maps")
    ap.add_argument("--libobs", choices=("c", "rust"), required=True, help="expected backend")
    ap.add_argument("--prefix", type=Path, help="install prefix every OBS library must come from")
    ap.add_argument("--json", type=Path, help="also write the identity to this file")
    args = ap.parse_args(argv)
    try:
        identity = {
            "renderer": renderer(args.log.read_text(errors="replace")),
            **libraries(args.maps.read_text(), expect=args.libobs, prefix=args.prefix),
        }
    except (CheckError, OSError) as e:
        print(f"::error::{e}", file=sys.stderr)
        return 1
    text = json.dumps(identity, indent=2)
    print(text)
    if args.json:
        args.json.write_text(text + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
