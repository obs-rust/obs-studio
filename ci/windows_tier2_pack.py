#!/usr/bin/env -S uv run --script
"""Stage the CMake Windows Tier 2 closure. Cargo parts remain explicitly pending.

    uv run --python 3.14 --no-project ci/windows_tier2_pack.py --build BUILD --output BUNDLE --archive PACK --dll-root DEPS

This producer never grants native inventory parity or full-suite admission.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import time

import windows_tier2_inventory as registry
import windows_tier2_fixtures as fixtures
import windows_tier2_probe as probe
import windows_tier2_run as consumer

CEILING = 500 * 1024**2
TARGET = 250 * 1024**2


def audit(root: Path, forbidden: list[str]) -> dict:
    """Inspect every copied byte, including UTF-16 PE/resource strings."""
    scanned, images = 0, 0
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        data = path.read_bytes()
        scanned += 1
        images += int(path.suffix.lower() in (".exe", ".dll"))
        for value in forbidden:
            for spelling in {value, value.replace("/", "\\")}:
                if any(
                    spelling.encode(encoding) in data
                    for encoding in ("utf-8", "utf-16le")
                ):
                    raise ValueError(
                        f"producer path survives in {path.relative_to(root)}: {value}"
                    )
    return {"scanned_files": scanned, "pe_images": images, "producer_path_matches": 0}


def copy_path(source: Path, destination: Path) -> None:
    """Copy directories themselves too, including intentional empty fixtures."""
    if source.is_symlink():
        raise ValueError(f"symlink in runtime data: {source}")
    if source.is_dir():
        destination.mkdir(parents=True, exist_ok=True)
        for child in sorted(source.iterdir()):
            copy_path(child, destination / child.name)
    elif source.is_file():
        destination.parent.mkdir(parents=True, exist_ok=True)
        if destination.exists() and probe.digest(source) != probe.digest(destination):
            raise ValueError(f"conflicting runtime file: {destination}")
        shutil.copyfile(source, destination)
    else:
        raise ValueError(f"missing runtime resource: {source}")


def copied_inventory(root: Path) -> tuple[dict, list[str]]:
    files, directories, names = {}, [], set()
    for path in sorted(root.rglob("*")):
        name = path.relative_to(root).as_posix()
        consumer.relative(name)
        if name.casefold() in names or path.is_symlink():
            raise ValueError(f"nonportable/duplicate runtime name: {name}")
        names.add(name.casefold())
        if path.is_dir():
            directories.append(name)
        elif path.is_file():
            files[name] = probe.digest(path)
        else:
            raise ValueError(f"nonregular runtime member: {name}")
    return files, directories


def tokens(build: Path, source: Path) -> dict[str, str]:
    result = {build.as_posix(): probe.BUILD, source.as_posix(): probe.SOURCE}
    cache = build / "CMakeCache.txt"
    if cache.is_file():
        for key, value in re.findall(
            r"^(CMAKE_(?:COMMAND|C_COMPILER|CXX_COMPILER|MAKE_PROGRAM)):[^=]+=(.+)$",
            cache.read_text(),
            re.M,
        ):
            identity = "CMAKE" if key == "CMAKE_COMMAND" else key.removeprefix("CMAKE_")
            result[value] = f"@OBS_TIER2_TOOL_{identity}@"
    # Resolve the PATH identity rather than assuming an installation prefix.
    for name in ("cmake", "ctest"):
        executable = shutil.which(name)
        if executable:
            result[executable] = f"@OBS_TIER2_TOOL_{name.upper()}@"
    return dict(sorted(result.items(), key=lambda item: len(item[0]), reverse=True))


def stage_data(build: Path, source: Path, output: Path, products: list[Path]) -> None:
    for name in ("libobs/data", "test/cmocka/data"):
        copy_path(source / name, output / "source" / name)
    copy_path(source / "libobs/data", output / "build/data/libobs")
    stage_owners(build, source, output, products)
    # Canonical resource helper's installed data layout and generated fixtures.
    for data in sorted((build / "rundir").glob("**/data")):
        if data.is_dir():
            copy_path(data, output / "build" / data.relative_to(build))
    # Only runtime fixture outputs; never ship CMake/Ninja/PCH/OBJ/PDB state.
    for path in sorted((build / "test/cmocka").iterdir()):
        if (
            path.is_file()
            and path.suffix.lower() in {".png", ".gif", ".ini", ".json", ".mkv", ".txt"}
            and path.name != "CMakeLists.txt"
        ):
            copy_path(path, output / "build/test/cmocka" / path.name)


def stage_owners(build: Path, source: Path, output: Path, products: list[Path]) -> None:
    owners = {
        p.relative_to(build).parts[1]
        for p in products
        if p.is_relative_to(build / "plugins")
    }
    for owner in sorted(owners):
        data = source / "plugins" / owner / "data"
        if data.is_dir():
            copy_path(data, output / "source/plugins" / owner / "data")
            copy_path(data, output / "build/core" / owner / "data")
        for product in products:
            if product.parent == build / "plugins" / owner:
                copy_path(product, output / "build/core" / owner / product.name)
        if owner == "win-capture":
            stage_capture_helpers(build, output, products)


def stage_capture_helpers(build: Path, output: Path, products: list[Path]) -> None:
    names = {"graphics-hook64.dll", "inject-helper64.exe", "get-graphics-offsets64.exe"}
    helpers = [
        p
        for p in products
        if p.is_relative_to(build / "plugins/win-capture") and p.name in names
    ]
    if {p.name for p in helpers} != names:
        raise ValueError("missing win-capture module-data helpers")
    for path in helpers:
        for base in ("build/core/win-capture/data", "source/plugins/win-capture/data"):
            copy_path(path, output / base / path.name)


def stage_arguments(build: Path, source: Path, output: Path, tests: list[dict]) -> None:
    for test in tests:
        command = test["command"]
        if Path(command[0]).suffix.lower() != ".exe":
            continue  # The known compile-only fixture is prepared separately.
        for value in command[1:]:
            path = Path(value)
            if not path.is_absolute():
                continue
            if path.is_relative_to(build):
                destination = output / "build" / path.relative_to(build)
            elif path.is_relative_to(source):
                destination = output / "source" / path.relative_to(source)
            else:
                raise ValueError(f"runtime argument outside logical roots: {value}")
            copy_path(path, destination)


def normalize_tests(tests: list[dict], build: Path, source: Path) -> list[dict]:
    replacements = tokens(build, source)
    for test in tests:
        if test["name"] != "test_core_module_manifest_source":
            continue
        for value in test["command"]:
            for key in ("C_COMPILER", "MAKE_PROGRAM", "GENERATOR_INSTANCE"):
                prefix = f"-DTEST_{key}="
                if value.startswith(prefix) and value[len(prefix) :]:
                    replacements[value[len(prefix) :]] = f"@OBS_TIER2_TOOL_{key}@"
    resolved = []
    for test in tests:
        command = list(test["command"])
        if command[0] in {"cmake", "ctest"}:
            executable = shutil.which(command[0])
            if executable is None:
                raise ValueError(f"missing producer tool: {command[0]}")
            command[0] = executable
        resolved.append({**test, "command": command})
    return consumer.normalized(
        resolved,
        dict(sorted(replacements.items(), key=lambda item: len(item[0]), reverse=True)),
    )


def prepare_fixture(
    build: Path, source: Path, output: Path, tests: list[dict], sha: str
) -> dict:
    if not any(t["name"] == "test_core_module_manifest_source" for t in tests):
        return {}
    receipt = fixtures.prepare(
        source, build, build / "stale-manifest-receipt.json", sha
    )
    receipt["command"] = normalize_tests(
        [
            {
                "name": "test_core_module_manifest_source",
                "command": receipt["command"],
                "properties": [],
            }
        ],
        build,
        source,
    )[0]["command"]
    name = "receipts/stale-manifest.json"
    (output / "receipts").mkdir(exist_ok=True)
    (output / name).write_text(json.dumps(receipt, indent=2) + "\n")
    for relative in (*fixtures.INPUTS, "ci/windows_tier2_assert_fixture.cmake"):
        copy_path(source / relative, output / "source" / relative)
    return {"test_core_module_manifest_source": name}


def release_image(path: Path) -> bool:
    return not any(
        name == "ucrtbased.dll"
        or re.fullmatch(r"(?:msvcp|vcruntime|concrt).*d\.dll", name)
        for name in probe.image(path.read_bytes())
    )


def plugin_images(tests: list[dict], source: Path, output: Path) -> list[Path]:
    """Qt loads platform/image/style DLLs dynamically, outside the PE import graph."""
    roots = set()
    prefix = "QT_PLUGIN_PATH=path_list_prepend:"
    for test in tests:
        for prop in test.get("properties", []):
            if prop["name"] == "ENVIRONMENT_MODIFICATION":
                roots.update(
                    Path(value[len(prefix) :])
                    for value in prop["value"]
                    if value.startswith(prefix)
                )
    images = []
    for root in sorted(roots):
        probe.contained(source, root)
        if not (root / "platforms/qminimal.dll").is_file():
            raise ValueError("missing Qt minimal platform plugin")
        for path in sorted(root.rglob("*")):
            if path.is_symlink():
                raise ValueError("symlink in Qt plugin tree")
            if path.is_dir():
                (output / "source" / path.relative_to(source)).mkdir(
                    parents=True, exist_ok=True
                )
            elif path.suffix.lower() == ".dll" and release_image(path):
                images.append(path)
    return images


def destination_name(path: Path, build: Path, source: Path) -> Path:
    if path.is_relative_to(build):
        return Path("build") / path.relative_to(build)
    if path.is_relative_to(source):
        return Path("source") / path.relative_to(source)
    return Path("deps") / path.name


def stage(
    build: Path, source: Path, output: Path, roots: list[Path], source_sha: str
) -> dict:
    build, source, output = build.resolve(), source.resolve(), output.absolute()
    if output.exists() or not re.fullmatch(r"[a-f0-9]{40}", source_sha):
        raise ValueError("require new bundle directory and exact source SHA")
    native = json.loads(
        (Path(__file__).parent / "windows-tier2/native-test-names.json").read_text()
    )
    inventory = registry.inventory(build, native)
    if inventory["unbuilt_tests"] or inventory["missing_native_tests"]:
        raise ValueError("incomplete CMake registry/build")
    products = [
        Path(line) for line in (build / "tier2-runtime.txt").read_text().splitlines()
    ]
    for path in products:
        probe.contained(build, path)
    dynamic = plugin_images(inventory["tests"], source, output)
    paths = probe.closure([*products, *dynamic], [build, *roots], hosted_runtime=True)
    for path in paths:
        name = destination_name(path, build, source)
        copy_path(path, output / name)
    stage_data(build, source, output, products)
    stage_arguments(build, source, output, inventory["tests"])
    receipts = prepare_fixture(build, source, output, inventory["tests"], source_sha)
    tests = normalize_tests(inventory["tests"], build, source)
    for test in tests:
        for prop in test["properties"]:
            if prop["name"] == "WORKING_DIRECTORY":
                cwd = (
                    prop["value"]
                    .replace(probe.BUILD, "build")
                    .replace(probe.SOURCE, "source")
                )
                (output / consumer.relative(cwd)).mkdir(parents=True, exist_ok=True)
    cores = {
        name: f"build/cores/{variant}/obs.dll"
        for name, variant in (("cmake-off", "reference"), ("cmake-on", "shared"))
    }
    exports = probe.parity(
        *(consumer.coff_exports(output / name) for name in cores.values())
    )
    files, directories = copied_inventory(output)
    images = [
        output / name for name in files if Path(name).suffix.lower() in {".exe", ".dll"}
    ]
    manifest = {
        "schema": 2,
        "target": probe.TARGET,
        "complete": False,
        "coverage": "cmake-part-only",
        "source_sha": source_sha,
        "pending": [
            "cargo parts pending",
            "independent same-commit native CTest parity",
            "hosted Windows execution",
        ],
        "files": files,
        "directories": directories,
        "tests": tests,
        "cores": cores,
        "exports": exports,
        "fixture_receipts": receipts,
        "runtime_directories": sorted(
            {
                str(Path(name).parent).replace("\\", "/")
                for name in files
                if Path(name).suffix.lower() in {".exe", ".dll"}
                and Path(name).name.lower() != "obs.dll"
            }
        ),
        "hosted_runtime": {
            "provider": "windows-2025-vs2026",
            "documentation": probe.RUNNER_DOC,
            "dlls": probe.runtime_requirements(images),
        },
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    audit(output, [str(build), str(source), str(Path.home())])
    verify(output)
    return manifest


def verify(root: Path) -> dict:
    manifest = json.loads((root / "manifest.json").read_text())
    if (
        manifest.get("schema") != 2
        or manifest.get("complete") is not False
        or manifest.get("pending", [None])[0] != "cargo parts pending"
    ):
        raise ValueError("require explicitly incomplete CMake part")
    files, directories = copied_inventory(root)
    files.pop("manifest.json")
    if directories != manifest["directories"]:
        raise ValueError("unexpected/missing runtime directories")
    if files != manifest["files"]:
        raise ValueError("changed/missing runtime files")
    consumer.verify_files(root, manifest)
    verify_tests(root.absolute(), manifest)
    return manifest


def verify_tests(root: Path, manifest: dict) -> None:
    tests = consumer.normalized(
        manifest["tests"],
        {
            probe.BUILD: (root / "build").as_posix(),
            probe.SOURCE: (root / "source").as_posix(),
        },
    )
    for test in tests:
        command = test["command"]
        if test["name"] == "test_core_module_manifest_source":
            name = manifest["fixture_receipts"][test["name"]]
            if name not in manifest["files"]:
                raise ValueError("unhashed Linux fixture receipt")
        else:
            consumer.runtime_command(command, root, {})
            name = probe.contained(root, Path(command[0])).as_posix()
            if name not in manifest["files"]:
                raise ValueError(f"unstaged CTest executable: {test['name']}")
        for prop in test.get("properties", []):
            if prop["name"] == "WORKING_DIRECTORY":
                name = probe.contained(root, Path(prop["value"])).as_posix()
                if name not in manifest["directories"]:
                    raise ValueError(
                        f"unstaged CTest working directory: {test['name']}"
                    )
    consumer.check_shadowing(root / "build", tests)


def archive(root: Path, destination: Path) -> dict:
    verify(root)
    if destination.exists():
        raise ValueError("refusing to overwrite archive")
    started = time.monotonic()
    with tarfile.open(destination, "w:zst", level=3) as output:
        for path in sorted(root.rglob("*")):
            output.add(path, arcname=path.relative_to(root).as_posix(), recursive=False)
    size = destination.stat().st_size
    if size > CEILING:
        raise ValueError(f"archive exceeds 500 MiB ceiling: {size}")
    return {
        "compressed_bytes": size,
        "target_bytes": TARGET,
        "ceiling_bytes": CEILING,
        "within_target": size <= TARGET,
        "pack_seconds": time.monotonic() - started,
        "sha256": probe.digest(destination),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", required=True, type=Path)
    parser.add_argument(
        "--source", type=Path, default=Path(__file__).resolve().parents[1]
    )
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--dll-root", action="append", type=Path, default=[])
    args = parser.parse_args()
    sha = subprocess.check_output(
        ["git", "rev-parse", "HEAD"], cwd=args.source, text=True
    ).strip()
    stage(args.build, args.source, args.output, args.dll_root, sha)
    print(json.dumps(archive(args.output, args.archive), indent=2))


if __name__ == "__main__":
    main()
