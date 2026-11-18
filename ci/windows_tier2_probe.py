#!/usr/bin/env -S uv run --script
"""Stage/verify a Linux-built Windows Tier 2 probe; never compile on Windows.

No run subcommand: this four-test probe is explicitly incomplete. The CTest
root tokens and manifest hashes follow the Darwin Tier 2 prototype design.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess

from windows_zip_check import imports

TARGET = "x86_64-pc-windows-msvc"
BUILD = "@OBS_TIER2_BUILD@"
SOURCE = "@OBS_TIER2_SOURCE@"
CMAKE = "@OBS_TIER2_CMAKE@"
# Exact Windows system imports only; the VC runtime must be transported.
SYSTEM = frozenset(
    """
advapi32.dll avrt.dll bcrypt.dll cfgmgr32.dll comctl32.dll comdlg32.dll
crypt32.dll dbghelp.dll dnsapi.dll dwmapi.dll dxgi.dll gdi32.dll imm32.dll
iphlpapi.dll kernel32.dll kernelbase.dll mpr.dll msimg32.dll ncrypt.dll
netapi32.dll normaliz.dll ntdll.dll ole32.dll oleaut32.dll opengl32.dll
powrprof.dll propsys.dll psapi.dll rpcrt4.dll secur32.dll setupapi.dll
shell32.dll shlwapi.dll user32.dll userenv.dll uxtheme.dll version.dll
winhttp.dll winmm.dll wintrust.dll ws2_32.dll wtsapi32.dll
""".split()
)


def contained(root: Path, path: Path) -> Path:
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"path escapes artifact root: {path}")
    return path.relative_to(root)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def image(data: bytes) -> list[str]:
    """Require a backed AMD64 PE32+ image before examining its imports."""
    try:
        if data[:2] != b"MZ":
            raise ValueError("expected AMD64 PE image")
        offset = struct.unpack_from("<I", data, 0x3C)[0]
        machine, count = struct.unpack_from("<HH", data, offset + 4)
        size = struct.unpack_from("<H", data, offset + 20)[0]
        magic = struct.unpack_from("<H", data, offset + 24)[0]
        if (
            data[offset : offset + 4] != b"PE\0\0"
            or machine != 0x8664
            or magic != 0x20B
            or size < 240
            or count == 0
        ):
            raise ValueError("expected AMD64 PE32+ image")
        sections = offset + 24 + size
        if sections + count * 40 > len(data):
            raise ValueError("truncated PE section table")
        for index in range(count):
            raw_size, raw = struct.unpack_from("<II", data, sections + index * 40 + 16)
            if raw + raw_size > len(data):
                raise ValueError("truncated PE section")
        return imports(data)
    except (struct.error, IndexError) as error:
        raise ValueError("truncated AMD64 PE image") from error


def exports(path: Path, readobj: str) -> set[str]:
    output = subprocess.check_output([readobj, "--coff-exports", str(path)], text=True)
    names = set(re.findall(r"^  Name: (\S+)$", output, re.M))
    if not names:
        raise ValueError(f"empty exports: {path}")
    return names


def parity(reference: set[str], hybrid: set[str]) -> list[str]:
    if not reference or reference != hybrid:
        raise ValueError(
            f"export parity: missing={sorted(reference - hybrid)}, extra={sorted(hybrid - reference)}"
        )
    return sorted(reference)


def closure(artifacts: list[Path], roots: list[Path]) -> list[Path]:
    index = {}
    for root in roots:
        for path in sorted(root.rglob("*")):
            if path.is_file() and path.suffix.lower() == ".dll":
                key = path.name.lower()
                if key in index and digest(path) != digest(index[key]):
                    raise ValueError(f"ambiguous dependency DLL: {key}")
                index[key] = path
    known = {path.name.lower() for path in artifacts}
    pending, result = list(artifacts), []
    while pending:
        path = pending.pop()
        result.append(path)
        for name in image(path.read_bytes()):
            if (
                name in known
                or name in SYSTEM
                or name.startswith(("api-ms-win-", "ext-ms-win-"))
            ):
                continue
            if name not in index:
                raise ValueError(
                    f"unresolved dependency DLL: {name} imported by {path}"
                )
            known.add(name)
            pending.append(index[name])
    return sorted(set(result))


def stage(
    build: Path,
    output: Path,
    source: Path,
    artifacts: list[Path],
    roots: list[Path],
    readobj: str,
) -> dict:
    build, output, source = build.resolve(), output.absolute(), source.resolve()
    cores = [path for path in artifacts if path.name.lower() == "obs.dll"]
    if len(cores) != 2:
        raise ValueError("probe inventory requires both obs.dll cores")
    symbols = parity(*(exports(path, readobj) for path in cores))
    paths = closure(artifacts, roots)
    cache = build / "CMakeCache.txt"
    match = (
        re.search(r"^CMAKE_COMMAND:INTERNAL=(.+)$", cache.read_text(), re.M)
        if cache.exists()
        else None
    )
    producer_cmake = match[1] if match else shutil.which("cmake")
    templates = {}
    for registry in build.rglob("CTestTestfile.cmake"):
        template = (
            registry.read_text().replace(str(build), BUILD).replace(str(source), SOURCE)
        )
        if producer_cmake:
            template = template.replace(producer_cmake, CMAKE)
        templates[str(contained(build, registry))] = template
    if not artifacts or not templates:
        raise ValueError("missing runtime inventory or CTest metadata")
    if output.exists():
        raise ValueError(f"refusing to overwrite bundle: {output}")
    files, embedded = {}, []
    for path in paths:
        relative = (
            Path("build") / contained(build, path)
            if path.is_relative_to(build)
            else Path("deps") / path.name
        )
        destination = output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
        image(destination.read_bytes())
        files[relative.as_posix()] = digest(destination)
        if (
            str(build).encode() in destination.read_bytes()
            or str(source).encode() in destination.read_bytes()
        ):
            embedded.append(relative.as_posix())
    manifest = {
        "schema": 1,
        "target": TARGET,
        "complete": False,
        "coverage": "probe-only",
        "source_root": str(source),
        "build_root": str(build),
        "embedded_paths": embedded,
        "exports": symbols,
        "files": files,
        "ctest_templates": templates,
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def prepare(bundle: Path, source: Path | None = None) -> dict:
    manifest = json.loads((bundle / "manifest.json").read_text())
    if manifest["schema"] != 1 or manifest["target"] != TARGET:
        raise ValueError("unsupported Windows Tier 2 manifest")
    for relative, expected in manifest["files"].items():
        path = bundle / relative
        contained(bundle, path)
        if digest(path) != expected:
            raise ValueError(f"changed artifact: {relative}")
        image(path.read_bytes())
    for relative, template in manifest["ctest_templates"].items():
        path = bundle / "build" / relative
        contained(bundle / "build", path)
        cmake = shutil.which("cmake")
        if CMAKE in template and not cmake:
            raise ValueError("consumer requires CMake/CTest")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(
            template.replace(BUILD, (bundle.absolute() / "build").as_posix())
            .replace(
                SOURCE, (source or Path(manifest["source_root"])).absolute().as_posix()
            )
            .replace(CMAKE, Path(cmake).as_posix() if cmake else "cmake")
        )
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    collect = sub.add_parser("stage")
    collect.add_argument("--build", type=Path, required=True)
    collect.add_argument(
        "--source", type=Path, default=Path(__file__).resolve().parents[1]
    )
    collect.add_argument("--output", type=Path, required=True)
    collect.add_argument("--dll-root", type=Path, action="append", default=[])
    collect.add_argument("--readobj", required=True)
    verify = sub.add_parser("verify")
    verify.add_argument("--bundle", type=Path, required=True)
    verify.add_argument("--source", type=Path)
    args = parser.parse_args()
    if args.command == "verify":
        manifest = prepare(args.bundle, args.source)
    else:
        artifacts = [
            Path(line)
            for line in (args.build / "tier2-runtime.txt").read_text().splitlines()
        ]
        if len([path for path in artifacts if path.name.lower() == "obs.dll"]) != 2:
            raise ValueError("probe inventory requires both obs.dll cores")
        manifest = stage(
            args.build, args.output, args.source, artifacts, args.dll_root, args.readobj
        )
    print(
        json.dumps(
            {key: manifest[key] for key in ("coverage", "complete", "embedded_paths")},
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
