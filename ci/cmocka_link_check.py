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
import re
import shlex
import subprocess
import time
import tempfile
from pathlib import Path

import local_gate
import macos_test_artifact
import tier2_front_cache

ROOT = Path(__file__).resolve().parent.parent
TRIPLES = {"win-x64": "x86_64-pc-windows-msvc", "mac-arm64": "aarch64-apple-darwin"}


def configured_targets(reply: Path) -> list[dict]:
    indexes = sorted(reply.glob("index-*.json"))
    if not indexes:
        raise FileNotFoundError(f"No CMake file-api reply in {reply}")
    index = json.loads(indexes[-1].read_text())
    model = json.loads((reply / index["reply"]["codemodel-v2"]["jsonFile"]).read_text())
    return [json.loads((reply / target["jsonFile"]).read_text())
            for target in model["configurations"][0]["targets"]]


def links_rust_archive(target: dict) -> bool:
    # Imported Corrosion archives do not have codemodel target entries.
    return any(Path(part["fragment"].strip('"')).name in ("libobs_rust.lib", "liblibobs_rust.a")
               for part in target.get("link", {}).get("commandFragments", []))


def validate_msvc_providers(reply: Path) -> None:
    for target in configured_targets(reply):
        if target["type"] != "EXECUTABLE" or not links_rust_archive(target):
            continue
        if any(Path(part["fragment"].strip('"')).name == "obs.lib"
               for part in target.get("link", {}).get("commandFragments", [])):
            raise ValueError(f"{target['name']}: standalone Rust/support must not mix with obs.dll's full import library")


def discover_targets(reply: Path) -> list[str]:
    targets = configured_targets(reply)
    by_id = {t.get("id", t["name"]): t for t in targets}

    def uses_rust(target: dict, visited: frozenset[str] = frozenset()) -> bool:
        name = target["name"]
        if name == "libobs_rust-static":
            return True
        if name in visited or target["type"] in ("SHARED_LIBRARY", "MODULE_LIBRARY"):
            return False
        return any(uses_rust(by_id[dep["id"]], visited | {name})
                   for dep in target.get("dependencies", []) if dep["id"] in by_id)

    rust = sorted(t["name"] for t in targets if t["type"] == "EXECUTABLE"
                  and (t["name"].endswith("_rust") or uses_rust(t) or links_rust_archive(t)))
    if not rust or not any(t["name"] == "libobs-cross-shared" and t["type"] == "SHARED_LIBRARY" for t in targets):
        raise ValueError("Tier 2 cross-link inventory needs Rust executables and hybrid libobs")
    return ["libobs-cross-shared", *rust]


# Mach-O filetypes that CTest loads or executes: MH_EXECUTE, MH_DYLIB, MH_BUNDLE.
LOADABLE_FILETYPES = (2, 6, 8)
# Aggregate every test executable joins (test/cmocka/CMakeLists.txt).
MAC_CLOSURE_TARGETS = ["cmocka-tests"]


def verify_signed_images(work: Path) -> int:
    """Check the ad-hoc signature of every linked arm64 executable and dylib."""
    count = 0
    for path in sorted(work.rglob("*")):
        if not path.is_file() or path.is_symlink() or path.stat().st_size < 32:
            continue
        with path.open("rb") as handle:
            header = handle.read(16)
        if header[:4] != b"\xcf\xfa\xed\xfe" or int.from_bytes(header[12:16], "little") not in LOADABLE_FILETYPES:
            continue
        macos_test_artifact.verify_macho(path, LOADABLE_FILETYPES)
        count += 1
    if not count:
        raise ValueError(f"no linked Mach-O image under {work}")
    print(f"Tier 2 mac-arm64: {count} signed arm64 images verified", flush=True)
    return count


def darwin_runtime_flags() -> dict[str, str]:
    """Link flags for libclang_rt.osx.a (soldr's managed Darwin compiler-rt).

    The stock SDK driver has no builtins for @available checks, which the
    VideoToolbox tests use (__isPlatformVersionAtLeast).
    """
    receipt = json.loads(subprocess.run(
        ["soldr", "toolchain", "darwin", "--target", TRIPLES["mac-arm64"], "--sdk-version", "26.5", "--json"],
        cwd=ROOT, capture_output=True, text=True, check=True).stdout)
    runtime = Path(receipt["compiler_rt"]) / "libclang_rt.osx.a"
    if not runtime.is_file():
        raise FileNotFoundError(f"soldr Darwin toolchain is missing {runtime}")
    return {f"CMAKE_{kind}_LINKER_FLAGS": str(runtime) for kind in ("EXE",)}


REGISTRY_SNAPSHOT = ROOT / "ci" / "macos_tier2_registry.txt"


def check_registry_parity(listing: str, snapshot: Path = REGISTRY_SNAPSHOT) -> None:
    """Every test the native macOS pass runs must be registered in the cross graph.

    The snapshot is master's native Tier 2 (macos-26) CTest name list; new
    tests are added to it when they land, never removed to pass.
    """
    registered = set(re.findall(r"Test +#\d+: (\S+)", listing))
    missing = sorted(set(snapshot.read_text().split()) - registered)
    if missing:
        raise ValueError(f"cross registry misses {len(missing)} native macOS tests: {', '.join(missing)}")


def record_registry(work: Path, env: dict[str, str]) -> None:
    """Record the CTest registry this closure was built for (parity evidence)."""
    names = subprocess.run(["ctest", "--test-dir", str(work), "-N"], env=env, check=True,
                           capture_output=True, text=True).stdout
    (work / "ctest-registry.txt").write_text(names)
    check_registry_parity(names)
    print(names.strip().splitlines()[-1], flush=True)


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


def cache_env(work: Path, tools: Path | None = None) -> dict[str, str]:
    """Fresh per-pass cache: share identical support compiles, never link results."""
    # Keep the launcher command stable for kept Ninja trees; only cached objects
    # and policy are ephemeral. The installer re-verifies its pinned binary.
    binary = tier2_front_cache.install(tools or work)
    config = work / "ccache.conf"
    config.write_text("")
    env = {key: value for key, value in local_gate.soldr_env().items() if not key.startswith("CCACHE_")}
    env.update(CCACHE_CONFIGPATH=str(config), CCACHE_DIR=str(work / "objects"),
               CCACHE_MAXSIZE="512M", CCACHE_COMPILERCHECK="content",
               CMAKE_C_COMPILER_LAUNCHER=str(binary), CMAKE_CXX_COMPILER_LAUNCHER=str(binary))
    return env


def build_targets(alias: str, work: Path, jobs: int, env: dict[str, str], all_tests: bool = False) -> None:
    reply = work / ".cmake/api/v1/reply"
    targets = discover_targets(reply)
    if all_tests:
        # Every cmocka executable, for the Wine run (ci/tier2_wine.py).
        targets = ["libobs-cross-shared", "cmocka-tests"]
    if alias == "win-x64":
        # LLD and native LINK resolve late archive members differently. Enforce
        # the component boundary even when LLD happens to accept an import mix.
        validate_msvc_providers(reply)
    if alias == "mac-arm64":
        targets = [*targets, *MAC_CLOSURE_TARGETS]
    (work / "link-targets.json").write_text(json.dumps(targets, indent=2) + "\n")
    print(f"Tier 2 {alias}: linking hybrid libobs and {len(targets) - 1} targets ({jobs} Ninja workers)", flush=True)
    # --all-tests tolerates executables the cross tree cannot build yet;
    # ci/tier2_wine.py fails unless the unbuilt set equals its NOT_CROSS_BUILT.
    subprocess.run(["cmake", "--build", str(work), "--parallel", str(jobs), "--target", *targets,
                    "--", "-k", "0"], cwd=ROOT, env=env, check=not all_tests)
    if alias == "mac-arm64":
        verify_signed_images(work)
        record_registry(work, env)


def link(alias: str, work: Path, jobs: int, all_tests: bool = False) -> None:
    with tempfile.TemporaryDirectory(prefix="cmocka-link-cache-") as directory:
        env = cache_env(Path(directory), work / "compiler-cache")
        try:
            _link(alias, work, jobs, env, all_tests)
        finally:
            subprocess.run([env["CMAKE_C_COMPILER_LAUNCHER"], "--show-stats"], env=env, check=False)


def _link(alias: str, work: Path, jobs: int, env: dict[str, str], all_tests: bool = False) -> None:
    started = time.monotonic()
    target = TRIPLES[alias]
    local_gate.add_target(target)
    # Invoke Cargo even on a warm tree: the cross project's legacy custom
    # command only depends on manifests, which cannot detect a Rust source edit.
    local_gate.soldr_cargo(["build", "--release", "--locked", "--target", target, "-p", "libobs-rust"])
    print(f"Tier 2 {alias}: Rust archive ready in {time.monotonic() - started:.2f}s", flush=True)
    work.mkdir(parents=True, exist_ok=True)
    wrappers = work / "soldr-tools"
    wrappers.mkdir(exist_ok=True)
    for name in ("cargo", "rustc"):
        wrapper = wrappers / name
        wrapper.write_text(f'#!/bin/sh\nexec soldr {name} "$@"\n')
        wrapper.chmod(0o755)
    toolchain_args = [f"-DCMAKE_TOOLCHAIN_FILE={windows_toolchain(work, target)}"] if alias == "win-x64" else []
    if alias == "mac-arm64":
        toolchain_args = [f"-D{key}={value}" for key, value in darwin_runtime_flags().items()]
    query = work / ".cmake/api/v1/query/codemodel-v2"
    query.parent.mkdir(parents=True, exist_ok=True)
    query.touch()
    archiver = subprocess.run(["soldr", "cc", "--target", target, "--print-ar"],
                              cwd=ROOT, capture_output=True, text=True, check=True).stdout.strip()
    env = dict(env, OBS_CROSS_TARGET=alias, SOLDR_PORCELAIN="0")
    env[f"AR_{target.replace('-', '_')}"] = archiver
    configured = time.monotonic()
    subprocess.run(["cmake", "-S", str(ROOT), "-B", str(work), "-G", "Ninja",
                    f"-DOBS_CROSS_TARGET={alias}", "-DOBS_CROSS_LINK=ON", "-DOBS_CROSS_TIER2=ON",
                    "-DCMAKE_BUILD_TYPE=RelWithDebInfo", f"-DRust_CARGO={wrappers / 'cargo'}",
                    f"-DRust_COMPILER={wrappers / 'rustc'}", f"-DRust_CARGO_TARGET={target}",
                    "-DOBS_TEST_SANITIZE_PER_TARGET=OFF",
                    f"-DCMAKE_C_COMPILER_LAUNCHER={env['CMAKE_C_COMPILER_LAUNCHER']}",
                    f"-DCMAKE_CXX_COMPILER_LAUNCHER={env['CMAKE_CXX_COMPILER_LAUNCHER']}", *toolchain_args], cwd=ROOT, env=env, check=True)
    print(f"Tier 2 {alias}: configured in {time.monotonic() - configured:.2f}s", flush=True)
    build_targets(alias, work, jobs, env, all_tests)
    print(f"Tier 2 {alias}: linked in {time.monotonic() - started:.2f}s", flush=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--alias", choices=TRIPLES, default="win-x64")
    parser.add_argument("--build", type=Path)
    parser.add_argument("--all-tests", action="store_true", help="build every cmocka executable, not only the Rust fixtures")
    parser.add_argument("--jobs", type=int, default=min(4, os.process_cpu_count() or 1))
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    link(args.alias, (args.build or ROOT / f"build_cmocka_link_{args.alias}").resolve(), args.jobs, args.all_tests)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
