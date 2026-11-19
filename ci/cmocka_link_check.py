#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Link the configured Tier 2 Rust fixtures and hybrid libobs on Linux.

No target program is executed. Reuses cmake/cross's soldr SDK/dependency setup
and the unchanged test/cmocka graph, including its deferred link support.
"""
from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import time
from pathlib import Path

import local_gate

ROOT = Path(__file__).resolve().parent.parent
TRIPLES = {"win-x64": "x86_64-pc-windows-msvc", "mac-arm64": "aarch64-apple-darwin"}


def discover_targets(reply: Path) -> list[str]:
    indexes = sorted(reply.glob("index-*.json"))
    if not indexes:
        raise FileNotFoundError(f"No CMake file-api reply in {reply}")
    index = json.loads(indexes[-1].read_text())
    model = json.loads((reply / index["reply"]["codemodel-v2"]["jsonFile"]).read_text())
    targets = [json.loads((reply / target["jsonFile"]).read_text())
               for target in model["configurations"][0]["targets"]]
    rust = sorted(t["name"] for t in targets if t["type"] == "EXECUTABLE" and t["name"].endswith("_rust"))
    if not rust or not any(t["name"] == "libobs-cross-shared" and t["type"] == "SHARED_LIBRARY" for t in targets):
        raise ValueError("Tier 2 cross-link inventory needs Rust executables and hybrid libobs")
    return ["libobs-cross-shared", *rust]


def windows_toolchain(work: Path, target: str) -> Path:
    """Use soldr's SDK drivers in clang-cl mode so CMake evaluates MSVC guards."""
    def query(option: str) -> str:
        return subprocess.run(["soldr", "cc", "--target", target, option], cwd=ROOT,
                              capture_output=True, text=True, check=True).stdout.strip()

    ar, rc, sdk = (query(flag) for flag in ("--print-ar", "--print-rc", "--print-sysroot"))
    llvm = Path(ar).resolve().parent
    compiler = llvm / "clang-cl"
    if not compiler.is_file():
        raise FileNotFoundError(f"soldr LLVM is missing {compiler}")
    tools = work / "soldr-tools"
    # soldr's GNU driver injects -isystem, which clang-cl does not accept.
    # Use the clang-cl sibling from the same managed LLVM and its xwin SDK.
    recipe = [str(compiler), f"--target={target}"]
    for directory in ("crt/include", "sdk/include/ucrt", "sdk/include/um", "sdk/include/shared",
                      "sdk/include/winrt", "sdk/include/cppwinrt"):
        recipe.extend(["-imsvc", str(Path(sdk) / directory)])
    for name in ("clang-cl", "clang-cl++"):
        path = tools / name
        path.write_text(f'#!/bin/sh\nexec {shlex.join(recipe)} "$@"\n')
        path.chmod(0o755)
    settings = {"CMAKE_SYSTEM_NAME": "Windows", "CMAKE_SYSTEM_PROCESSOR": "AMD64",
                "CMAKE_TRY_COMPILE_CONFIGURATION": "Release", "CMAKE_MSVC_RUNTIME_LIBRARY": "MultiThreadedDLL",
                "CMAKE_C_COMPILER": str(tools / "clang-cl"), "CMAKE_CXX_COMPILER": str(tools / "clang-cl++"),
                "CMAKE_AR": ar, "CMAKE_LINKER": str(llvm / "lld-link"), "CMAKE_RC_COMPILER": rc,
                "CMAKE_MT": str(llvm / "llvm-mt"), "CMAKE_TRY_COMPILE_TARGET_TYPE": "STATIC_LIBRARY",
                "CMAKE_FIND_ROOT_PATH": sdk, "CMAKE_FIND_ROOT_PATH_MODE_PROGRAM": "NEVER"}
    # CMake's MSVC link rule invokes lld-link directly, bypassing the compiler
    # driver's SDK config. Retain exactly that SDK's real import libraries.
    search = " ".join(f'/libpath:"{sdk}/{directory}/x86_64"'
                      for directory in ("crt/lib", "sdk/lib/um", "sdk/lib/ucrt"))
    for kind in ("EXE", "SHARED", "MODULE"):
        settings[f"CMAKE_{kind}_LINKER_FLAGS"] = search
    path = work / "windows-msvc.cmake"
    path.write_text("".join(f'set({key} [==[{value}]==] CACHE STRING "soldr Tier 2 toolchain" FORCE)\n'
                            for key, value in settings.items()))
    return path


def link(alias: str, work: Path, jobs: int) -> None:
    started = time.monotonic()
    target = TRIPLES[alias]
    local_gate.add_target(target)
    # Invoke Cargo even on a warm tree: the cross project's legacy custom
    # command only depends on manifests, which cannot detect a Rust source edit.
    local_gate.soldr_cargo(["build", "--release", "--locked", "--target", target, "-p", "libobs-rust"])
    work.mkdir(parents=True, exist_ok=True)
    wrappers = work / "soldr-tools"
    wrappers.mkdir(exist_ok=True)
    for name in ("cargo", "rustc"):
        wrapper = wrappers / name
        wrapper.write_text(f'#!/bin/sh\nexec soldr {name} "$@"\n')
        wrapper.chmod(0o755)
    toolchain_args = [f"-DCMAKE_TOOLCHAIN_FILE={windows_toolchain(work, target)}"] if alias == "win-x64" else []
    query = work / ".cmake/api/v1/query/codemodel-v2"
    query.parent.mkdir(parents=True, exist_ok=True)
    query.touch()
    archiver = subprocess.run(["soldr", "cc", "--target", target, "--print-ar"],
                              cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    env = local_gate.soldr_env({"OBS_CROSS_TARGET": alias, "SOLDR_PORCELAIN": "0",
                               f"AR_{target.replace('-', '_')}": archiver})
    subprocess.run(["cmake", "-S", str(ROOT), "-B", str(work), "-G", "Ninja",
                    f"-DOBS_CROSS_TARGET={alias}", "-DOBS_CROSS_LINK=ON", "-DOBS_CROSS_TIER2=ON",
                    "-DCMAKE_BUILD_TYPE=Release", f"-DRust_CARGO={wrappers / 'cargo'}",
                    f"-DRust_COMPILER={wrappers / 'rustc'}", f"-DRust_CARGO_TARGET={target}",
                    "-DOBS_TEST_SANITIZE_PER_TARGET=OFF", *toolchain_args], cwd=ROOT, env=env, check=True)
    targets = discover_targets(work / ".cmake/api/v1/reply")
    (work / "link-targets.json").write_text(json.dumps(targets, indent=2) + "\n")
    print(f"Tier 2 {alias}: linking hybrid libobs and {len(targets) - 1} Rust fixtures", flush=True)
    subprocess.run(["cmake", "--build", str(work), "--parallel", str(jobs), "--target", *targets,
                    "--", "-k", "0"], cwd=ROOT, env=env, check=True)
    print(f"Tier 2 {alias}: linked in {time.monotonic() - started:.2f}s", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--alias", choices=TRIPLES, default="win-x64")
    parser.add_argument("--build", type=Path)
    parser.add_argument("--jobs", type=int, default=min(4, os.cpu_count() or 1))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    link(args.alias, (args.build or ROOT / f"build_cmocka_link_{args.alias}").resolve(), args.jobs)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
