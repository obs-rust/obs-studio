#!/usr/bin/env -S uv run --script
"""Stage/run the opt-in Tier 2 prototype, without compiling on the run host.

The verifier is supplied by #5008's macos_test_artifact.py; this module reuses
its CodeDirectory page verifier, including for dylibs and loadable bundles.
Source roots must match because unchanged C fixtures embed source paths.
This prototype does not attest that the native graphics/capture contract exists.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import struct
import subprocess
from collections.abc import Callable
from pathlib import Path

TOKEN = "@OBS_TIER2_BUILD@"


def contained(root: Path, path: Path) -> Path:
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"path escapes artifact root: {path}")
    return path.relative_to(root)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def arm64_slice(data: bytes) -> bytes:
    """Extract a universal image's arm64 slice without changing signed bytes."""
    if data[:4] not in (b"\xca\xfe\xba\xbe", b"\xca\xfe\xba\xbf"):
        return data
    count = struct.unpack_from(">I", data, 4)[0]
    wide = data[:4] == b"\xca\xfe\xba\xbf"
    stride, format_ = (32, ">IIQQII") if wide else (20, ">IIIII")
    for index in range(count):
        cpu, _, offset, size, *_ = struct.unpack_from(format_, data, 8 + index * stride)
        if cpu == 0x100000C:
            if offset < 8 + count * stride or offset + size > len(data):
                raise ValueError("invalid universal arm64 slice")
            return data[offset : offset + size]
    raise ValueError("universal image lacks arm64")


def signature(data: bytes) -> tuple[bytes, int]:
    magic, cpu, _, kind, count, size, _, _ = struct.unpack_from("<8I", data)
    if magic != 0xFEEDFACF or cpu != 0x100000C or kind not in (2, 6, 8):
        raise ValueError("expected thin arm64 executable, dylib or bundle")
    offset = 32
    for _ in range(count):
        command, length = struct.unpack_from("<2I", data, offset)
        if length < 8 or offset + length > 32 + size:
            raise ValueError("invalid Mach-O load command")
        if command == 0x1D:
            start, length = struct.unpack_from("<2I", data, offset + 8)
            return data[start : start + length], start
        offset += length
    raise ValueError("unsigned arm64 image")


def verifier(helper: Path) -> Callable[[Path], None]:
    spec = importlib.util.spec_from_file_location("macos_test_artifact", helper)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot load #5008 verifier: {helper}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)

    def verify(path: Path) -> None:
        data = arm64_slice(path.read_bytes())
        blob, start = signature(data)
        magic, length, count = struct.unpack_from(">3I", blob)
        if (
            magic != 0xFADE0CC0
            or length > len(blob)
            or len(blob) - length > 15
            or any(blob[length:])
        ):
            raise ValueError(f"invalid signature superblob: {path}")
        for index in range(count):
            slot, offset = struct.unpack_from(">2I", blob, 12 + index * 8)
            if slot == 0:
                module.verify_directory(path, blob[offset:], data, start)
                return
        raise ValueError(f"missing CodeDirectory: {path}")

    return verify


def stage(
    build: Path,
    source: Path,
    output: Path,
    artifacts: list[Path],
    verify: Callable[[Path], None],
    dependencies: list[Path] | None = None,
) -> dict:
    build, source, output = build.absolute(), source.absolute(), output.absolute()
    templates = {}
    for registry in build.rglob("CTestTestfile.cmake"):
        relative = contained(build, registry)
        templates[str(relative)] = registry.read_text().replace(str(build), TOKEN)
    if not templates or not artifacts:
        raise ValueError("missing CTest registry or runtime inventory")
    images = {str(Path("build") / contained(build, path)): path for path in artifacts}
    for path in dependencies or []:
        images[str(Path("deps") / path.name)] = path
    for path in images.values():
        if not path.is_file():
            raise FileNotFoundError(path)
        verify(path)
    if output.exists():
        raise ValueError(f"refusing to overwrite bundle: {output}")
    files = {}
    for relative, path in images.items():
        destination = output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(arm64_slice(path.read_bytes()))
        shutil.copymode(path, destination)
        files[str(relative)] = digest(destination)
    manifest = {
        "schema": 1,
        "target": "aarch64-apple-darwin",
        "source_root": str(source),
        "native_contract_prebuilt": False,
        "files": files,
        "ctest_templates": templates,
    }
    output.mkdir(parents=True, exist_ok=True)
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def prepare(bundle: Path, source: Path) -> dict:
    manifest = json.loads((bundle / "manifest.json").read_text())
    if manifest["schema"] != 1 or manifest["target"] != "aarch64-apple-darwin":
        raise ValueError("unsupported Tier 2 artifact manifest")
    if source.absolute() != Path(manifest["source_root"]):
        raise ValueError("unchanged fixtures require the same absolute source root")
    for relative, expected in manifest["files"].items():
        path = bundle / relative
        contained(bundle, path)
        if digest(path) != expected:
            raise ValueError(f"changed artifact: {relative}")
    for relative, template in manifest["ctest_templates"].items():
        path = bundle / "build" / relative
        contained(bundle / "build", path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(template.replace(TOKEN, str(bundle.absolute() / "build")))
    return manifest


def run(bundle: Path, source: Path, mode: str, jobs: int) -> int:
    if mode not in ("shared", "reference") or jobs < 1:
        raise ValueError("select shared/reference and positive CTest parallelism")
    manifest = prepare(bundle, source)
    if not manifest["native_contract_prebuilt"]:
        raise ValueError(
            "incomplete prototype: native contract and runtime compiler fixtures must be split first"
        )
    environment = dict(os.environ)
    environment["DYLD_FRAMEWORK_PATH"] = str(bundle.absolute() / "build/cores" / mode)
    environment["DYLD_LIBRARY_PATH"] = str(bundle.absolute() / "deps")
    return subprocess.run(
        [
            "ctest",
            "--test-dir",
            str(bundle / "build"),
            "-C",
            "Release",
            "--output-on-failure",
            "-j",
            str(jobs),
            "--no-tests=error",
        ],
        env=environment,
        check=False,
    ).returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    collect = subparsers.add_parser("stage")
    collect.add_argument("--build", required=True, type=Path)
    collect.add_argument("--source", required=True, type=Path)
    collect.add_argument("--output", required=True, type=Path)
    collect.add_argument("--verifier", required=True, type=Path)
    collect.add_argument(
        "--deps", type=Path, help="directory containing signed dependency dylibs"
    )
    execute = subparsers.add_parser("run")
    execute.add_argument("--bundle", required=True, type=Path)
    execute.add_argument("--source", required=True, type=Path)
    execute.add_argument("--mode", required=True, choices=["shared", "reference"])
    execute.add_argument("--jobs", type=int, default=3)
    args = parser.parse_args()
    if args.command == "run":
        return run(args.bundle, args.source, args.mode, args.jobs)
    artifacts = [
        Path(line)
        for line in (args.build / "tier2-runtime.txt").read_text().splitlines()
    ]
    dependencies = list(args.deps.glob("*.dylib")) if args.deps else []
    stage(
        args.build,
        args.source,
        args.output,
        artifacts,
        verifier(args.verifier),
        dependencies,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
