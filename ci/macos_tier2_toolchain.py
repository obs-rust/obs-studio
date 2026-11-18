#!/usr/bin/env -S uv run --script
"""Provision the Linux-only Tier 2 producer from soldr's Darwin receipt.

OBS_TIER2_SOLDR is a local proof override for an unreleased soldr build. CI
continues to use its pinned soldr; no native compiler fallback is available.
"""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
from pathlib import Path

TARGET = "aarch64-apple-darwin"
RUNTIMES = (
    "libclang_rt.asan_osx_dynamic.dylib",
    "libclang_rt.ubsan_osx_dynamic.dylib",
    "libclang_rt.osx.a",
)
TOOLS = ("clang", "clangxx", "ar", "ranlib", "dsymutil", "swiftc", "cmake_toolchain_file")
DIRECTORIES = ("sdk", "compiler_rt", "clang_resource_dir", "swift_resource_dir")


def validate(receipt: dict) -> None:
    if (receipt.get("schema_version"), receipt.get("target"), receipt.get("sdk_version")) != (1, TARGET, "26.5"):
        raise ValueError("Tier 2 requires soldr Darwin schema 1, arm64 and SDK 26.5")
    if receipt.get("swift_version") != "6.3.2":
        raise ValueError("Tier 2 requires the managed Swift 6.3.2 closure")
    for key in TOOLS:
        if not Path(receipt[key]).is_file():
            raise ValueError(f"missing managed Darwin tool: {key}")
    for key in DIRECTORIES:
        if not Path(receipt[key]).is_dir():
            raise ValueError(f"missing managed Darwin directory: {key}")
    for name in RUNTIMES:
        if not (Path(receipt["compiler_rt"]) / name).is_file():
            raise ValueError(f"missing managed Darwin runtime: {name}")


def provision(output: Path, environ: dict | None = None) -> dict:
    selected = os.environ if environ is None else environ
    if sys.platform != "linux":
        raise ValueError("Tier 2 compilation is Linux-only")
    command = [selected.get("OBS_TIER2_SOLDR", "soldr"), "toolchain", "darwin",
               "--target", TARGET, "--sdk-version", "26.5", "--json"]
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    receipt = json.loads(result.stdout)
    validate(receipt)
    output.mkdir(parents=True, exist_ok=True)
    (output / "darwin.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return receipt


def wrapper(path: Path, executable: str, arguments: list[str]) -> str:
    path.write_text("#!/bin/sh\nexec " + shlex.join([executable, *arguments]) + ' "$@"\n')
    path.chmod(0o755)
    return str(path.absolute())


def environment(receipt: dict, output: Path) -> dict[str, str]:
    validate(receipt)
    output.mkdir(parents=True, exist_ok=True)
    flags = receipt["c_compile_args"] + receipt["c_link_args"]
    cc = wrapper(output / "darwin-clang", receipt["clang"], flags)
    cxx = wrapper(output / "darwin-clang++", receipt["clangxx"], flags)
    swift = wrapper(output / "darwin-swiftc", receipt["swiftc"],
                    receipt["swift_compile_args"] + receipt["swift_link_args"])
    return {
        "CC_aarch64_apple_darwin": cc,
        "CXX_aarch64_apple_darwin": cxx,
        "AR_aarch64_apple_darwin": receipt["ar"],
        "SWIFTC": swift,
        "SDKROOT": receipt["sdk"],
        "DSYMUTIL": receipt["dsymutil"],
        "CMAKE_GENERATOR": "Ninja",
        "SOLDR_RUSTC_WRAPPER": "none",
    }


def cmake_toolchain(receipt: dict, output: Path, env: dict) -> Path:
    path = output / "tier2-darwin.cmake"
    lines = [f'include({json.dumps(receipt["cmake_toolchain_file"])})']
    # The soldr CMake file sets the C/C++ targets. Objective-C must use the
    # identical targeted driver and compiler-rt resource directory too.
    for language, key in (("OBJC", "CC_aarch64_apple_darwin"),
                          ("OBJCXX", "CXX_aarch64_apple_darwin")):
        lines.append(f'set(CMAKE_{language}_COMPILER {json.dumps(env[key])})')
    lines.extend([
        'set(CMAKE_OSX_DEPLOYMENT_TARGET "13.0" CACHE STRING "Tier 2 minimum" FORCE)',
        'set(CMAKE_Swift_COMPILER_TARGET "arm64-apple-macosx13.0")',
        'set(CMAKE_TRY_COMPILE_TARGET_TYPE STATIC_LIBRARY)',
        f'set(CMAKE_FIND_ROOT_PATH {json.dumps(receipt["sdk"])})',
        'set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)',
        'add_link_options("$<$<LINK_LANGUAGE:OBJC,OBJCXX>:-fuse-ld=lld>")',
    ])
    for name, binary in (("NM", "llvm-nm"), ("STRIP", "llvm-strip")):
        tool = Path(receipt["clang"]).parent / binary
        lines.append(f'set(CMAKE_{name} {json.dumps(str(tool))})')
    path.write_text("\n".join(lines) + "\n")
    return path.absolute()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--configure", type=Path, help="configure this cross build root")
    args = parser.parse_args()
    receipt = provision(args.output)
    env = environment(receipt, args.output / "wrappers")
    toolchain = cmake_toolchain(receipt, args.output, env)
    (args.output / "environment.json").write_text(json.dumps(env, indent=2) + "\n")
    if args.configure:
        root = Path(__file__).resolve().parents[1]
        subprocess.run([
            "cmake", "-S", str(root), "-B", str(args.configure), "-G", "Ninja",
            "-DOBS_CROSS_TARGET=mac-arm64", "-DOBS_CROSS_TIER2=ON",
            f"-DCMAKE_TOOLCHAIN_FILE={toolchain}",
            f"-DOBS_TIER2_DARWIN_RECEIPT={args.output.absolute() / 'darwin.json'}",
        ], check=True, env={**os.environ, **env})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
