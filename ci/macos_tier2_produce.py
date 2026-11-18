#!/usr/bin/env -S uv run --script
"""Build every macOS Tier 2 input on Linux, then record exact-source evidence.

Run after macos_test_workspace.py place. OBS_TIER2_SOLDR may select the
maintainer's source-built soldr until the Darwin provisioning API is released.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess

import host_platform
import macos_tier2_bundle as bundle
import macos_tier2_packet as packet
import macos_tier2_registry as registry
import macos_tier2_toolchain as toolchain


def executable(root: Path, env: dict, target: str) -> Path:
    result = subprocess.run([
        "soldr", "cargo", "test", "--no-run", "--locked", "--target", target,
        "-p", "obs-studio-rust", "--features", "libobs-integration", "--test", "obs-tests-app",
        "--message-format=json",
    ], cwd=root, env=env, capture_output=True, text=True, check=False)
    if result.returncode:
        raise RuntimeError(f"contract compilation failed: {result.stderr[-6000:]}")
    artifacts = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
    paths = [Path(item["executable"]) for item in artifacts
             if item.get("reason") == "compiler-artifact" and item.get("executable")
             and item.get("target", {}).get("name") == "obs-tests-app"]
    if len(paths) != 1:
        raise ValueError("one Application-category contract executable required")
    return paths[0]


def produce(root: Path = packet.COMMON) -> dict:
    if not host_platform.is_linux() or root != packet.COMMON or not root.is_dir() or root.resolve() != root:
        raise ValueError("production requires Linux and the common real checkout root")
    sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=root):
        raise ValueError("production requires a clean source commit")
    work, build, native = root / ".tier2", root / packet.BUILD, root / packet.NATIVE
    work.mkdir(exist_ok=True)
    # Invalidate old certification *before* the first build, including retries.
    evidence = work / "production.json"
    evidence.write_text(json.dumps({"complete": False, "source_sha": sha}) + "\n")
    receipt = toolchain.provision(work / "toolchain")
    env = {**os.environ, **toolchain.environment(receipt, work / "toolchain/wrappers")}
    prefix = f"{Path.home()}=/toolchain"
    for key, compiler in (("CC_aarch64_apple_darwin", "clang"), ("CXX_aarch64_apple_darwin", "clangxx")):
        toolchain.wrapper(Path(env[key]), receipt[compiler], receipt["c_compile_args"] + receipt["c_link_args"]
                          + [f"-ffile-prefix-map={prefix}", f"-fdebug-prefix-map={prefix}"])
    toolchain.wrapper(Path(env["SWIFTC"]), receipt["swiftc"], receipt["swift_compile_args"]
                      + receipt["swift_link_args"] + ["-debug-prefix-map", prefix])
    env["CARGO_TARGET_AARCH64_APPLE_DARWIN_RUSTFLAGS"] = f"--remap-path-prefix={Path.home()}=/toolchain"
    cmake_toolchain = toolchain.cmake_toolchain(receipt, work / "toolchain", env)
    with cmake_toolchain.open("a") as output:
        for language in ("C", "CXX", "OBJC", "OBJCXX"):
            flags = f" -ffile-prefix-map={prefix} -fdebug-prefix-map={prefix}"
            output.write(f"string(APPEND CMAKE_{language}_FLAGS_INIT {json.dumps(flags)})\n")
        output.write(f"string(APPEND CMAKE_Swift_FLAGS_INIT {json.dumps(' -debug-prefix-map ' + prefix)})\n")
    subprocess.run([
        "cmake", "-S", str(root), "-B", str(build), "-G", "Ninja",
        "-DOBS_CROSS_TARGET=mac-arm64", "-DOBS_CROSS_TIER2=ON",
        f"-DCMAKE_TOOLCHAIN_FILE={cmake_toolchain}",
        f"-DOBS_TIER2_DARWIN_RECEIPT={work / 'toolchain/darwin.json'}",
    ], cwd=root, env=env, check=True)
    subprocess.run(["cmake", "--build", str(build), "--target", "tier2-cross", "--", "-j", "4"], cwd=root, env=env, check=True)
    host = subprocess.check_output(["soldr", "rustc", "-vV"], cwd=root, env=env, text=True)
    triple = next(line.removeprefix("host: ") for line in host.splitlines() if line.startswith("host: "))
    host_contract = executable(root, env, triple)
    archive = root / "target/aarch64-apple-darwin/release/liblibobs_rust.a"
    subprocess.run(["soldr", "cargo", "build", "--locked", "--release", "--target", toolchain.TARGET, "-p", "libobs-rust"], cwd=root, env=env, check=True)
    native_env = {**env, "OBS_BUILD_MACOS_PREPARE_ONLY": "1", "OBS_BUILD_MACOS_TARGET": toolchain.TARGET,
                  "OBS_BUILD_MACOS_OUTPUT": str(native), "OBS_BUILD_MACOS_DEPENDENCIES": str(root / ".deps/obs-deps-2026-10-25-universal"),
                  "OBS_BUILD_MACOS_MODULES": str(build / "tier2-modules.txt"), "OBS_BUILD_MACOS_RUST_ARCHIVE": str(archive),
                  "OBS_BUILD_MACOS_REFERENCE": str(build / "cores/reference/libobs.framework/Versions/A/libobs"),
                  "OBS_BUILD_MACOS_GRAPHICS": "1", "OBS_BUILD_MACOS_CAPTURE": "1", "OBS_BUILD_MACOS_PCH": "1"}
    subprocess.run([str(host_contract), "native_libobs::macos_core_builds_from_the_canonical_manifest", "--exact", "--nocapture"],
                   cwd=root, env=native_env, check=True)
    contract = executable(root, env, toolchain.TARGET)
    prepared = packet.prepared_artifacts(native)
    paths = bundle.selected_runtime(build, [Path(line) for line in (build / "tier2-runtime.txt").read_text().splitlines()])
    paths.extend([Path(path) for names in prepared["artifacts"].values() for path in names if Path(path).is_file()])
    paths.append(contract)
    # Same-commit configure provenance plus clean source are independently checked.
    if registry.snapshot(build, root)["revision"] != sha:
        raise ValueError("source commit changed during Linux production")
    proof = {"schema": 1, "complete": True, "source_sha": sha,
             "contract": contract.relative_to(root).as_posix(),
             "files": {path.relative_to(root).as_posix(): bundle.digest(path) for path in paths}}
    evidence.write_text(json.dumps(proof, indent=2) + "\n")
    return proof


if __name__ == "__main__":
    print(json.dumps(produce(), indent=2))
