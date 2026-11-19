#!/usr/bin/env -S uv run --script
"""Prepare obs-build's runtime fixtures on Linux for Darwin (#5008)."""
from __future__ import annotations

import json
import hashlib
import errno
import os
import platform
import shlex
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.request
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed

TARGET = "aarch64-apple-darwin"
SWIFT_RELEASE = "swift-6.1.2-RELEASE-ubuntu22.04"
SWIFT_URL = f"https://download.swift.org/swift-6.1.2-release/ubuntu2204/swift-6.1.2-RELEASE/{SWIFT_RELEASE}.tar.gz"
SWIFT_SHA256 = "ed40901b5bccf9022ff5d41425e2316f8e710a7e0c99717ba62080744e81fd66"
# soldr's LLVM-tools catalogue bundle includes dsymutil; its primary LLVM
# cross-link bundle omits it. Pin both the immutable archive and extracted tool.
# LFS media remains under zackees after the forge-ci repository transfer.
LLVM_TOOLS_URL = "https://media.githubusercontent.com/media/zackees/soldr-toolchain/c84b2d6ed993c4fa6106145c90c4654520051cd5/llvm-tools/20.1.7/linux-x64-gnu/bundle.tar.zst"
LLVM_TOOLS_SHA256 = "a359fdba95ab7529a03c3e72410e2bb5aab523c21c1816a96daa2c6fff82d44a"
DSYMUTIL_SHA256 = "61722bc1457bb9a7e630276e17f4c31474bb797fc9518ba915a30f8df4da1e6b"
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
    # Actions keeps RUNNER_TOOL_CACHE across clean checkouts on persistent
    # runners. Never cache mutable fixture outputs or bypass their assertions.
    base = Path(env.get("RUNNER_TOOL_CACHE") or root / "target/macos-native-toolchains")
    cache = base / "obs-darwin-swift" / SWIFT_SHA256
    compiler = cache / SWIFT_RELEASE / "usr/bin/swiftc"
    if cache.exists():
        if not compiler.is_file():
            raise ValueError(f"incomplete Swift toolchain cache: {cache}")
        print(f"Darwin Swift toolchain: cache hit {cache}", flush=True)
        return compiler
    cache.parent.mkdir(parents=True, exist_ok=True)
    print(f"Darwin Swift toolchain: installing {SWIFT_RELEASE} into {cache}", flush=True)
    # A cancelled download/extraction must never expose a partial swiftc as a
    # hit. Concurrent producers may race to publish the same verified payload.
    with tempfile.TemporaryDirectory(prefix=".swift-", dir=cache.parent) as temporary:
        staging = Path(temporary)
        archive = staging / "swift.tar.gz"
        urllib.request.urlretrieve(SWIFT_URL, archive)
        with archive.open("rb") as stream:
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        if digest != SWIFT_SHA256:
            raise ValueError("Swift toolchain checksum mismatch")
        install = staging / "install"
        with tarfile.open(archive) as tar:
            tar.extractall(install, filter="data")
        if not (install / SWIFT_RELEASE / "usr/bin/swiftc").is_file():
            raise ValueError("Swift toolchain archive contains no compiler")
        try:
            install.rename(cache)
        except OSError as error:
            if error.errno not in (errno.EEXIST, errno.ENOTEMPTY) or not compiler.is_file():
                raise
    print("Darwin Swift toolchain: installation complete", flush=True)
    return compiler


def target_setting(env: dict, name: str, value: str) -> None:
    for triple in (TARGET, TARGET.replace("-", "_")):
        env[f"{name}_{triple}"] = value


def provision_dsymutil(root: Path, env: dict) -> Path:
    """Use the same checksummed LLVM-tools payload soldr provisions on Linux."""
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise ValueError("supply DSYMUTIL for this Linux host architecture")
    started = time.monotonic()
    base = Path(env.get("RUNNER_TOOL_CACHE") or root / "target/macos-native-toolchains")
    cache = base / "obs-darwin-dsymutil" / LLVM_TOOLS_SHA256
    relative = Path("package/bin/dsymutil")
    tool = cache / relative
    managed = Path.home() / ".soldr/bin/syslib/llvm-tools/20.1.7/linux-x64-gnu" / relative
    # Probe the published directory once: a concurrent installer may publish
    # between this check and candidate validation, but never exposes half a tool.
    candidates = [managed]
    if cache.exists():
        if not tool.is_file():
            raise ValueError(f"incomplete dsymutil toolchain cache: {cache}")
        candidates.insert(0, tool)
    for candidate in candidates:
        if candidate.is_file():
            with candidate.open("rb") as stream:
                if hashlib.file_digest(stream, "sha256").hexdigest() != DSYMUTIL_SHA256:
                    raise ValueError("managed dsymutil checksum mismatch")
            print(f"Darwin dsymutil: verified cache hit in {time.monotonic() - started:.2f}s: {candidate}", flush=True)
            return candidate
    cache.parent.mkdir(parents=True, exist_ok=True)
    print(f"Darwin dsymutil: installing into {cache}", flush=True)
    # Keep the large archive and partial executable private until both checksums
    # pass. Persistent runners reuse only the extracted immutable tool, as Swift
    # does; clean checkouts must not download the 474 MB payload again.
    with tempfile.TemporaryDirectory(prefix=".dsymutil-", dir=cache.parent) as temporary:
        staging = Path(temporary)
        archive = staging / "bundle.tar.zst"
        downloaded = time.monotonic()
        urllib.request.urlretrieve(LLVM_TOOLS_URL, archive)
        print(f"Darwin dsymutil: archive downloaded in {time.monotonic() - downloaded:.2f}s", flush=True)
        with archive.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != LLVM_TOOLS_SHA256:
                raise ValueError("LLVM-tools archive checksum mismatch")
        install = staging / "install"
        # Python 3.14 supplies zstd. Keep only the regular executable, with its
        # archive mode, and validate it before atomic publication.
        with tarfile.open(archive) as tar:
            member = tar.getmember("./" + str(relative))
            if not member.isfile():
                raise ValueError("LLVM-tools dsymutil is not a regular file")
            tar.extract(member, install, filter="data")
        with (install / relative).open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != DSYMUTIL_SHA256:
                raise ValueError("extracted dsymutil checksum mismatch")
        try:
            install.rename(cache)
        except OSError as error:
            if error.errno not in (errno.EEXIST, errno.ENOTEMPTY):
                raise
    with tool.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != DSYMUTIL_SHA256:
            raise ValueError("managed dsymutil checksum mismatch")
    print(f"Darwin dsymutil: installation complete in {time.monotonic() - started:.2f}s", flush=True)
    return tool


def swift_tools(root: Path, llvm: Path, env: dict) -> Path:
    """Private aliases add dsymutil without changing soldr's shared tool tree."""
    explicit = env.get("DSYMUTIL")
    dsymutil = Path(explicit) if explicit else next(
        (path for path in (llvm / "dsymutil", llvm / "llvm-dsymutil") if path.is_file()), None)
    if dsymutil is None:
        ambient = shutil.which("dsymutil", path=env.get("PATH"))
        dsymutil = Path(ambient) if ambient else provision_dsymutil(root, env)
    if not dsymutil.is_file():
        raise FileNotFoundError(dsymutil)
    identity = hashlib.sha256(f"{llvm}\n{dsymutil}".encode()).hexdigest()[:16]
    tools = root / "target/macos-native-toolchains/swift-tools" / identity
    tools.mkdir(parents=True, exist_ok=True)
    sources = {source.name: source for source in llvm.iterdir() if source.is_file()}
    sources["dsymutil"] = dsymutil
    for name, source in sources.items():
        alias = tools / name
        if not alias.exists():
            alias.symlink_to(source.resolve())
        if alias.resolve() != source.resolve():
            raise ValueError(f"stale Swift tool alias: {alias}")
    return tools


def configure_swift(root: Path, env: dict) -> None:
    compiler = swift_compiler(root, env)
    started = time.monotonic()
    version = subprocess.run([str(compiler), "--version"], cwd=root, env=env,
                             text=True, capture_output=True, check=True).stdout
    if "Swift version 6." not in version and "Swift version 7." not in version:
        raise ValueError(f"Darwin fixtures require Swift 6 or newer: {version}")
    print(f"Darwin fixtures: Swift version checked in {time.monotonic() - started:.2f}s", flush=True)
    started = time.monotonic()
    clang = subprocess.run(["soldr", "cc", "--target", TARGET, "--", "-print-prog-name=clang"],
                           cwd=root, env=env, text=True, capture_output=True, check=True).stdout.strip()
    print(f"Darwin fixtures: soldr clang resolved in {time.monotonic() - started:.2f}s", flush=True)
    tools = Path(clang).parent
    if not (tools / "ld64.lld").is_file():
        raise FileNotFoundError(tools / "ld64.lld")
    tools = swift_tools(root, tools, env)
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


def fixture_workers(env: dict) -> int:
    """Share the producer with other jobs; explicit budgets still obey CPU limits."""
    cpus = max(1, os.cpu_count() or 1)
    value = env.get("OBS_NATIVE_FIXTURE_WORKERS")
    if value is None:
        return min(2, cpus)
    try:
        workers = int(value)
        if workers < 1:
            raise ValueError
    except ValueError as error:
        raise ValueError("OBS_NATIVE_FIXTURE_WORKERS must be a positive integer") from error
    return min(workers, cpus, len(TESTS))


def prepare_fixtures(binary: str, names: list[str], root: Path, directory: Path, env: dict, workers: int) -> None:
    """A fixture's rebuild/mutation sequence remains one indivisible subprocess."""
    def one(name: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run([binary, "--exact", name, "--nocapture"], cwd=root, env=env,
                              capture_output=True, text=True, errors="replace", check=True)

    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = {pool.submit(one, name): name for name in names}
        try:
            for future in as_completed(futures):
                result = future.result()
                print(result.stdout, end="", flush=True)
                print(result.stderr, end="", file=sys.stderr, flush=True)
                name = futures[future]
                manifest = json.loads((directory / name.replace("::", "--") / "manifest.json").read_text())
                if manifest.get("complete") is not True:
                    raise ValueError(f"incomplete native fixture: {name}")
        except BaseException as error:
            for future in futures:
                future.cancel()
            if isinstance(error, subprocess.CalledProcessError):
                print(error.stdout or "", end="", flush=True)
                print(error.stderr or "", end="", file=sys.stderr, flush=True)
            raise


def prepare(root: Path, bundle: Path, env: dict) -> None:
    if platform.system() != "Linux":
        raise ValueError("native fixture compilation is permitted only on the Linux producer")
    workers = fixture_workers(env)
    directory = bundle / "native-fixtures"
    directory.mkdir(parents=True)
    environment = dict(env)
    environment["OBS_NATIVE_FIXTURES_PREPARE"] = str(directory)
    environment["OBS_NATIVE_HOST_CMAKE"] = shutil.which("cmake") or "cmake"
    started = time.monotonic()
    print("Darwin fixtures: preparing toolchain", flush=True)
    cc, cxx, ar, sdk = (tool(option, root, env) for option in (
        "--print-cc", "--print-cxx", "--print-ar", "--print-sysroot"))
    print(f"Darwin fixtures: soldr cc/cxx/ar/SDK resolved in {time.monotonic() - started:.2f}s", flush=True)
    for name, value in (("CC", cc), ("CXX", cxx), ("AR", ar)):
        target_setting(environment, name, value)
    target_setting(environment, "SDKROOT", sdk)
    configure_swift(root, environment)
    print(f"Darwin fixtures: toolchain ready in {time.monotonic() - started:.2f}s", flush=True)
    toolchain = directory / "darwin.cmake"
    toolchain.write_text("\n".join(f'set({name} "{value}")' for name, value in {
        "CMAKE_SYSTEM_NAME": "Darwin", "CMAKE_OSX_ARCHITECTURES": "arm64",
        "CMAKE_OSX_DEPLOYMENT_TARGET": "13.0", "CMAKE_OSX_SYSROOT": sdk,
        "CMAKE_C_COMPILER": cc, "CMAKE_CXX_COMPILER": cxx, "CMAKE_AR": ar,
    }.items()) + "\n")
    environment["CMAKE_TOOLCHAIN_FILE"] = str(toolchain)
    started = time.monotonic()
    print("Darwin fixtures: building host Application category", flush=True)
    result = subprocess.run(["soldr", "cargo", "test", "-p", "obs-studio-rust", "--test", "obs-tests-app",
                             "--no-run", "--locked", "--message-format=json"], cwd=root, env=env,
                            capture_output=True, text=True, check=True)
    print(f"Darwin fixtures: host Application category built in {time.monotonic() - started:.2f}s", flush=True)
    events = [json.loads(line) for line in result.stdout.splitlines()]
    binaries = [e["executable"] for e in events if e.get("reason") == "compiler-artifact"
                and e.get("target", {}).get("name") == "obs-tests-app" and e.get("executable")]
    if len(binaries) != 1:
        raise ValueError("missing unique host Application category binary")
    names = ["native_build::" + name for name in TESTS]
    started = time.monotonic()
    print(f"Darwin fixtures: preparing {len(names)} contracts with {workers} workers", flush=True)
    prepare_fixtures(binaries[0], names, root, directory, environment, workers)
    print(f"Darwin fixtures: contracts and snapshots ready in {time.monotonic() - started:.2f}s", flush=True)
    signed = signed_fixtures(directory)
    # Directory creation now races; preserve fixture inventory order in the index.
    order = {name.replace("::", "--"): index for index, name in enumerate(names)}
    signed.sort(key=lambda path: (order.get(Path(path).parts[0], len(names)), path))
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
