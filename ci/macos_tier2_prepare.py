#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Prepare the Cargo-native macOS contract on Linux (#5009, #5848).

`macos_core_builds_from_the_canonical_manifest` builds both libobs cores, the
capture modules, the OpenGL/Metal renderers and their probes. On Linux it runs
with OBS_BUILD_MACOS_PREPARE_ONLY=1 (host test binary, Darwin target, soldr's
LLVM and swiftc) and writes macos-prepared.json. The Darwin test binary is
cross-compiled here too; macOS only runs it with OBS_BUILD_MACOS_ASSERT_ONLY=1.
"""
from __future__ import annotations

import argparse
import json
import os
import shlex
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "aarch64-apple-darwin"
TEST = "native_libobs::macos_core_builds_from_the_canonical_manifest"
NATIVE_OUTPUT = "cargo-native-core"
CONTRACT_DIRECTORY = "cargo-native-tests"


def wrapper(path: Path, executable: str, arguments: list[str]) -> str:
    path.write_text("#!/bin/sh\nexec " + shlex.join([executable, *arguments]) + ' "$@"\n')
    path.chmod(0o755)
    return str(path)


def toolchain_environment(receipt: dict, directory: Path) -> dict[str, str]:
    """Compiler, archiver, swiftc and SDK for obs-build's Darwin cross build."""
    directory.mkdir(parents=True, exist_ok=True)
    flags = receipt["c_compile_args"] + receipt["c_link_args"]
    return {
        "CC_aarch64_apple_darwin": wrapper(directory / "cc", receipt["clang"], flags),
        "CXX_aarch64_apple_darwin": wrapper(directory / "c++", receipt["clangxx"], flags),
        "AR_aarch64_apple_darwin": receipt["ar"],
        "SWIFTC": wrapper(directory / "swiftc", receipt["swiftc"],
                          receipt["swift_compile_args"] + receipt["swift_link_args"]),
        "SDKROOT": receipt["sdk"],
        "OBS_BUILD_MACOS_NM": str(Path(receipt["clang"]).parent / "llvm-nm"),
        "SOLDR_PORCELAIN": "0",
    }


def contract_environment(root: Path, work: Path, dependencies: Path, *, assert_only: bool) -> dict[str, str]:
    """Environment shared by preparation (Linux) and assertions (macOS) of the contract."""
    modules = work / "plugins"
    bundles = {name: modules / name / f"{name}.plugin/Contents/MacOS/{name}"
               for name in ("mac-capture", "mac-avcapture", "mac-avcapture-legacy")}
    reference = work / "libobs-c-reference/libobs.framework/Versions/A/libobs"
    return {
        "OBS_BUILD_MACOS_ASSERT_ONLY" if assert_only else "OBS_BUILD_MACOS_PREPARE_ONLY": "1",
        "OBS_BUILD_MACOS_TARGET": TARGET,
        "OBS_BUILD_MACOS_OUTPUT": str(work / NATIVE_OUTPUT),
        "OBS_BUILD_MACOS_DEPENDENCIES": str(dependencies),
        "OBS_BUILD_MACOS_RUST_ARCHIVE": str(root / "target" / TARGET / "release/liblibobs_rust.a"),
        "OBS_BUILD_MACOS_REFERENCE": str(reference),
        "OBS_BUILD_MACOS_GRAPHICS": "1",
        "OBS_BUILD_MACOS_CAPTURE": "1",
        "OBS_BUILD_MACOS_OPENGL_REFERENCE": str(work / "libobs-opengl/libobs-opengl.0.dylib"),
        "OBS_BUILD_MACOS_METAL_REFERENCE": str(work / "libobs-metal/libobs-metal.dylib"),
        "OBS_BUILD_MACOS_MAC_CAPTURE_REFERENCE": str(bundles["mac-capture"]),
        "OBS_BUILD_MACOS_MAC_AVCAPTURE_REFERENCE": str(bundles["mac-avcapture"]),
        "OBS_BUILD_MACOS_MAC_AVCAPTURE_LEGACY_REFERENCE": str(bundles["mac-avcapture-legacy"]),
    }


def test_executable(root: Path, env: dict[str, str], *extra: str) -> Path:
    """Build the Application-category contract binary and return its path."""
    result = subprocess.run(
        ["soldr", "cargo", "test", "--no-run", "--locked", "-p", "obs-studio-rust", "--features",
         "libobs-integration", "--test", "obs-tests-app", "--message-format=json", *extra],
        cwd=root, env=env, capture_output=True, text=True, check=False)
    if result.returncode:
        raise RuntimeError(f"contract compilation failed: {result.stderr[-4000:]}")
    paths = []
    for line in result.stdout.splitlines():
        if line.startswith("{"):
            message = json.loads(line)
            if (message.get("reason") == "compiler-artifact" and message.get("executable")
                    and message.get("target", {}).get("name") == "obs-tests-app"):
                paths.append(Path(message["executable"]))
    if len(paths) != 1:
        raise ValueError(f"expected one obs-tests-app executable, found {len(paths)}")
    return paths[0]


def receipt() -> dict:
    return json.loads(subprocess.run(
        ["soldr", "toolchain", "darwin", "--target", TARGET, "--sdk-version", "26.5", "--json"],
        cwd=ROOT, capture_output=True, text=True, check=True).stdout)


def prepare(root: Path, work: Path) -> Path:
    """Run the Linux half; return the Darwin contract executable placed under the work tree."""
    if (work / NATIVE_OUTPUT).exists():
        shutil.rmtree(work / NATIVE_OUTPUT)
    darwin = receipt()
    env = {**os.environ, **toolchain_environment(darwin, work / "toolchain-wrappers")}
    dependencies = next((root / ".deps").glob("obs-deps-2*-universal"))
    subprocess.run(["soldr", "cargo", "build", "--locked", "--release", "--target", TARGET, "-p", "libobs-rust"],
                   cwd=root, env=env, check=True)
    host = test_executable(root, env)
    subprocess.run([str(host), TEST, "--exact", "--nocapture"], cwd=root, check=True,
                   env={**env, **contract_environment(root, work, dependencies, assert_only=False)})
    receipt_path = work / NATIVE_OUTPUT / "macos-prepared.json"
    if json.loads(receipt_path.read_text()).get("complete") is not True:
        raise ValueError(f"incomplete preparation receipt: {receipt_path}")
    darwin_contract = test_executable(root, env, "--target", TARGET)
    destination = work / CONTRACT_DIRECTORY / "obs-tests-app"
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(darwin_contract, destination)
    # The compile-only obs-sys consumer check cross-compiles on Linux as well.
    subprocess.run(["soldr", "cargo", "check", "--locked", "-p", "obs-sys", "--features", "bundled", "--target", TARGET],
                   cwd=root, check=True,
                   env={**env, "OBS_SYS_NATIVE_DEPENDENCIES": str(dependencies),
                        "OBS_BUILD_RUST_ARCHIVE": str(root / "target" / TARGET / "release/liblibobs_rust.a")})
    return destination


def assertions(root: Path, work: Path) -> None:
    """macOS half: execute the prebuilt Darwin contract in assertion-only mode."""
    contract = work / CONTRACT_DIRECTORY / "obs-tests-app"
    if not contract.is_file():
        raise FileNotFoundError(f"missing prebuilt contract executable: {contract}")
    dependencies = next((root / ".deps").glob("obs-deps-2*-universal"))
    env = {**os.environ, **contract_environment(root, work, dependencies, assert_only=True)}
    subprocess.run([str(contract), TEST, "--exact", "--nocapture"], cwd=root, env=env, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for name in ("prepare", "assert"):
        command = sub.add_parser(name)
        command.add_argument("--work", type=Path, required=True)
    args = parser.parse_args()
    if args.action == "prepare":
        print(prepare(ROOT, args.work.resolve()))
    else:
        assertions(ROOT, args.work.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
