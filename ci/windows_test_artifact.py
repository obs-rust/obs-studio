#!/usr/bin/env -S uv run --script
"""Prepare Windows Rust tests, doctests and native fixtures on Linux.

    uv run --no-project ci/windows_test_artifact.py build --output build_windows_tests
    uv run --no-project ci/windows_test_artifact.py verify --manifest build_windows_tests/manifest.json

Qt contracts are appended by windows_qt_artifact. windows_test_proof provides
optional hosted root/receipt qualification; the required Windows job is unchanged.
The shared Darwin collector preserves Cargo's cwd, environment, helpers and
OUT_DIR; its checksummed transport is reused without changing Darwin policy.
"""
from __future__ import annotations

import argparse
import json
import platform
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
PREPARED_COVERAGE = "rust-tests-doctests-native-fixtures"
# Runtime libraries supplied by the supported Windows runner, not by xwin's
# import-library SDK. Record VC dependencies separately for later consumer setup.
SYSTEM_DLLS = frozenset({
    "advapi32.dll", "authz.dll", "avrt.dll", "bcrypt.dll", "bcryptprimitives.dll", "cfgmgr32.dll",
    "comctl32.dll", "comdlg32.dll", "crypt32.dll", "d3d11.dll", "d3d12.dll", "dbghelp.dll",
    "dwmapi.dll", "dwrite.dll", "dxgi.dll", "gdi32.dll",
    # Qt's winsdkicu feature imports the Windows system ICU C API (Win10 1703+):
    # https://learn.microsoft.com/en-us/windows/win32/intl/international-components-for-unicode--icu-
    "icuuc.dll", "iphlpapi.dll", "kernel32.dll", "msvcrt.dll", "ncrypt.dll", "netapi32.dll",
    "mpr.dll", "ntdll.dll", "ole32.dll", "oleaut32.dll", "pdh.dll", "powrprof.dll", "propsys.dll",
    "psapi.dll", "rpcrt4.dll", "secur32.dll", "setupapi.dll", "shell32.dll",
    "shlwapi.dll", "ucrtbase.dll", "user32.dll", "userenv.dll", "uxtheme.dll", "version.dll",
    "winhttp.dll", "winmm.dll", "winspool.drv", "ws2_32.dll", "wtsapi32.dll",
})
VC_DLLS = frozenset({"vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll",
                     "msvcp140_1.dll", "msvcp140_2.dll", "concrt140.dll"})


def verify_pe_header(data: bytes, *, allow_dll: bool = False) -> None:
    """Validate AMD64 console headers, file-backed sections and entry point."""
    if data[:2] != b"MZ":
        raise ValueError("missing DOS header")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    machine, sections = struct.unpack_from("<HH", data, pe + 4)
    size, flags = struct.unpack_from("<HH", data, pe + 20)
    magic = struct.unpack_from("<H", data, pe + 24)[0]
    entry = struct.unpack_from("<I", data, pe + 40)[0]
    subsystem = struct.unpack_from("<H", data, pe + 92)[0]
    is_dll = bool(flags & 0x2000)
    if (data[pe:pe + 4] != b"PE\0\0" or machine != 0x8664 or magic != 0x20B
            or not flags & 2 or (is_dll and not allow_dll) or not entry
            or subsystem not in ((2, 3) if is_dll and allow_dll else (3,))
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


def pe_imports(data: bytes, *, allow_dll: bool = False, staged_dlls: set[str] = frozenset()) -> list[str]:
    """Require an AMD64 PE32+ console executable and known runtime dependencies.

    PE import parsing (including delay-loads) is shared with the Windows zip
    check. This is format/dependency evidence, not a Windows execution result.
    """
    try:
        verify_pe_header(data, allow_dll=allow_dll)
        dependencies = sorted(set(imports(data)))
        if not dependencies:
            raise ValueError("missing Windows runtime imports")
    except (struct.error, IndexError) as error:
        raise ValueError("truncated PE executable") from error
    for name in dependencies:
        runtime = VC_RUNTIME.match(name)
        if name == "ucrtbased.dll" or (runtime and runtime.group(3)):
            raise ValueError(f"unsupported debug Windows runtime dependency: {name}")
        if (name not in SYSTEM_DLLS and name not in VC_DLLS and name not in staged_dlls
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


def validate_coverage(data: dict, root: Path) -> None:
    if (data.get("version") != 1 or data.get("target") != TARGET
            or data.get("coverage") not in (COVERAGE, PREPARED_COVERAGE)):
        raise ValueError("not a Windows Rust executable slice")
    if data["coverage"] == COVERAGE and ("doctests_complete" in data or data.get("native_fixtures")):
        raise ValueError("partial coverage cannot include prepared claims")
    if data["coverage"] == PREPARED_COVERAGE:
        if data.get("doctests_complete") is not True:
            raise ValueError("incomplete Windows doctests")
        if any(e["package"] == "obs-studio-rust" for e in data["tests"]):
            if data.get("native_fixtures") is not True:
                raise ValueError("missing Windows native fixtures")
            from windows_native_fixtures import validate

            validate(root)


def verify(manifest: Path) -> dict:
    data = json.loads(manifest.read_text())
    root = manifest.parent
    validate_coverage(data, root)
    if not data["tests"] and not (data.get("scope_skipped") or data.get("scope_no_tests")):
        raise ValueError("empty test inventory")
    shared.validate_files(data, root)
    qt_images = data.get("qt_images", {})
    staged_dlls = set()
    if ("qt_contracts_complete" in data or qt_images
            or any(entry.get("kind") == "qt-contract" for entry in data["tests"])
            or any(name.startswith("target/windows-qt/") for name in data["files"])):
        from windows_qt_artifact import validate

        validate(data, root)
        staged_dlls = {Path(name).name.lower() for name in qt_images if name.endswith(".dll")}
    images = data["images"]
    if set(images) != {name for name in data["files"] if name.endswith(".exe")}:
        raise ValueError("PE image inventory mismatch")
    for entry in data["tests"]:
        if entry["binary"] not in images:
            raise ValueError("test executable missing from PE inventory")
    for name, dependencies in images.items():
        # Only Qt entries receive qt/bin in PATH. Category/helper executables
        # must not borrow a library merely because it exists elsewhere in payload.
        loader_dlls = staged_dlls if name.startswith("target/windows-qt/") else frozenset()
        if pe_imports(shared.within(root, name).read_bytes(), staged_dlls=loader_dlls) != dependencies:
            raise ValueError("PE runtime dependency inventory mismatch")
    expected = sorted({dll for deps in [*images.values(), *qt_images.values()] for dll in deps if VC_RUNTIME.match(dll)})
    if data["runtime_dlls"] != expected:
        raise ValueError("VC runtime inventory mismatch")
    return data


def build_command(packages: list[str]) -> list[str]:
    arguments = windows_link_check.link_args()
    if packages != ["--workspace"]:
        arguments.remove("--workspace")
        arguments.extend(packages)
    return ["soldr", "cargo", *arguments, "--message-format=json"]


def replay_doctests(manifest: Path, root: Path) -> int:
    """Replay captured harnesses through the shared runner on native Windows."""
    if platform.system() != "Windows":
        raise ValueError("doctest execution requires native Windows")
    verify(manifest)
    return shared.execute(manifest, root, target=TARGET, kinds={"doctest"},
                          validate=lambda data, directory: shared.validate_inventory(data, directory))


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
        data = collect(metadata, events, ROOT, output, packages)
        shared.compile_doctests(ROOT, output, env, packages, data, target=TARGET)
        if any(entry["package"] == "obs-studio-rust" for entry in data["tests"]):
            from windows_native_fixtures import prepare

            prepare(ROOT, output, env)
            data["native_fixtures"] = True
            for entry in data["tests"]:
                if entry["package"] == "obs-studio-rust":
                    entry["env"]["OBS_NATIVE_FIXTURES"] = "{root}/native-fixtures"
        data["coverage"] = PREPARED_COVERAGE
        data["images"] = {str(p.relative_to(output)): pe_imports(p.read_bytes()) for p in sorted(output.rglob("*.exe"))}
        data["runtime_dlls"] = sorted({dll for deps in data["images"].values() for dll in deps if VC_RUNTIME.match(dll)})
        shared.finalize(output, data)
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
    replay = sub.add_parser("run-doctests")
    replay.add_argument("--manifest", type=Path, required=True)
    replay.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    if args.command == "build":
        return build(args.output.resolve(), args.base)
    if args.command == "run-doctests":
        return replay_doctests(args.manifest.resolve(), args.root.resolve())
    verify(args.manifest.resolve())
    return 0


if __name__ == "__main__":
    sys.exit(main())
