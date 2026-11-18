#!/usr/bin/env -S uv run --script
"""Seal the complete Linux-built Tier 2 closure and consume it without a compiler.

The packet uses #5019's common *real* source/build paths. It never relocates
linked pointers or edits signed images. A copied-byte path scan is mandatory.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import struct
import subprocess
import tarfile
import time
import zipfile

import host_platform
import macos_test_workspace as workspace
import macos_tier2_audit as audit
import macos_tier2_bundle as bundle
import macos_tier2_registry as registry

COMMON = Path("/opt/obs-tier2-run-only/src")
BUILD = Path(".tier2/cross")
NATIVE = Path(".tier2/native")
CONTRACT = Path(".tier2/obs-tests-app")
QT = Path(".deps/obs-deps-qt6-2026-10-25-universal")
RUNTIMES = ("libclang_rt.asan_osx_dynamic.dylib", "libclang_rt.ubsan_osx_dynamic.dylib")
TOOLS = ("cmake", "ctest", "nm", "bash")
COMPILERS = ("cc", "c++", "gcc", "g++", "clang", "clang++", "swift", "swiftc",
             "cargo", "rustc", "soldr", "dsymutil", "ninja", "make", "xcrun", "xcodebuild")


def relative(value: str) -> Path:
    if (not value or "\\" in value or any(c in value for c in "\x00\n\r")
            or PurePosixPath(value).is_absolute()
            or any(part in ("", ".", "..") for part in value.split("/"))):
        raise ValueError(f"nonportable packet path: {value!r}")
    return Path(value)


def inventory(root: Path) -> tuple[dict, list[str]]:
    files, directories, names = {}, [], set()
    for path in sorted(root.rglob("*")):
        name = path.relative_to(root).as_posix()
        relative(name)
        if path.is_symlink() or name.casefold() in names:
            raise ValueError(f"symlink or case collision in packet inventory: {name}")
        names.add(name.casefold())
        if path.is_dir():
            directories.append(name)
        elif path.is_file():
            if name != "manifest.json":
                files[name] = {"sha256": bundle.digest(path), "mode": path.stat().st_mode & 0o777}
        else:
            raise ValueError(f"nonregular packet inventory: {name}")
    return files, directories


def seal(root: Path, metadata: dict) -> dict:
    files, directories = inventory(root)
    manifest = {**metadata, "files": files, "directories": directories}
    (root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def verify_files(root: Path, manifest: dict) -> None:
    for name in (*manifest["files"], *manifest["directories"]):
        relative(name)
    if inventory(root) != (manifest["files"], manifest["directories"]):
        raise ValueError("changed/missing/unexpected packet inventory")


def scan(root: Path, forbidden: list[str]) -> dict:
    """Scan every image and resource, not just load commands or CTest strings."""
    needles = [(value, value.encode(encoding)) for value in forbidden if value
               for encoding in ("utf-8", "utf-16le")]
    files = images = 0
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        data = path.read_bytes()
        files += 1
        images += int(data[:4] in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe"))
        for value, needle in needles:
            if needle in data:
                raise ValueError(f"producer path survives in {path.relative_to(root)}: {value}")
    return {"scanned_files": files, "macho_images": images, "producer_path_matches": 0}


def image(path: Path) -> bool:
    data = bundle.arm64_slice(path.read_bytes())
    return len(data) >= 16 and data[:4] == b"\xcf\xfa\xed\xfe" and struct.unpack_from("<I", data, 12)[0] in (2, 6, 8)


def copy(source: Path, destination: Path, verify) -> None:
    """Materialize framework aliases; no archive link may escape the packet."""
    if source.is_dir():
        destination.mkdir(parents=True, exist_ok=True)
        for child in sorted(source.iterdir()):
            copy(child, destination / child.name, verify)
        return
    if not source.is_file():
        raise ValueError(f"missing runtime input: {source}")
    data = source.read_bytes()
    runtime = image(source)
    if runtime:
        verify(source)
        data = bundle.arm64_slice(data)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if destination.exists() and destination.read_bytes() != data:
        raise ValueError(f"conflicting packet input: {destination}")
    destination.write_bytes(data)
    shutil.copymode(source, destination)
    if runtime:
        verify(destination)


def snapshot(build: Path) -> dict:
    return json.loads(subprocess.check_output(
        ["ctest", "--test-dir", str(build), "-C", "RelWithDebInfo", "--show-only=json-v1"], text=True))


def normalized_tests(snapshot_: dict) -> list[dict]:
    # Numeric backtraces are configure-local; every command/property is exact.
    result = []
    for test in snapshot_["tests"]:
        entry = {key: value for key, value in test.items() if key != "backtrace"}
        entry["properties"] = sorted(entry.get("properties", []), key=lambda prop: prop["name"])
        result.append(entry)
    return result


def command_tokens(command: list[str], tools: dict[str, str]) -> list[str]:
    """Only executable slots have host-specific tool identities (#5019)."""
    result = command.copy()
    identities = {alias: f"@OBS_TIER2_TOOL_{name.upper()}@" for name, program in tools.items()
                  for alias in (program, str(Path(program).resolve()))}
    index = 0
    while index < len(command):
        program = command[index]
        if program not in identities:
            break
        result[index] = identities[program]
        if identities[program] != "@OBS_TIER2_TOOL_CMAKE@" or command[index + 1:index + 3] != ["-E", "env"]:
            break
        index += 3
        while index < len(command) and ("=" in command[index] or command[index] == "--"):
            index += 1
    return result


def bracket(text: str) -> str:
    equals = "="
    while f"]{equals}]" in text:
        equals += "="
    return f"[{equals}[{text}]{equals}]"


def ctest_text(tests: list[dict]) -> str:
    """Serialize the actual registry, preserving empty args and list properties."""
    lines = []
    for test in tests:
        lines.append("add_test(" + " ".join(bracket(arg) for arg in [test["name"], *test["command"]]) + ")")
        for prop in test.get("properties", []):
            value = prop["value"]
            if isinstance(value, list) and all(isinstance(item, str) for item in value):
                value = ";".join(item.replace(";", "\\;") for item in value)
            elif isinstance(value, bool):
                value = "TRUE" if value else "FALSE"
            elif isinstance(value, (str, int, float)):
                value = str(value)
            else:
                raise ValueError(f"unsupported CTest property serialization: {prop['name']}")
            lines.append(f"set_tests_properties({bracket(test['name'])} PROPERTIES {bracket(prop['name'])} {bracket(value)})")
    return "\n".join(lines) + "\n"


def prepared_artifacts(native: Path) -> dict:
    prepared = json.loads((native / "macos-prepared.json").read_text())
    required = {"cores", "capture-probe", "mac-capture", "mac-avcapture", "mac-avcapture-legacy", "opengl", "metal", "graphics-probe"}
    if (prepared.get("complete") is not True or prepared.get("waiting")
            or set(prepared["artifacts"]) != required or len(prepared["artifacts"]["cores"]) != 2):
        raise ValueError("incomplete native contract preparation")
    return prepared


def qt_plugins(source: Path) -> list[Path]:
    plugins = sorted((source / QT / "plugins").rglob("*.dylib"))
    required = {source / QT / f"plugins/platforms/libq{name}.dylib" for name in ("minimal", "offscreen")}
    if not required.issubset(plugins):
        raise ValueError("missing Qt runtime platform plugins")
    return plugins


def stage_images(source: Path, output: Path, receipt: Path, contract: Path, prepared: dict, verify) -> None:
    build = source / BUILD
    artifacts = bundle.selected_runtime(build, [Path(line) for line in (build / "tier2-runtime.txt").read_text().splitlines()])
    compiler_rt = Path(json.loads(receipt.read_text())["compiler_rt"])
    dependencies = source / ".deps/obs-deps-2026-10-25-universal/lib"
    roots = artifacts + [contract] + qt_plugins(source)
    for paths in prepared["artifacts"].values():
        for name in paths:
            path = Path(name)
            if path.is_file():
                roots.append(path)
    closure = audit.closure(roots, [source, dependencies, compiler_rt, source / QT / "lib"], verify,
                            build / "test/cmocka", [dependencies, compiler_rt])
    output.mkdir(parents=True)
    for name in closure:
        path = Path(name)
        if path.is_relative_to(compiler_rt):
            destination = Path(".tier2/deps") / path.name
        else:
            destination = bundle.contained(source, path)
        copy(path, output / destination, verify)
        if path.suffix == ".dylib" and path.is_relative_to(dependencies):
            copy(path, output / ".tier2/deps" / path.name, verify)
        for parent in path.parents:
            if parent.suffix in (".plugin", ".framework") and parent.is_relative_to(source):
                copy(parent, output / parent.relative_to(source), verify)
                break
    copy(contract, output / CONTRACT, verify)
    for name in RUNTIMES:
        copy(compiler_rt / name, output / ".tier2/deps" / name, verify)


def stage_resources(source: Path, output: Path, prepared: dict, verify) -> None:
    build, native = source / BUILD, source / NATIVE
    for paths in prepared["artifacts"].values():
        for name in paths:
            path = Path(name)
            if path.is_dir():
                copy(path, output / bundle.contained(source, path), verify)
    copy(native / "macos-prepared.json", output / NATIVE / "macos-prepared.json", verify)
    copy(build / "tier2-modules.txt", output / BUILD / "tier2-modules.txt", verify)
    stage_fixture(source, output, verify)
    for variant in ("obs", "obs-c-reference"):
        copy(native / variant / "libobs.framework", output / NATIVE / variant / "libobs.framework", verify)
    source_resources(source, output, verify)
    for script in ("macos-run-prepared.sh", "macos-assertions.sh"):
        copy(source / "rust/tools/tier2-validate" / script,
             output / "rust/tools/tier2-validate" / script, verify)
    # Preserve *all* configured working directories, including empty ones.
    for path in build.rglob("*"):
        if path.is_dir():
            (output / path.relative_to(source)).mkdir(parents=True, exist_ok=True)


def normalize_fixture(fixture: Path, producer_home: Path) -> dict:
    """Normalize producer-only diagnostics, retaining every prepared-file check.

    The original receipt must pass before copying. No image/object/archive is
    edited. Each diagnostic rewrite records both hashes; the CMake assertion
    script then verifies the corresponding portable hash of *every* file.
    """
    evidence = fixture / "evidence.cmake"
    original_evidence = bundle.digest(evidence)
    receipt = evidence.read_text()
    changed = {}
    replacements = {}
    for path in sorted(fixture.rglob("*")):
        if not path.is_file() or path == evidence:
            continue
        data = path.read_bytes()
        if str(producer_home).encode() not in data:
            continue
        if data[:4] in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe") or data.startswith(b"!<arch>"):
            raise ValueError(f"producer path in compiled fixture; rebuild with prefix maps: {path.name}")
        text = data.decode("utf-8")  # Nontext evidence cannot be rewritten.
        before = bundle.digest(path)
        path.write_text(text.replace(str(producer_home), "/toolchain"))
        after = bundle.digest(path)
        if before not in receipt:
            raise ValueError(f"unhashed producer diagnostic: {path.name}")
        replacements[before] = after
        changed[path.relative_to(fixture).as_posix()] = {"linux_sha256": before, "portable_sha256": after}
    for before, after in replacements.items():
        receipt = receipt.replace(before, after)
    evidence.write_text(receipt)
    return {"normalization": "producer-home-to-logical-toolchain-v1",
            "linux_receipt_sha256": original_evidence, "portable_receipt_sha256": bundle.digest(evidence),
            "files": changed}


def stage_fixture(source: Path, output: Path, verify) -> None:
    relative_ = BUILD / "test/cmocka/core_module_manifest_source"
    command = ["cmake", f"-DOBS_CORE_MODULES_CMAKE={source / 'libobs/cmake/obs-core-modules.cmake'}",
               f"-DTEST_FIXTURE_ROOT={source / relative_}", "-P", str(source / "test/cmocka/check_core_module_manifest_source.cmake")]
    subprocess.run(command, env={**os.environ, "OBS_TIER2_FIXTURE_PREPARED": "1"}, check=True)
    copy(source / relative_, output / relative_, verify)
    receipt = normalize_fixture(output / relative_, Path.home())
    path = output / ".tier2/fixture-normalization.json"
    path.write_text(json.dumps(receipt, indent=2) + "\n")


def source_resources(source: Path, output: Path, verify) -> None:
    for directory in ("rust/obs-build", "rust/obs-studio-rust"):
        (output / directory).mkdir(parents=True, exist_ok=True)
    # Both embedded source fixtures and dynamically located module/core data.
    for relative_ in ("test/cmocka", "libobs/data", "libobs/cmake/obs-core-modules.cmake"):
        path = source / relative_
        if path.is_dir() and relative_ == "test/cmocka":
            for child in path.rglob("*"):
                if child.is_file() and child.suffix not in (".c", ".h", ".cpp", ".hpp"):
                    copy(child, output / child.relative_to(source), verify)
        else:
            copy(path, output / relative_, verify)
    for data in (source / "plugins").glob("*/data"):
        copy(data, output / data.relative_to(source), verify)


def stage_registry(source: Path, output: Path) -> tuple[list[dict], dict]:
    tests = registry.snapshot(source / BUILD, source)["registry"]
    if not tests["tests"] or any(not test.get("command") for test in tests["tests"]):
        raise ValueError("complete CTest command inventory required")
    tools = {name: shutil.which(name) for name in TOOLS}
    if any(program is None for program in tools.values()):
        raise ValueError("producer requires PATH-resolved CMake, CTest, nm and bash")
    runtime_tests = normalized_tests(tests)
    for test in runtime_tests:
        test["command"] = command_tokens(test["command"], tools)
    name = (BUILD / "CTestTestfile.cmake").as_posix()
    templates = {name: ctest_text(runtime_tests)}
    (output / name).write_text(templates[name])
    return runtime_tests, templates


def verify_production(source: Path, sha: str, contract: Path, prepared: dict, runtime: list[Path]) -> None:
    production = json.loads((source / ".tier2/production.json").read_text())
    if (production.get("complete") is not True or production.get("source_sha") != sha
            or source / relative(production["contract"]) != contract):
        raise ValueError("complete exact-source Linux production evidence required")
    roots = runtime + [contract] + [Path(path) for paths in prepared["artifacts"].values()
                                   for path in paths if Path(path).is_file()]
    expected = {bundle.contained(source, path).as_posix() for path in roots}
    if set(production["files"]) != expected:
        raise ValueError("Linux production image inventory mismatch")
    for name, digest in production["files"].items():
        if bundle.digest(source / relative(name)) != digest:
            raise ValueError(f"changed Linux production artifact: {name}")


def stage(source: Path, output: Path, receipt: Path, contract: Path, forbidden: list[str]) -> dict:
    if not host_platform.is_linux():
        raise ValueError("Tier 2 production is Linux-only")
    if source != COMMON or not source.is_dir() or source.resolve() != COMMON:
        raise ValueError("producer must use the common real checkout root before compilation")
    if output.exists():
        raise ValueError("packet output must be absent")
    if subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=source):
        raise ValueError("certification requires a clean source commit")
    sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip()
    verify = bundle.verifier(Path(__file__).with_name("macos_test_artifact.py"))
    prepared = prepared_artifacts(source / NATIVE)
    build = source / BUILD
    runtime = bundle.selected_runtime(build, [Path(line) for line in (build / "tier2-runtime.txt").read_text().splitlines()])
    verify_production(source, sha, contract, prepared, runtime)
    stage_images(source, output, receipt, contract, prepared, verify)
    stage_resources(source, output, prepared, verify)
    copy(source / ".tier2/production.json", output / ".tier2/production.json", verify)
    runtime_tests, templates = stage_registry(source, output)
    forbidden = list(dict.fromkeys([*forbidden, str(Path.home())]))
    result = scan(output, forbidden)
    return seal(output, {
        "schema": 2, "target": "aarch64-apple-darwin", "complete": True,
        "common_root": str(COMMON), "source_sha": sha, "build": BUILD.as_posix(),
        "native": NATIVE.as_posix(), "contract": CONTRACT.as_posix(),
        "native_contract_prebuilt": True, "tests": runtime_tests,
        "ctest_templates": templates, "audit": result,
        "signed_images": sorted(path.relative_to(output).as_posix() for path in output.rglob("*") if path.is_file() and image(path)),
        "dyld_library_path": [".tier2/deps", ".deps/obs-deps-2026-10-25-universal/lib"],
        "sanitizer_runtimes": list(RUNTIMES),
        "qualification_pending": ["same-commit native CTest property comparison", "hosted four-pass execution"],
    })


def verify(root: Path, sha: str) -> dict:
    manifest = json.loads((root / "manifest.json").read_text())
    if (manifest.get("schema") != 2 or manifest.get("target") != "aarch64-apple-darwin"
            or manifest.get("complete") is not True or manifest.get("native_contract_prebuilt") is not True
            or manifest.get("source_sha") != sha or manifest.get("common_root") != str(COMMON)
            or (manifest.get("build"), manifest.get("native"), manifest.get("contract"))
            != (BUILD.as_posix(), NATIVE.as_posix(), CONTRACT.as_posix())):
        raise ValueError("complete exact-source schema-2 macOS Tier 2 packet required")
    verify_files(root, manifest)
    check_signature = bundle.verifier(Path(__file__).with_name("macos_test_artifact.py"))
    actual_images = sorted(path.relative_to(root).as_posix() for path in root.rglob("*") if path.is_file() and image(path))
    if actual_images != manifest["signed_images"] or not actual_images:
        raise ValueError("signed image inventory mismatch")
    for name in actual_images:
        check_signature(root / relative(name))
    for name in (CONTRACT.as_posix(), *(".tier2/deps/" + name for name in RUNTIMES)):
        if name not in manifest["files"] or name not in actual_images:
            raise ValueError(f"unsealed contract/runtime: {name}")
    return manifest


def archive(root: Path, destination: Path, sha: str) -> dict:
    verify(root, sha)
    if destination.exists():
        raise ValueError("archive must be absent")
    started = time.monotonic()
    with tarfile.open(destination, "w:zst", level=3) as output:
        for path in sorted(root.rglob("*")):
            output.add(path, arcname=path.relative_to(root).as_posix(), recursive=False)
    result = {"compressed_bytes": destination.stat().st_size,
              "sha256": workspace.checksum(destination), "pack_seconds": time.monotonic() - started}
    destination.with_suffix(destination.suffix + ".sha256").write_text(result["sha256"] + "\n")
    return result


def download(repository: str, artifact_id: str, sha: str, destination: Path) -> Path:
    if not re.fullmatch(r"[1-9][0-9]*", artifact_id) or not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("immutable numeric artifact ID and full source SHA required")
    metadata = json.loads(subprocess.check_output(
        ["gh", "api", f"repos/{repository}/actions/artifacts/{artifact_id}"], text=True))
    if metadata.get("expired") or metadata.get("workflow_run", {}).get("head_sha") != sha:
        raise ValueError("artifact expired or belongs to another source SHA")
    destination.mkdir(parents=True, exist_ok=True)
    zipped = destination / "artifact.zip"
    with zipped.open("xb") as output:
        subprocess.run(["gh", "api", f"repos/{repository}/actions/artifacts/{artifact_id}/zip"], stdout=output, check=True)
    with zipfile.ZipFile(zipped) as archive_:
        if set(archive_.namelist()) != {"macos-tier2.tar.zst", "macos-tier2.tar.zst.sha256"}:
            raise ValueError("unexpected immutable artifact contents")
        archive_.extractall(destination)
    return destination / "macos-tier2.tar.zst"


def extract(archive_: Path, root: Path, expected_digest: str) -> None:
    if not re.fullmatch(r"[0-9a-f]{64}", expected_digest) or workspace.checksum(archive_) != expected_digest:
        raise ValueError("archive SHA256 mismatch")
    if root.exists():
        raise ValueError("extraction destination must be absent")
    with tarfile.open(archive_, "r:zst") as archive_file:
        members = archive_file.getmembers()
        names = set()
        for member in members:
            relative(member.name)
            if not (member.isfile() or member.isdir()) or member.name.casefold() in names:
                raise ValueError("duplicate or nonregular archive entry")
            names.add(member.name.casefold())
        archive_file.extractall(root, members=members, filter="data")


def render(root: Path, manifest: dict) -> None:
    tools = {}
    for name in TOOLS:
        program = shutil.which(name)
        if not program:
            raise ValueError(f"run host missing runtime utility: {name}")
        tools[f"@OBS_TIER2_TOOL_{name.upper()}@"] = program
    def substitute(text):
        for token, program in tools.items():
            text = text.replace(token, program)
        return text
    for name, template in manifest["ctest_templates"].items():
        if name not in manifest["files"] or (root / relative(name)).read_text() != template:
            raise ValueError("unsealed CTest template")
        (root / name).write_text(substitute(template))
    expected = json.loads(substitute(json.dumps(manifest["tests"])))
    if normalized_tests(snapshot(root / BUILD)) != expected:
        raise ValueError("CTest commands/properties changed in transport")
    verify_tests(root, manifest, expected)


def verify_tests(root: Path, manifest: dict, expected: list[dict]) -> None:
    for test in expected:
        executable = Path(test["command"][0])
        if test["name"] != "test_core_module_manifest_source":
            name = bundle.contained(root, executable).as_posix()
            if name not in manifest["files"] or name not in manifest["signed_images"]:
                raise ValueError(f"unsealed CTest executable: {test['name']}")
        for prop in test.get("properties", []):
            if prop["name"] == "WORKING_DIRECTORY":
                name = bundle.contained(root, Path(prop["value"])).as_posix()
                if name not in manifest["directories"]:
                    raise ValueError(f"unsealed CTest working directory: {test['name']}")


def runtime_environment(root: Path, manifest: dict) -> dict:
    env = dict(os.environ)
    env.update({"OBS_TIER2_BUILD": str(root / BUILD), "OBS_TIER2_NATIVE_OUTPUT": str(root / NATIVE),
                "OBS_TIER2_CONTRACT": str(root / CONTRACT), "OBS_TIER2_DEPS": str(root / ".tier2/deps"),
                "OBS_BUILD_MACOS_RUST_ARCHIVE": "sealed-prebuilt-contract", "OBS_CTEST_JOBS": "3"})
    # All dependency dylibs share the explicit leaf-name override directory,
    # including compiler-rt's preserved signed Conan install names.
    env["DYLD_LIBRARY_PATH"] = ":".join(str(root / relative(name)) for name in manifest["dyld_library_path"])
    env["QT_PLUGIN_PATH"] = str(root / QT / "plugins")
    env["QT_QPA_PLATFORM_PLUGIN_PATH"] = str(root / QT / "plugins/platforms")
    refs = {"OPENGL": BUILD / "libobs-opengl/libobs-opengl.0.dylib",
            "METAL": BUILD / "libobs-metal-managed/libobs-metal.0.dylib"}
    for module in ("mac-capture", "mac-avcapture", "mac-avcapture-legacy"):
        candidates = [name for name in manifest["files"] if name.startswith(BUILD.as_posix() + "/")
                      and name.endswith(f"/{module}.plugin/Contents/MacOS/{module}")]
        if len(candidates) != 1:
            raise ValueError(f"one sealed CMake capture reference required: {module}")
        refs[module.upper().replace("-", "_")] = relative(candidates[0])
    for key, path in refs.items():
        if path.as_posix() not in manifest["files"]:
            raise ValueError(f"unsealed CMake reference: {path}")
        env[f"OBS_BUILD_MACOS_{key}_REFERENCE"] = str(root / path)
    return env


def poison_compilers(directory: Path) -> None:
    directory.mkdir()
    for name in COMPILERS:
        path = directory / name
        path.write_text('#!/bin/sh\necho "compiler forbidden on run host: $0" >&2\nexit 97\n')
        path.chmod(0o755)


def sanitizer_controls(root: Path, env: dict) -> None:
    for kind, test in (("asan", "test_amf3_classname"), ("ubsan", "test_amf_decode")):
        runtime = root / ".tier2/deps" / f"libclang_rt.{kind}_osx_dynamic.dylib"
        command = [str(root / BUILD / "test/cmocka" / test)]
        options = {"cwd": root / BUILD / "test/cmocka", "capture_output": True, "text": True, "timeout": 120}
        loaded = subprocess.run(command, env={**env, "DYLD_PRINT_LIBRARIES": "1"}, check=False, **options)
        if loaded.returncode != 0 or str(runtime) not in loaded.stderr:
            raise ValueError(f"{kind} did not execute with the sealed runtime: {loaded.stderr}")
        absent_env = {key: value for key, value in env.items() if key != "DYLD_LIBRARY_PATH"}
        failed = subprocess.run(command, env=absent_env, check=False, **options)
        if failed.returncode == 0 or runtime.name not in failed.stderr:
            raise ValueError(f"{kind} missing-environment negative control failed")
        hidden = runtime.with_suffix(runtime.suffix + ".held")
        runtime.rename(hidden)
        try:
            failed = subprocess.run(command, env=env, check=False, **options)
            if failed.returncode == 0 or runtime.name not in failed.stderr:
                raise ValueError(f"{kind} missing-runtime negative control failed")
        finally:
            hidden.rename(runtime)


def consume(archive_: Path, digest: str, sha: str, staging: Path) -> int:
    if not host_platform.is_macos():
        raise ValueError("Tier 2 execution requires macOS")
    extract(archive_, staging, digest)
    manifest = verify(staging, sha)
    if COMMON.exists():
        raise ValueError("common runtime root already exists")
    COMMON.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(staging, COMMON)
    # Keep staged tree for diagnostics. No checkout move/restore is needed on
    # this consumer; Actions checkout and post steps keep their original root.
    verify(COMMON, sha)
    render(COMMON, manifest)
    poison = COMMON / ".tier2/no-compiler"
    poison_compilers(poison)
    env = runtime_environment(COMMON, manifest)
    env["PATH"] = str(poison) + os.pathsep + env["PATH"]
    for key in ("CC", "CXX", "OBJC", "OBJCXX", "SWIFTC", "RUSTC", "CARGO"):
        env[key] = str(poison / {"CC": "cc", "CXX": "c++", "OBJC": "cc", "OBJCXX": "c++", "SWIFTC": "swiftc", "RUSTC": "rustc", "CARGO": "cargo"}[key])
    sanitizer_controls(COMMON, env)
    return subprocess.run(["bash", str(COMMON / "rust/tools/tier2-validate/macos-run-prepared.sh")],
                          cwd=COMMON, env=env, check=False).returncode


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    producer = sub.add_parser("stage")
    producer.add_argument("--output", type=Path, required=True)
    producer.add_argument("--darwin-receipt", type=Path, required=True)
    producer.add_argument("--contract", type=Path, required=True)
    producer.add_argument("--forbidden-root", action="append", default=[])
    producer.add_argument("--archive", type=Path, required=True)
    consumer = sub.add_parser("consume")
    consumer.add_argument("--repository", required=True)
    consumer.add_argument("--artifact-id", required=True)
    consumer.add_argument("--sha256", required=True)
    consumer.add_argument("--source-sha", required=True)
    consumer.add_argument("--scratch", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "stage":
        manifest = stage(COMMON, args.output, args.darwin_receipt, args.contract, args.forbidden_root)
        print(json.dumps(archive(args.output, args.archive, manifest["source_sha"]), indent=2))
        return 0
    archive_ = download(args.repository, args.artifact_id, args.source_sha, args.scratch)
    return consume(archive_, args.sha256, args.source_sha, args.scratch / "packet")


if __name__ == "__main__":
    raise SystemExit(main())
