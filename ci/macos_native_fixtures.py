#!/usr/bin/env -S uv run --script
"""Prepare obs-build's runtime fixtures on Linux for Darwin (#5008)."""
from __future__ import annotations

import json
import hashlib
import platform
import shlex
import shutil
import struct
import subprocess
import tarfile
import urllib.request
from pathlib import Path

TARGET = "aarch64-apple-darwin"
SWIFT_RELEASE = "swift-6.1.2-RELEASE-ubuntu22.04"
SWIFT_URL = f"https://download.swift.org/swift-6.1.2-release/ubuntu2204/swift-6.1.2-RELEASE/{SWIFT_RELEASE}.tar.gz"
SWIFT_SHA256 = "ed40901b5bccf9022ff5d41425e2316f8e710a7e0c99717ba62080744e81fd66"
# All default-enabled Darwin runtime compilation contracts, including CMake's
# independent consumer and Swift. Qt/libobs feature contracts are another slice.
TESTS = (
    "native_profile_switch_rebuilds_assertion_policy",
    "swift_bridge_preserves_native_linkage_header_flags_and_source_rebuilds",
    "native_plugin_links_shared_core_and_observes_rebuilt_dependency",
    "unity_toggle_preserves_exports_and_explicit_generated_source_wins",
    "executable_runs_against_its_shared_core_dependency",
    "pch_preserves_mixed_languages_unity_and_header_flag_invalidation",
    "explicit_groups_compile_as_individual_sources_and_observe_member_edits",
    "mixed_c_and_cpp_use_independent_flags_and_the_cpp_linker",
    "mixed_objective_languages_compile_load_and_rebuild_with_unity_and_pch",
    "arc_and_manual_groups_keep_source_flags_through_expansion_and_pch",
    "native_source_options_reject_ambiguous_or_orphan_assignments",
    "native_test_graph_conditional_prefixes_match_consumers",
    "native_test_graph_prepends_runtime_paths_in_order",
    "native_test_graph_preserves_include_precedence",
    "native_test_graph_preserves_cpp_standard_without_changing_c_compilation",
    "native_test_graph_cmake_builds_and_runs_selected_contract",
    "canonical_websocket_helpers_is_registered::configured_headers::configured_headers_match_consumers_and_rebuild_after_template_or_value_changes",
)


def tool(option: str, root: Path, env: dict) -> str:
    return subprocess.run(["soldr", "cc", "--target", TARGET, option], cwd=root,
                          env=env, text=True, capture_output=True, check=True).stdout.strip()


def swift_compiler(root: Path, env: dict) -> Path:
    explicit = env.get("SWIFTC_aarch64-apple-darwin") or env.get("SWIFTC_aarch64_apple_darwin") or env.get("SWIFTC")
    if explicit:
        return Path(explicit).absolute()  # Preserve swiftc argv[0]; resolving its symlink selects swift-driver.
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise ValueError("supply SWIFTC for this Linux host architecture")
    cache = root / "target/macos-native-toolchains"
    compiler = cache / SWIFT_RELEASE / "usr/bin/swiftc"
    if not compiler.is_file():
        cache.mkdir(parents=True, exist_ok=True)
        archive = cache / f"{SWIFT_RELEASE}.tar.gz"
        if not archive.is_file():
            urllib.request.urlretrieve(SWIFT_URL, archive)
        with archive.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if digest != SWIFT_SHA256:
            raise ValueError("Swift toolchain checksum mismatch")
        with tarfile.open(archive) as tar:
            tar.extractall(cache, filter="data")
    return compiler


def target_setting(env: dict, name: str, value: str) -> None:
    for triple in (TARGET, TARGET.replace("-", "_")):
        env[f"{name}_{triple}"] = value


def configure_swift(root: Path, env: dict) -> None:
    compiler = swift_compiler(root, env)
    version = subprocess.run([str(compiler), "--version"], cwd=root, env=env,
                             text=True, capture_output=True, check=True).stdout
    if "Swift version 6." not in version and "Swift version 7." not in version:
        raise ValueError(f"Darwin fixtures require Swift 6 or newer: {version}")
    clang = subprocess.run(["soldr", "cc", "--target", TARGET, "--", "-print-prog-name=clang"],
                           cwd=root, env=env, text=True, capture_output=True, check=True).stdout.strip()
    tools = Path(clang).parent
    if not (tools / "ld64.lld").is_file():
        raise FileNotFoundError(tools / "ld64.lld")
    wrapper = root / "target/macos-native-toolchains/swiftc-darwin"
    wrapper.parent.mkdir(parents=True, exist_ok=True)
    wrapper.write_text(f"#!/bin/sh\nexec {shlex.quote(str(compiler))} -tools-directory {shlex.quote(str(tools))} -use-ld=lld \"$@\"\n")
    wrapper.chmod(0o755)
    target_setting(env, "SWIFTC", str(wrapper))


def signed_fixtures(directory: Path) -> list[str]:
    from macos_test_artifact import verify_macho

    signed = []
    for path in directory.rglob("*"):
        if path.is_file():
            with path.open("rb") as stream:
                header = stream.read(16)
            if len(header) == 16 and struct.unpack_from("<I", header)[0] == 0xFEEDFACF:
                if struct.unpack_from("<I", header, 12)[0] in (2, 6, 8):
                    verify_macho(path, allowed_filetypes=(2, 6, 8))
                    signed.append(str(path.relative_to(directory)))
    if not signed:
        raise ValueError("fixture preparation produced no signed Darwin images")
    return signed


def prepare(root: Path, bundle: Path, env: dict) -> None:
    if platform.system() != "Linux":
        raise ValueError("native fixture compilation is permitted only on the Linux producer")
    directory = bundle / "native-fixtures"
    directory.mkdir(parents=True)
    environment = dict(env)
    environment["OBS_NATIVE_FIXTURES_PREPARE"] = str(directory)
    environment["OBS_NATIVE_HOST_CMAKE"] = shutil.which("cmake") or "cmake"
    cc, cxx, ar, sdk = (tool(option, root, env) for option in (
        "--print-cc", "--print-cxx", "--print-ar", "--print-sysroot"))
    for name, value in (("CC", cc), ("CXX", cxx), ("AR", ar)):
        target_setting(environment, name, value)
    target_setting(environment, "SDKROOT", sdk)
    configure_swift(root, environment)
    toolchain = directory / "darwin.cmake"
    toolchain.write_text("\n".join(f'set({name} "{value}")' for name, value in {
        "CMAKE_SYSTEM_NAME": "Darwin", "CMAKE_OSX_ARCHITECTURES": "arm64",
        "CMAKE_OSX_DEPLOYMENT_TARGET": "13.0", "CMAKE_OSX_SYSROOT": sdk,
        "CMAKE_C_COMPILER": cc, "CMAKE_CXX_COMPILER": cxx, "CMAKE_AR": ar,
    }.items()) + "\n")
    environment["CMAKE_TOOLCHAIN_FILE"] = str(toolchain)
    result = subprocess.run(["soldr", "cargo", "test", "-p", "obs-studio-rust", "--test", "obs-tests-app",
                             "--no-run", "--locked", "--message-format=json"], cwd=root, env=env,
                            capture_output=True, text=True, check=True)
    events = [json.loads(line) for line in result.stdout.splitlines()]
    binaries = [e["executable"] for e in events if e.get("reason") == "compiler-artifact"
                and e.get("target", {}).get("name") == "obs-tests-app" and e.get("executable")]
    if len(binaries) != 1:
        raise ValueError("missing unique host Application category binary")
    names = ["native_build::" + name for name in TESTS]
    for name in names:
        subprocess.run([binaries[0], "--exact", name, "--nocapture"], cwd=root, env=environment, check=True)
        manifest = json.loads((directory / name.replace("::", "--") / "manifest.json").read_text())
        if manifest.get("complete") is not True:
            raise ValueError(f"incomplete native fixture: {name}")
    signed = signed_fixtures(directory)
    (directory / "index.json").write_text(json.dumps({"version": 1, "target": TARGET,
        "tests": names, "signed": signed, "complete": True}, indent=2) + "\n")


def main() -> None:
    import argparse
    import local_gate
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    prepare(root, args.output.resolve(), local_gate.soldr_env({"SOLDR_PORCELAIN": "0", "SOLDR_VERDICTS": "off"}))


if __name__ == "__main__":
    main()
