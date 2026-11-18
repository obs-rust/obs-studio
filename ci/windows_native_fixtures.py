#!/usr/bin/env -S uv run --script
"""Linux preparation of MSVC native contracts using the shared fixture transport."""
from __future__ import annotations

import json
import platform
import shlex
import shutil
import subprocess
from pathlib import Path

import macos_native_fixtures as shared
from windows_test_artifact import TARGET, pe_imports
from windows_cross_tools import sdk_libraries

# Preserve the default Windows contracts; Darwin language/PCH variants keep their
# original host guards. Qt and feature-gated libobs contracts are separate slices.
TESTS = tuple(name for name in shared.TESTS if name not in {
    "swift_bridge_preserves_native_linkage_header_flags_and_source_rebuilds",
    "pch_preserves_mixed_languages_unity_and_header_flag_invalidation",
    "mixed_objective_languages_compile_load_and_rebuild_with_unity_and_pch",
    "arc_and_manual_groups_keep_source_flags_through_expansion_and_pch",
}) + (
    "windows_resource_payload_tracks_headers_definitions_and_unity",
    "msvc_pch_links_its_object_and_preserves_language_header_and_flag_changes",
)


def fixture_images(directory: Path) -> dict[str, list[str]]:
    # Blob names are content hashes; recover DLL basenames from every snapshot
    # rather than guessing dependencies from the blob's extension.
    images, dlls = {}, set()
    for manifest in directory.glob("*/manifest.json"):
        for event in json.loads(manifest.read_text())["events"]:
            for entry in event["files"]:
                destination = entry["destination"]
                if destination.endswith(".dll"):
                    dlls.add(Path(destination).name.lower())
                if destination.endswith((".exe", ".dll")):
                    images[entry["file"]] = destination.endswith(".dll")
    if not images:
        raise ValueError("no PE native fixture images")
    return {name: pe_imports((directory / name).read_bytes(), allow_dll=is_dll, staged_dlls=dlls)
            for name, is_dll in sorted(images.items())}


def validate(root: Path) -> None:
    directory = root / "native-fixtures"
    index = json.loads((directory / "index.json").read_text())
    names = ["native_build::" + name for name in TESTS]
    if (index.get("version") != 1 or index.get("target") != TARGET
            or index.get("complete") is not True or index.get("tests") != names):
        raise ValueError("incomplete Windows fixture inventory")
    for name in names:
        manifest = json.loads((directory / name.replace("::", "--") / "manifest.json").read_text())
        if (manifest.get("complete") is not True or manifest.get("target") != TARGET
                or "linux" not in manifest.get("host", "") or not manifest.get("events")):
            raise ValueError(f"incompatible Windows fixture evidence: {name}")
    if fixture_images(directory) != index.get("images"):
        raise ValueError("Windows native PE/import inventory mismatch")


def prepare(root: Path, bundle: Path, env: dict) -> None:
    if platform.system() != "Linux":
        raise ValueError("native fixture compilation requires the Linux producer")
    directory = bundle / "native-fixtures"
    directory.mkdir(parents=True)
    environment = dict(env)
    environment.update(OBS_NATIVE_FIXTURES_PREPARE=str(directory),
                       OBS_NATIVE_FIXTURE_TARGET=TARGET, OBS_NATIVE_WINDOWS_TARGET=TARGET,
                       OBS_NATIVE_HOST_CMAKE=shutil.which("cmake") or "cmake")
    environment, tools = configure_tools(root, directory, environment)
    shared.prepare_tests(root, directory, environment, env, TESTS)
    images = fixture_images(directory)
    (directory / "index.json").write_text(json.dumps({"version": 1, "target": TARGET,
        "tests": ["native_build::" + name for name in TESTS], "images": images,
        "tools": tools, "complete": True}, indent=2) + "\n")
    validate(bundle)


def configure_tools(root: Path, directory: Path, env: dict) -> tuple[dict, dict]:
    """Resolve the same managed MSVC tools for native and Qt preparation."""
    environment = dict(env)
    tools = {name: shared.tool(option, root, env, target=TARGET) for name, option in {
        "CC": "--print-cc", "CXX": "--print-cxx", "AR": "--print-ar", "RC": "--print-rc",
        "SDKROOT": "--print-sysroot", "LINK": "--print-linker",
    }.items()}
    # soldr's GNU-style SDK driver cannot be switched to CL mode: -isystem/-L
    # become file operands. Resolve its managed clang and translate SDK settings
    # explicitly to CL syntax, preserving the case-insensitive Windows headers.
    clang = subprocess.run([tools["CC"], "-print-prog-name=clang"], cwd=root, env=env,
                           text=True, capture_output=True, check=True).stdout.strip()
    sdk = Path(tools["SDKROOT"])
    overlays = sorted(sdk.glob("soldr-case-overlay-*.yaml"))
    if len(overlays) != 1:
        raise ValueError("missing unique soldr xwin case overlay")
    includes = [sdk / "crt/include", *(sdk / "sdk/include" / part for part in ("ucrt", "um", "shared", "winrt", "cppwinrt"))]
    libraries = sdk_libraries(sdk)
    flags = ["--driver-mode=cl", f"--target={TARGET}", "-fuse-ld=lld", "-vfsoverlay", str(overlays[0])]
    for include in includes:
        flags.extend(["-imsvc", str(include)])
    # lld-link uses the Windows LIB convention even when it runs on Linux.
    environment["LIB"] = ";".join(map(str, libraries))
    tools["LINK"] = subprocess.run([tools["CC"], "-print-prog-name=lld-link"], cwd=root, env=env,
                                   text=True, capture_output=True, check=True).stdout.strip()
    for name in ("CC", "CXX"):
        wrapper = directory / f"{name.lower()}-clang-cl"
        # clang-cl rejects existing try/catch code without /EHsc, whereas MSVC
        # accepts it. Keep exceptions enabled for the real C++ contracts.
        language_flags = ["/EHsc"] if name == "CXX" else []
        wrapper.write_text(f'#!/bin/sh\nexec {shlex.join([clang, *flags, *language_flags])} "$@"\n')
        wrapper.chmod(0o755)
        tools[name] = str(wrapper)
    for name, value in tools.items():
        shared.target_setting(environment, name, value, target=TARGET)
    # CMake uses Ninja on Linux and the same cross SDK, never native discovery.
    toolchain = directory / "windows.cmake"
    toolchain.write_text("\n".join(f'set({name} "{value}")' for name, value in {
        "CMAKE_SYSTEM_NAME": "Windows", "CMAKE_SYSTEM_PROCESSOR": "AMD64",
        "CMAKE_MSVC_RUNTIME_LIBRARY": "MultiThreadedDLL",
        "CMAKE_C_COMPILER": tools["CC"], "CMAKE_CXX_COMPILER": tools["CXX"],
        "CMAKE_AR": tools["AR"], "CMAKE_RC_COMPILER": tools["RC"], "CMAKE_LINKER": tools["LINK"],
        "CMAKE_MT": str(Path(tools["LINK"]).with_name("llvm-mt")),
    }.items()) + "\n")
    environment.update(CMAKE_TOOLCHAIN_FILE=str(toolchain), CMAKE_GENERATOR="Ninja")
    return environment, tools
