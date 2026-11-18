#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Prepare the four Windows Qt contracts on Linux, without activating workflows.

    uv run --no-project ci/windows_qt_artifact.py build --work WORK --bundle BUNDLE

The bundle must already contain the Rust/doctest/native slices. Windows execution
is pending: this helper proves compilation, original graph assertions and imports.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
from pathlib import Path

import local_gate
import macos_qt_artifact as shared
import macos_test_artifact as transport
import windows_test_artifact as artifact
from fetch_libs_asset import fetch
from windows_native_fixtures import configure_tools
from windows_zip_check import imports

ROOT, TARGET = artifact.ROOT, artifact.TARGET
CONTRACTS = shared.CONTRACTS
QT_TAG, HOST_ASSET, HOST_SHA = shared.QT_TAG, shared.HOST_ASSET, shared.HOST_SHA
TARGET_ASSET = "qt6-6.11.1-windows-x86_64-relwithdebinfo.tar.zst"
TARGET_SHA = "b05c17386d2a1b0564b00755bc1fba152a18eaac34e512d9315922990715b751"
RELATIVE = "target/windows-qt"


def image_inventory(directory: Path) -> dict[str, list[str]]:
    paths = sorted(path for path in directory.rglob("*") if path.suffix.lower() in (".exe", ".dll"))
    dlls = {path.name.lower() for path in paths if path.suffix.lower() == ".dll"}
    return {str(path.relative_to(directory)): artifact.pe_imports(
        path.read_bytes(), allow_dll=path.suffix.lower() == ".dll", staged_dlls=dlls) for path in paths}


def available_dlls(qt: Path, prefix: Path) -> dict[str, Path]:
    available: dict[str, Path] = {}
    for directory in (qt / "bin", prefix / "bin"):
        for path in directory.glob("*.dll"):
            key = path.name.lower()
            if key in available and path.read_bytes() != available[key].read_bytes():
                raise ValueError(f"ambiguous runtime DLL: {key}")
            available[key] = path
    return available


def stage_runtime(qt: Path, prefix: Path, destination: Path, required: set[str]) -> None:
    """Stage every transitive normal/delay import, plus the dynamically loaded QPA plugin."""
    available = available_dlls(qt, prefix)
    plugin = qt / "plugins/platforms/qminimal.dll"
    pending = [plugin, *(available[name] for name in ("qt6core.dll", "qt6gui.dll", "qt6widgets.dll", "cmocka.dll"))]
    for name in required:
        if name in available:
            pending.append(available[name])
        elif name not in artifact.SYSTEM_DLLS and name not in artifact.VC_DLLS and not name.startswith(("api-ms-win-", "ext-ms-win-")):
            raise ValueError(f"missing runtime DLL: {name}")
    staged = set()
    while pending:
        path = pending.pop()
        if path.name.lower() in staged:
            continue
        staged.add(path.name.lower())
        dependencies = imports(path.read_bytes())
        for name in dependencies:
            if name in available:
                pending.append(available[name])
            elif name not in artifact.SYSTEM_DLLS and name not in artifact.VC_DLLS and not name.startswith(("api-ms-win-", "ext-ms-win-")):
                raise ValueError(f"missing runtime DLL: {name} imported by {path.name}")
        artifact.pe_imports(path.read_bytes(), allow_dll=True, staged_dlls=set(available))
        output = destination / ("plugins/platforms" if path == plugin else "bin") / path.name
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, output)


def validate(data: dict, root: Path) -> None:
    if data.get("qt_contracts_complete") != list(CONTRACTS):
        raise ValueError("incomplete Windows Qt contracts")
    transport.validate_qt_inventory(data)
    runtime = root / RELATIVE
    if image_inventory(runtime) != data.get("qt_images"):
        raise ValueError("Windows Qt PE/import inventory mismatch")
    for name in data["qt_images"]:
        if f"{RELATIVE}/{name}" not in data["files"]:
            raise ValueError("Qt image absent from checksummed inventory")
        audit_runtime(runtime / name, runtime / "qt")
    plugin = f"{RELATIVE}/qt/plugins/platforms/qminimal.dll"
    if plugin not in data["files"]:
        raise ValueError("missing Windows Qt minimal platform plugin")
    for entry in data["tests"]:
        if entry.get("kind") == "qt-contract":
            if "{root}/" + RELATIVE + "/qt/bin" not in entry.get("env_path_prepend", {}).get("PATH", []):
                raise ValueError("Qt runtime directory missing from loader PATH")


def audit_runtime(path: Path, runtime: Path) -> None:
    dlls = {dll.name.lower() for directory in (path.parent, runtime / "bin") for dll in directory.glob("*.dll")}
    artifact.pe_imports(path.read_bytes(), allow_dll=path.suffix.lower() == ".dll", staged_dlls=dlls)


def append_bundle(bundle: Path, work: Path, qt: Path) -> None:
    prefix = work / "cmocka-prefix"
    required = {name for path in (work / "contracts").rglob("*.exe") for name in imports(path.read_bytes())}

    def verify_executable(path):
        artifact.verify_pe_header(path.read_bytes())

    shared.append_contracts(bundle, work, qt, target=TARGET, relative=RELATIVE,
        verify_executable=verify_executable,
        stage_runtime=lambda sdk, destination: stage_runtime(sdk, prefix, destination, required),
        audit_runtime=audit_runtime)
    manifest = json.loads((bundle / "manifest.json").read_text())
    runtime = "{root}/" + RELATIVE + "/qt/bin"
    for entry in manifest["tests"]:
        if entry.get("kind") == "qt-contract":
            entry.setdefault("env_path_prepend", {}).setdefault("PATH", []).insert(0, runtime)
    manifest["qt_images"] = image_inventory(bundle / RELATIVE)
    manifest["qt_inputs"] = {"target_asset": TARGET_ASSET, "target_sha256": TARGET_SHA,
        "host_asset": HOST_ASSET, "host_sha256": HOST_SHA, "cmocka_sha256": shared.CMOCKA_SHA,
        **json.loads((work / "inputs.json").read_text())}
    dlls = {path.name.lower() for path in (bundle / RELATIVE).rglob("*.dll")}
    manifest["images"] = {name: artifact.pe_imports((bundle / name).read_bytes(), staged_dlls=dlls)
                          for name in manifest["files"] if name.endswith(".exe")}
    manifest["runtime_dlls"] = sorted({name for deps in [*manifest["images"].values(), *manifest["qt_images"].values()]
                                       for name in deps if name in artifact.VC_DLLS})
    transport.finalize(bundle, manifest)
    artifact.verify(bundle / "manifest.json")


def cmocka(work: Path, toolchain: Path, env: dict) -> Path:
    return shared.cmocka(work, toolchain, windows=True, env=env)


def replay(manifest: Path, root: Path) -> int:
    if platform.system() != "Windows":
        raise ValueError("Qt execution requires native Windows")
    artifact.verify(manifest)
    return transport.execute(manifest, root, target=TARGET, require_qt=True, kinds={"qt-contract"},
                             validate=lambda data, directory: artifact.verify(directory / "manifest.json"))


def build(work: Path, bundle: Path) -> None:
    if platform.system() != "Linux":
        raise ValueError("Windows Qt compilation requires Linux")
    work.mkdir(parents=True, exist_ok=True)
    contracts = work / "contracts"
    contracts.mkdir()
    qt, host = work / "target-qt", work / "host-qt"
    fetch(QT_TAG, TARGET_ASSET, TARGET_SHA, qt)
    fetch(QT_TAG, HOST_ASSET, HOST_SHA, host)
    tools_dir = work / "producer-tools"
    tools_dir.mkdir(exist_ok=True)
    environment, _ = configure_tools(ROOT, tools_dir, local_gate.soldr_env({"SOLDR_PORCELAIN": "0", "SOLDR_VERDICTS": "off"}))
    toolchain = tools_dir / "windows.cmake"
    with toolchain.open("a") as stream:
        stream.write(f'set(QT_HOST_PATH "{host}")\nset(CMAKE_BUILD_TYPE Release)\n')
    prefix = cmocka(work, toolchain, environment)
    pkg_config = shutil.which("pkg-config")
    if not pkg_config:
        raise ValueError("missing Linux pkgconf/pkg-config producer input")
    version = subprocess.run([pkg_config, "--version"], capture_output=True, text=True, check=True).stdout.strip()
    generators = {}
    for name in ("moc", "uic", "rcc"):
        path = host / "libexec" / name
        result = subprocess.run([str(path), "-v"], capture_output=True, text=True, check=True)
        output = result.stdout + result.stderr
        if "6.11.1" not in output:
            raise ValueError(f"unexpected Linux Qt generator version: {name}: {output}")
        generators[name] = {"version": output.strip(), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
    (work / "inputs.json").write_text(json.dumps({"pkg_config": {"path": pkg_config, "version": version,
        "sha256": hashlib.sha256(Path(pkg_config).read_bytes()).hexdigest()}, "generators": generators}))
    environment.pop("CMAKE_GENERATOR", None)
    environment.update({"OBS_QT_CONTRACT_PREPARE": str(contracts), "OBS_QT_CONTRACT_TARGET": TARGET,
        "OBS_QT_TARGET_PREFIX": str(qt), "OBS_QT_TEST_WINDOWS_PREFIX": str(qt), "OBS_QT_HOST_PREFIX": str(host),
        "HOST": "x86_64-unknown-linux-gnu", "TARGET": TARGET,
        "PKG_CONFIG": pkg_config, "PKG_CONFIG_LIBDIR": str(prefix / "lib/pkgconfig"),
        "PKG_CONFIG_PATH": str(prefix / "lib/pkgconfig"), "PKG_CONFIG_ALLOW_CROSS": "1",
        "CMAKE_TOOLCHAIN_FILE": str(toolchain), "CMAKE_PREFIX_PATH": str(qt), "QT_HOST_PATH": str(host)})
    subprocess.run(["soldr", "cargo", "test", "-p", "obs-studio-rust", "--locked",
        "--config", 'env.CMAKE_GENERATOR.value="Ninja"', "--config", "env.CMAKE_GENERATOR.force=true",
        "--features", "libobs-integration", "--test", "obs-tests-app", "--", "--exact",
        "qt_sdk::qt_sdk_resolves_published_sdks", *CONTRACTS, "--show-output", "--test-threads=1"],
        cwd=ROOT, env=environment, check=True)
    append_bundle(bundle, work, qt)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    build_parser = sub.add_parser("build")
    build_parser.add_argument("--work", type=Path, required=True)
    build_parser.add_argument("--bundle", type=Path, required=True)
    replay_parser = sub.add_parser("run")
    replay_parser.add_argument("--manifest", type=Path, required=True)
    replay_parser.add_argument("--root", type=Path, default=ROOT)
    recorder = sub.add_parser("record")
    recorder.add_argument("--work", type=Path, required=True)
    recorder.add_argument("--contract", choices=CONTRACTS, required=True)
    choice = recorder.add_mutually_exclusive_group(required=True)
    choice.add_argument("--entry", type=json.loads)
    choice.add_argument("--ctest", type=Path)
    args = parser.parse_args()
    if args.action == "record":
        shared.record(args.work, args.contract, args.entry, args.ctest, runtime="Windows")
    elif args.action == "run":
        raise SystemExit(replay(args.manifest.resolve(), args.root.resolve()))
    else:
        build(args.work.resolve(), args.bundle.resolve())


if __name__ == "__main__":
    main()
