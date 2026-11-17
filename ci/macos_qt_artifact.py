#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Prepare the four published-Qt contracts on Linux; append macOS runtime entries.

    uv run --no-project ci/macos_qt_artifact.py build --work /tmp/qt-work --bundle BUNDLE

BUNDLE is an existing slice-(a) bundle. No workflow is activated by this helper.
Preparation executes the original Rust graph and CMake assertions, and records
target executions as pending. Only the common macOS manifest runner proves them.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import struct
import subprocess
import tarfile
import urllib.request
from pathlib import Path

import macos_test_artifact as artifact
from fetch_libs_asset import fetch

ROOT = artifact.ROOT
TARGET = artifact.TARGET
CONTRACTS = (
    "native_build::native_test_graph_generates_qt_metaobjects",
    "native_build::canonical_remux_queue_runs_unchanged",
    "native_build::canonical_frontend_product_and_meter_tests_run_unchanged",
    "native_build::canonical_frontend_utility_tests_run_unchanged",
)
QT_TAG = "qt6-6.11.1"
TARGET_ASSET = "qt6-6.11.1-darwin-universal2-11.0-release.tar.zst"
TARGET_SHA = "85093eae764f83d167f8850c5b057c0ff5d2f928ad7d49fe2544ce0ca20074b0"
HOST_ASSET = "qt6-6.11.1-linux-x86_64-glibc2.28-release.tar.zst"
HOST_SHA = "f0edfc3432bd57058bbb9a2cf18aacceb55a2564b018346778c34eb0e3369dbf"
CMOCKA_URL = "https://cmocka.org/files/1.1/cmocka-1.1.7.tar.xz"
CMOCKA_SHA = "810570eb0b8d64804331f82b29ff47c790ce9cd6b163e98d47a4807047ecad82"
RELATIVE = "target/macos-qt"


def ctest_entries(inventory: dict) -> list[dict]:
    entries = []
    for test in inventory["tests"]:
        properties = {prop["name"]: prop["value"] for prop in test["properties"]}
        for name in ("WILL_FAIL", "DISABLED", "PASS_REGULAR_EXPRESSION", "FAIL_REGULAR_EXPRESSION",
                     "SKIP_REGULAR_EXPRESSION", "SKIP_RETURN_CODE",
                     "FIXTURES_REQUIRED", "FIXTURES_SETUP", "FIXTURES_CLEANUP", "REQUIRED_FILES"):
            if properties.get(name):
                raise ValueError(f"unsupported execution property: {name}")
        entries.append({"name": test["name"], "binary": test["command"][0], "args": test["command"][1:],
                        "cwd": properties["WORKING_DIRECTORY"],
                        "env": dict(value.split("=", 1) for value in properties.get("ENVIRONMENT", [])),
                        "timeout": properties.get("TIMEOUT", 60.0)})
        prepend = ctest_path_prepend(properties.get("ENVIRONMENT_MODIFICATION", []))
        if prepend:
            entries[-1]["env_path_prepend"] = prepend
    if not entries:
        raise ValueError("empty CTest runtime inventory")
    return entries


def ctest_path_prepend(modifications: list[str]) -> dict[str, list[str]]:
    result: dict[str, list[str]] = {}
    for value in modifications:
        key, operation = value.split("=", 1)
        action, path = operation.split(":", 1)
        if action != "path_list_prepend":
            raise ValueError(f"unsupported execution property ENVIRONMENT_MODIFICATION: {value}")
        result.setdefault(key, []).insert(0, path)
    return result


def record(work: Path, contract: str, entry: dict | None, ctest: Path | None) -> None:
    if contract not in CONTRACTS:
        raise ValueError(f"unexpected Qt contract: {contract}")
    entries = [entry] if entry is not None else ctest_entries(json.loads(subprocess.run(
        ["ctest", "--test-dir", str(ctest), "-C", "Release", "--show-only=json-v1"],
        capture_output=True, text=True, check=True).stdout))
    with (work / "records.jsonl").open("a") as stream:
        for item in entries:
            stream.write(json.dumps(item | {"contract": contract}) + "\n")
    print(f"Prepared {len(entries)} target executions for {contract}; macOS runtime PENDING")


def read_records(work: Path) -> list[dict]:
    entries = [json.loads(line) for line in (work / "records.jsonl").read_text().splitlines()]
    actual = {entry["contract"] for entry in entries}
    if actual != set(CONTRACTS):
        raise ValueError(f"incomplete Qt contracts: missing={set(CONTRACTS) - actual}, extra={actual - set(CONTRACTS)}")
    return entries


def portable(value: str, mappings: dict[str, str]) -> str:
    for source, destination in sorted(mappings.items(), key=lambda pair: -len(pair[0])):
        value = value.replace(source, destination)
    if value.startswith("/"):
        raise ValueError(f"unmapped producer path: {value}")
    return value


def toolchain(work: Path) -> Path:
    def query(option):
        return subprocess.run(["soldr", "cc", "--target", TARGET, option],
                              capture_output=True, text=True, check=True).stdout.strip()
    cc, cxx, sdk = (query(option) for option in ("--print-cc", "--print-cxx", "--print-sysroot"))
    path = work / "darwin.cmake"
    settings = {"CMAKE_SYSTEM_NAME": "Darwin", "CMAKE_SYSTEM_PROCESSOR": "arm64", "CMAKE_OSX_ARCHITECTURES": "arm64",
                "CMAKE_OSX_SYSROOT": sdk, "CMAKE_OSX_DEPLOYMENT_TARGET": "11.0", "CMAKE_C_COMPILER": cc,
                "CMAKE_CXX_COMPILER": cxx, "CMAKE_OBJC_COMPILER": cc, "CMAKE_OBJCXX_COMPILER": cxx,
                "CMAKE_TRY_COMPILE_TARGET_TYPE": "STATIC_LIBRARY", "CMAKE_BUILD_TYPE": "Release",
                "QT_HOST_PATH": str(work / "host-qt"), "CMAKE_BUILD_WITH_INSTALL_RPATH": "TRUE",
                "CMAKE_INSTALL_RPATH": "@executable_path/../../../qt/lib"}
    path.write_text("".join(f'set({key} "{value}")\n' for key, value in settings.items()))
    return path


def cmocka(work: Path, toolchain_file: Path) -> Path:
    archive = work / "cmocka-1.1.7.tar.xz"
    if not archive.exists():
        urllib.request.urlretrieve(CMOCKA_URL, archive)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != CMOCKA_SHA:
        raise ValueError("cmocka archive checksum mismatch")
    source = work / "cmocka-1.1.7"
    if not source.exists():
        with tarfile.open(archive) as stream:
            stream.extractall(work, filter="data")
    output = work / "cmocka-build"
    subprocess.run(["cmake", "-S", str(source), "-B", str(output), "-G", "Ninja",
                    f"-DCMAKE_TOOLCHAIN_FILE={toolchain_file}", "-DBUILD_SHARED_LIBS=OFF",
                    "-DUNIT_TESTING=OFF", "-DWITH_EXAMPLES=OFF", "-DCMAKE_POLICY_VERSION_MINIMUM=3.5"], check=True)
    subprocess.run(["cmake", "--build", str(output), "--target", "cmocka", "--parallel", "2"], check=True)
    prefix = work / "cmocka-prefix"
    (prefix / "lib/pkgconfig").mkdir(parents=True, exist_ok=True)
    shutil.copytree(source / "include", prefix / "include", dirs_exist_ok=True)
    shutil.copy2(output / "src/libcmocka.a", prefix / "lib/libcmocka.a")
    (prefix / "lib/pkgconfig/cmocka.pc").write_text(
        f"prefix={prefix}\nName: cmocka\nDescription: Darwin cross-built cmocka\nVersion: 1.1.7\n"
        "Libs: -L${prefix}/lib -lcmocka\nCflags: -I${prefix}/include\n")
    return prefix


def arm64_image(data: bytes) -> bytes:
    if data[:4] == b"\xca\xfe\xba\xbe":
        count = struct.unpack_from(">I", data, 4)[0]
        for index in range(count):
            cpu, _, offset, size, _ = struct.unpack_from(">5I", data, 8 + index * 20)
            if cpu == 0x100000C:
                return data[offset:offset + size]
        raise ValueError("universal Qt library has no arm64 slice")
    return data


def load_paths(path: Path) -> list[tuple[int, str]]:
    data = path.read_bytes()
    count = struct.unpack_from("<I", data, 16)[0]
    offset, result = 32, []
    for _ in range(count):
        command, size = struct.unpack_from("<2I", data, offset)
        if command in (0xC, 0xD, 0x80000018, 0x8000001C, 0x8000001F, 0x80000023):
            start = offset + struct.unpack_from("<I", data, offset + 8)[0]
            result.append((command, data[start:offset + size].split(b"\0", 1)[0].decode()))
        offset += size
    return result


def audit_linkage(path: Path, runtime: Path) -> None:
    paths = load_paths(path)
    for command, value in paths:
        if value.startswith(("/usr/lib/", "/System/Library/")):
            continue
        if command == 0x8000001C and value.startswith(("@executable_path", "@loader_path")):
            continue
        if value.startswith("@rpath/") and (runtime / "lib" / value.removeprefix("@rpath/")).is_file():
            continue
        raise ValueError(f"unbundled Mach-O load path in {path}: {value}")
    if struct.unpack_from("<I", path.read_bytes(), 12)[0] == 2 and any(value.startswith("@rpath/") for _, value in paths):
        rpaths = {Path(value.replace("@executable_path", str(path.parent)).replace("@loader_path", str(path.parent))).resolve()
                  for command, value in paths if command == 0x8000001C}
        if (runtime / "lib").resolve() not in rpaths:
            raise ValueError(f"executable cannot resolve bundled Qt frameworks: {path}")


def stage_qt(qt: Path, destination: Path) -> None:
    for module in ("Core", "Gui", "Widgets"):
        name = "Qt" + module
        library = destination / "lib" / (name + ".framework/Versions/A") / name
        library.parent.mkdir(parents=True, exist_ok=True)
        library.write_bytes(arm64_image((qt / "lib" / (name + ".framework/Versions/A") / name).read_bytes()))
        library.chmod(0o755)
        artifact.verify_macho(library, allowed_filetypes=(6,))
    plugin = destination / "plugins/platforms/libqminimal.dylib"
    plugin.parent.mkdir(parents=True, exist_ok=True)
    plugin.write_bytes(arm64_image((qt / "plugins/platforms/libqminimal.dylib").read_bytes()))
    plugin.chmod(0o755)
    artifact.verify_macho(plugin, allowed_filetypes=(6, 8))
    for library in destination.rglob("*"):
        if library.is_file():
            audit_linkage(library, destination)


def append_bundle(bundle: Path, work: Path, qt: Path) -> None:
    manifest = json.loads((bundle / "manifest.json").read_text())
    if (manifest["build_root"] != str(ROOT) or manifest.get("qt_contracts_complete")
            or manifest["version"] != 1 or manifest["target"] != TARGET):
        raise ValueError("Qt bundle source root mismatch or already complete")
    artifact.validate_inventory(manifest, bundle)
    entries = read_records(work / "contracts")
    destination = bundle / RELATIVE
    shutil.copytree(work / "contracts", destination / "contracts")
    stage_qt(qt, destination / "qt")
    mappings = {str(work / "contracts"): "{root}/" + RELATIVE + "/contracts", str(ROOT): "{root}",
                str(qt): "{root}/" + RELATIVE + "/qt"}
    for entry in entries:
        binary = Path(entry["binary"])
        artifact.verify_macho(binary)
        item = {key: portable(entry[key], mappings) for key in ("binary", "cwd")}
        item = {key: value.removeprefix("{root}/") for key, value in item.items()}
        audit_linkage(bundle / item["binary"], destination / "qt")
        item.update({"package": "Qt/" + entry["contract"], "name": entry["name"], "kind": "qt-contract",
                     "args": [portable(value, mappings) for value in entry["args"]],
                     "env": {key: portable(value, mappings) for key, value in entry["env"].items()},
                     "timeout": entry["timeout"]})
        item["env_path_prepend"] = {key: [portable(value, mappings) for value in values]
                                    for key, values in entry.get("env_path_prepend", {}).items()}
        runtime = "{root}/" + RELATIVE + "/qt"
        item["env"].update({"QT_PLUGIN_PATH": runtime + "/plugins",
                            "QT_QPA_PLATFORM_PLUGIN_PATH": runtime + "/plugins/platforms"})
        manifest["tests"].append(item)
    manifest["qt_contracts_complete"] = list(CONTRACTS)
    artifact.finalize(bundle, manifest)


def build(work: Path, bundle: Path) -> None:
    import local_gate

    work.mkdir(parents=True, exist_ok=True)
    contracts = work / "contracts"
    contracts.mkdir()  # stale preparation cannot prove coverage
    qt, host = work / "target-qt", work / "host-qt"
    fetch(QT_TAG, TARGET_ASSET, TARGET_SHA, qt)
    fetch(QT_TAG, HOST_ASSET, HOST_SHA, host)
    toolchain_file = toolchain(work)
    prefix = cmocka(work, toolchain_file)
    env = local_gate.soldr_env({"SOLDR_PORCELAIN": "0", "SOLDR_VERDICTS": "off"})
    env.update({"OBS_MACOS_QT_PREPARE": str(contracts), "OBS_QT_TARGET_PREFIX": str(qt),
                "OBS_QT_HOST_PREFIX": str(host), "HOST": "x86_64-unknown-linux-gnu", "TARGET": TARGET,
                "PKG_CONFIG_LIBDIR": str(prefix / "lib/pkgconfig"), "PKG_CONFIG_PATH": str(prefix / "lib/pkgconfig"),
                "PKG_CONFIG_ALLOW_CROSS": "1", "CMAKE_TOOLCHAIN_FILE": str(toolchain_file),
                "CMAKE_PREFIX_PATH": str(qt), "QT_HOST_PATH": str(host),
                "CC_aarch64_apple_darwin": subprocess.run(["soldr", "cc", "--target", TARGET, "--print-cc"],
                                                        capture_output=True, text=True, check=True).stdout.strip(),
                "CXX_aarch64_apple_darwin": subprocess.run(["soldr", "cc", "--target", TARGET, "--print-cxx"],
                                                         capture_output=True, text=True, check=True).stdout.strip()})
    subprocess.run(["soldr", "cargo", "test", "-p", "obs-studio-rust", "--locked", "--features", "libobs-integration",
                    "--test", "obs-tests-app", "--", "--exact", *CONTRACTS, "--show-output", "--test-threads=1"],
                   cwd=ROOT, env=env, check=True)
    append_bundle(bundle, work, qt)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    producer = sub.add_parser("build")
    producer.add_argument("--work", type=Path, required=True)
    producer.add_argument("--bundle", type=Path, required=True)
    recorder = sub.add_parser("record")
    recorder.add_argument("--work", type=Path, required=True)
    recorder.add_argument("--contract", choices=CONTRACTS, required=True)
    choice = recorder.add_mutually_exclusive_group(required=True)
    choice.add_argument("--entry", type=json.loads)
    choice.add_argument("--ctest", type=Path)
    args = parser.parse_args()
    if args.action == "record":
        record(args.work, args.contract, args.entry, args.ctest)
    else:
        build(args.work.resolve(), args.bundle.resolve())


if __name__ == "__main__":
    main()
