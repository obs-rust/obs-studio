#!/usr/bin/env -S uv run --script
"""Stage/verify a Linux-built Windows Tier 2 probe; never compile on Windows.

No run subcommand: this four-test probe is explicitly incomplete. The CTest
root tokens and manifest hashes follow the Darwin Tier 2 prototype design.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
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
# Exact Windows system imports. VC runtime is a separate explicit contract.
SYSTEM = frozenset(
    """
advapi32.dll authz.dll avicap32.dll avrt.dll bcrypt.dll bcryptprimitives.dll cfgmgr32.dll comctl32.dll comdlg32.dll
coremessaging.dll crypt32.dll d3d9.dll d3d11.dll d3d12.dll d3dcompiler_47.dll dbghelp.dll dnsapi.dll dwmapi.dll dwrite.dll dxgi.dll gdi32.dll icuuc.dll imm32.dll
iphlpapi.dll kernel32.dll kernelbase.dll mpr.dll msimg32.dll ncrypt.dll
netapi32.dll normaliz.dll ntdll.dll ole32.dll oleaut32.dll opengl32.dll
powrprof.dll propsys.dll psapi.dll rpcrt4.dll secur32.dll setupapi.dll
shell32.dll shlwapi.dll user32.dll userenv.dll uxtheme.dll version.dll
winhttp.dll winmm.dll wintrust.dll wldap32.dll ws2_32.dll wtsapi32.dll
""".split()
)
HOSTED_VC = frozenset(
    "vcruntime140.dll vcruntime140_1.dll msvcp140.dll msvcp140_1.dll "
    "msvcp140_2.dll msvcp140_atomic_wait.dll msvcp140_codecvt_ids.dll concrt140.dll".split()
)
RUNNER_DOC = "https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-VS2026-Readme.md#microsoft-visual-c"


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


def core_symbols(artifacts: list[Path], readobj: str) -> list[str]:
    cores = [path for path in artifacts if path.name.lower() == "obs.dll"]
    variants = {path.parent.name: path for path in cores}
    if len(cores) != 2 or set(variants) != {"reference", "shared"}:
        raise ValueError(
            "probe inventory requires distinct shared/reference obs.dll cores"
        )
    return parity(
        exports(variants["reference"], readobj), exports(variants["shared"], readobj)
    )


def dependency_index(roots: list[Path], known: set[str]) -> dict[str, Path]:
    index = {}
    for root in roots:
        for path in sorted(root.rglob("*")):
            if path.is_file() and path.suffix.lower() == ".dll":
                key = path.name.lower()
                if key in known:
                    continue
                if key in index and digest(path) != digest(index[key]):
                    raise ValueError(f"ambiguous dependency DLL: {key}")
                index[key] = path
    return index


def closure(
    artifacts: list[Path], roots: list[Path], *, hosted_runtime: bool = False
) -> list[Path]:
    known = {path.name.lower() for path in artifacts}
    index = dependency_index(roots, known)
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
            if name not in index and hosted_runtime and name in HOSTED_VC:
                continue
            if name not in index:
                raise ValueError(
                    f"unresolved dependency DLL: {name} imported by {path}"
                )
            known.add(name)
            pending.append(index[name])
    return sorted(set(result))


def runtime_requirements(paths: list[Path]) -> list[str]:
    present = {path.name.lower() for path in paths}
    return sorted(
        {name for path in paths for name in image(path.read_bytes())}
        & HOSTED_VC - present
    )


def verify_runtime(manifest: dict, root: Path) -> dict:
    """Require actual AMD64 installed release DLLs and their recursive imports."""
    names = manifest.get("hosted_runtime", {}).get("dlls", [])
    if any(name not in HOSTED_VC for name in names):
        raise ValueError("unsupported hosted VC runtime requirement")
    pending, verified = list(names), {}
    while pending:
        name = pending.pop()
        if name in verified:
            continue
        path = root / name
        for dependency in image(path.read_bytes()):
            if dependency in SYSTEM or dependency.startswith(
                ("api-ms-win-", "ext-ms-win-")
            ):
                continue
            if dependency not in HOSTED_VC:
                raise ValueError(
                    f"unexpected installed runtime dependency: {dependency}"
                )
            pending.append(dependency)
        verified[name] = digest(path)
    return verified


def stage(
    build: Path,
    output: Path,
    source: Path,
    artifacts: list[Path],
    roots: list[Path],
    readobj: str,
    *,
    hosted_runtime: bool = False,
) -> dict:
    build, output, source = build.resolve(), output.absolute(), source.resolve()
    core_symbols(artifacts, readobj)
    paths = closure(artifacts, roots, hosted_runtime=hosted_runtime)
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
    files, embedded, staged = {}, [], []
    for path in paths:
        relative = (
            Path("build") / contained(build, path)
            if path.is_relative_to(build)
            else Path("deps") / path.name
        )
        destination = output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
        staged.append(destination)
        image(destination.read_bytes())
        files[relative.as_posix()] = digest(destination)
        if (
            str(build).encode() in destination.read_bytes()
            or str(source).encode() in destination.read_bytes()
        ):
            embedded.append(relative.as_posix())
    # Imports/exports can change during a concurrent rebuild. Validate the bytes
    # we will transport, including newly introduced transitive dependencies.
    closure(staged, [output / "build", output / "deps"], hosted_runtime=hosted_runtime)
    symbols = core_symbols(staged, readobj)
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
        "hosted_runtime": {
            "provider": "windows-2025-vs2026" if hosted_runtime else None,
            "documentation": RUNNER_DOC if hosted_runtime else None,
            "dlls": runtime_requirements(staged),
        },
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def check_runtime_contract(
    manifest: dict, paths: list[Path], runtime_root: Path | None
) -> None:
    runtime = manifest.get("hosted_runtime", {})
    if runtime.get("dlls", []) != runtime_requirements(paths):
        raise ValueError("hosted VC runtime declaration does not match imports")
    if runtime.get("dlls") and runtime.get("provider") != "windows-2025-vs2026":
        raise ValueError("unsupported hosted VC runtime provider")
    closure(paths, [], hosted_runtime=bool(runtime.get("dlls")))
    if runtime.get("dlls"):
        if runtime_root is None and os.name == "nt":
            runtime_root = Path(os.environ["SystemRoot"]) / "System32"
        if runtime_root is not None:
            manifest["verified_hosted_runtime"] = verify_runtime(manifest, runtime_root)
        elif os.name != "nt":
            # Linux can validate transport, but cannot attest runner DLL bytes.
            manifest["verified_hosted_runtime"] = None


def prepare(
    bundle: Path, source: Path | None = None, runtime_root: Path | None = None
) -> dict:
    manifest = json.loads((bundle / "manifest.json").read_text())
    if manifest["schema"] != 1 or manifest["target"] != TARGET:
        raise ValueError("unsupported Windows Tier 2 manifest")
    paths = []
    for relative, expected in manifest["files"].items():
        path = bundle / relative
        contained(bundle, path)
        if digest(path) != expected:
            raise ValueError(f"changed artifact: {relative}")
        image(path.read_bytes())
        paths.append(path)
    check_runtime_contract(manifest, paths, runtime_root)
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
    collect.add_argument("--hosted-vc-runtime", action="store_true")
    verify = sub.add_parser("verify")
    verify.add_argument("--bundle", type=Path, required=True)
    verify.add_argument("--source", type=Path)
    verify.add_argument("--runtime-root", type=Path)
    args = parser.parse_args()
    if args.command == "verify":
        manifest = prepare(args.bundle, args.source, args.runtime_root)
    else:
        artifacts = [
            Path(line)
            for line in (args.build / "tier2-runtime.txt").read_text().splitlines()
        ]
        if len([path for path in artifacts if path.name.lower() == "obs.dll"]) != 2:
            raise ValueError("probe inventory requires both obs.dll cores")
        manifest = stage(
            args.build,
            args.output,
            args.source,
            artifacts,
            args.dll_root,
            args.readobj,
            hosted_runtime=args.hosted_vc_runtime,
        )
    print(
        json.dumps(
            {
                key: manifest.get(key)
                for key in (
                    "coverage",
                    "complete",
                    "embedded_paths",
                    "hosted_runtime",
                    "verified_hosted_runtime",
                )
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
