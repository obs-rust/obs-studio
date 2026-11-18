#!/usr/bin/env -S uv run --script
"""Prepare the Windows Rust executable slice on Linux; no workflow activation.

    uv run --no-project ci/windows_test_artifact.py build --output build_windows_tests
    uv run --no-project ci/windows_test_artifact.py verify --manifest build_windows_tests/manifest.json

This is a partial bundle: doctests, runtime native/Qt fixtures and the Windows
source-path handoff are follow-up slices. It cannot replace the Windows job.
The shared Darwin collector preserves Cargo's cwd, environment, helpers and
OUT_DIR; its checksummed transport is reused without changing Darwin policy.
"""
from __future__ import annotations

import argparse
import json
import struct
import subprocess
import sys
from pathlib import Path

import local_gate
import macos_test_artifact as shared
import windows_link_check
from windows_zip_check import VC_RUNTIME, imports

TARGET = windows_link_check.TARGET
ROOT = Path(__file__).resolve().parents[1]
COVERAGE = "rust-test-executables"
# Runtime libraries supplied by the supported Windows runner, not by xwin's
# import-library SDK. Record VC dependencies separately for later consumer setup.
SYSTEM_DLLS = frozenset({
    "advapi32.dll", "bcrypt.dll", "bcryptprimitives.dll", "cfgmgr32.dll",
    "comctl32.dll", "comdlg32.dll", "crypt32.dll", "dbghelp.dll", "gdi32.dll",
    "iphlpapi.dll", "kernel32.dll", "msvcrt.dll", "ncrypt.dll", "netapi32.dll",
    "ntdll.dll", "ole32.dll", "oleaut32.dll", "powrprof.dll", "propsys.dll",
    "psapi.dll", "rpcrt4.dll", "secur32.dll", "setupapi.dll", "shell32.dll",
    "shlwapi.dll", "ucrtbase.dll", "user32.dll", "userenv.dll", "version.dll",
    "winhttp.dll", "winmm.dll", "winspool.drv", "ws2_32.dll", "wtsapi32.dll",
})
VC_DLLS = frozenset({"vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll",
                     "msvcp140_1.dll", "msvcp140_2.dll", "concrt140.dll"})


def pe_imports(data: bytes) -> list[str]:
    """Require an AMD64 PE32+ console executable and known runtime dependencies.

    PE import parsing (including delay-loads) is shared with the Windows zip
    check. This is format/dependency evidence, not a Windows execution result.
    """
    try:
        if data[:2] != b"MZ":
            raise ValueError("missing DOS header")
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        machine, sections = struct.unpack_from("<HH", data, pe + 4)
        size, flags = struct.unpack_from("<HH", data, pe + 20)
        magic = struct.unpack_from("<H", data, pe + 24)[0]
        entry = struct.unpack_from("<I", data, pe + 40)[0]
        subsystem = struct.unpack_from("<H", data, pe + 92)[0]
        if (data[pe:pe + 4] != b"PE\0\0" or machine != 0x8664 or magic != 0x20B
                or not flags & 2 or flags & 0x2000 or not entry or subsystem != 3
                or size < 240 or not sections or pe + 24 + size + sections * 40 > len(data)):
            raise ValueError("not an x86_64 PE32+ console executable")
        executable_entry = False
        for index in range(sections):
            section = pe + 24 + size + index * 40
            virtual, raw_size, raw = struct.unpack_from("<III", data, section + 12)
            characteristics = struct.unpack_from("<I", data, section + 36)[0]
            if raw_size and (raw < pe + 24 + size + sections * 40 or raw + raw_size > len(data)):
                raise ValueError("truncated PE section body")
            if characteristics & 0x20000000 and virtual <= entry < virtual + raw_size:
                executable_entry = True
        if not executable_entry:
            raise ValueError("PE entry point has no executable section")
        dependencies = sorted(set(imports(data)))
        if not dependencies:
            raise ValueError("missing Windows runtime imports")
    except (struct.error, IndexError) as error:
        raise ValueError("truncated PE executable") from error
    for name in dependencies:
        if (name not in SYSTEM_DLLS and name not in VC_DLLS
                and not (name.startswith(("api-ms-win-", "ext-ms-win-")) and name.endswith(".dll")
                         and "/" not in name and "\\" not in name)):
            raise ValueError(f"unsupported Windows runtime dependency: {name}")
    return dependencies


def verify_pe(path: Path) -> None:
    pe_imports(path.read_bytes())


def collect(metadata: dict, events: list[dict], root: Path, bundle: Path, packages: list[str]) -> dict:
    if not events or events[-1] != {"reason": "build-finished", "success": True}:
        raise ValueError("Cargo did not report a complete successful build")
    has_tests = shared.check_test_inventory(metadata, events, packages)
    data = shared.collect(metadata, events, root, bundle, [], target=TARGET, verify_executable=verify_pe)
    images = {str(path.relative_to(bundle)): pe_imports(path.read_bytes())
              for path in sorted(bundle.rglob("*.exe"))}
    data.update(coverage=COVERAGE, scope_no_tests=not has_tests, images=images,
                runtime_dlls=sorted({dll for deps in images.values() for dll in deps if VC_RUNTIME.match(dll)}))
    shared.finalize(bundle, data)
    return data


def verify(manifest: Path) -> dict:
    data = json.loads(manifest.read_text())
    root = manifest.parent
    if (data.get("version") != 1 or data.get("target") != TARGET or data.get("coverage") != COVERAGE
            or "doctests_complete" in data or data.get("native_fixtures")):
        raise ValueError("not a Windows Rust executable slice")
    if not data["tests"] and not (data.get("scope_skipped") or data.get("scope_no_tests")):
        raise ValueError("empty test inventory")
    shared.validate_files(data, root)
    images = data["images"]
    if set(images) != {name for name in data["files"] if name.endswith(".exe")}:
        raise ValueError("PE image inventory mismatch")
    for entry in data["tests"]:
        if entry["binary"] not in images:
            raise ValueError("test executable missing from PE inventory")
    for name, dependencies in images.items():
        if pe_imports(shared.within(root, name).read_bytes()) != dependencies:
            raise ValueError("PE runtime dependency inventory mismatch")
    expected = sorted({dll for deps in images.values() for dll in deps if VC_RUNTIME.match(dll)})
    if data["runtime_dlls"] != expected:
        raise ValueError("VC runtime inventory mismatch")
    return data


def build_command(packages: list[str]) -> list[str]:
    arguments = windows_link_check.link_args()
    if packages != ["--workspace"]:
        arguments.remove("--workspace")
        arguments.extend(packages)
    return ["soldr", "cargo", *arguments, "--message-format=json"]


def build(output: Path, base: str | None) -> int:
    if output.exists() and any(output.iterdir()):
        raise ValueError("build output must be empty")
    env = local_gate.soldr_env({"SOLDR_PORCELAIN": "0", "SOLDR_VERDICTS": "off"})
    metadata = json.loads(subprocess.run(
        ["soldr", "cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
        cwd=ROOT, env=env, capture_output=True, text=True, check=True).stdout)
    files = None if base is None else subprocess.run(
        ["git", "diff", "--name-only", base, "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.splitlines()
    packages = shared.packages_for(files, metadata)
    if not packages:
        output.mkdir(parents=True, exist_ok=True)
        shared.finalize(output, {"version": 1, "target": TARGET, "coverage": COVERAGE,
                                "build_root": str(ROOT), "tests": [], "images": {},
                                "runtime_dlls": [], "scope_skipped": True})
    else:
        local_gate.add_target(TARGET)
        result = subprocess.run(build_command(packages), cwd=ROOT, env=env, capture_output=True, text=True)
        sys.stderr.write(result.stderr)
        result.check_returncode()
        events = [json.loads(line) for line in result.stdout.splitlines()]
        collect(metadata, events, ROOT, output, packages)
    data = verify(output / "manifest.json")
    print(f"Verified {len(data['tests'])} harnesses, {len(data['images'])} PE executables; VC runtime: {data['runtime_dlls']}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    producer = sub.add_parser("build")
    producer.add_argument("--output", type=Path, required=True)
    producer.add_argument("--base")
    verifier = sub.add_parser("verify")
    verifier.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "build":
        return build(args.output.resolve(), args.base)
    verify(args.manifest.resolve())
    return 0


if __name__ == "__main__":
    sys.exit(main())
