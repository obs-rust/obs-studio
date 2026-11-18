#!/usr/bin/env -S uv run --script
"""Verify and execute an immutable Linux-produced Windows Tier 2 runtime tree.

Schema 2 is deliberately stricter than the four-test probe. Incomplete trees
cannot run. No compiler, LLVM installation, Cargo, or CMake build is invoked.
"""

from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import struct
import subprocess
import tarfile

import windows_tier2_probe as probe
from windows_zip_check import _off, _cstr

PASSES = ("cmake-off", "cmake-on", "cargo-off", "cargo-on")
COMPILERS = frozenset(
    "cl clang clang-cl gcc cc c++ g++ rustc cargo soldr ninja msbuild link lld-link".split()
)


def artifact_identity(identity: str, sha256: str) -> int:
    if not re.fullmatch(r"[1-9][0-9]*", identity) or not re.fullmatch(
        r"[a-f0-9]{64}", sha256
    ):
        raise ValueError("require immutable numeric artifact ID and lowercase SHA256")
    return int(identity)


def relative(value: str) -> Path:
    path = PurePosixPath(value)
    if (
        not value
        or path.is_absolute()
        or any(x in value for x in ("\\", ":", "\x00"))
        or ".." in path.parts
    ):
        raise ValueError(f"invalid bundle path: {value}")
    return Path(*path.parts)


def extract(archive: Path, output: Path) -> None:
    """Validate every tar member before extracting (Python 3.14 supports zstd)."""
    if output.exists():
        raise ValueError("refusing to overwrite extracted tree")
    with tarfile.open(archive, "r:*") as source:
        members = source.getmembers()
        names = set()
        for member in members:
            relative(member.name)
            if (
                not (member.isfile() or member.isdir())
                or member.name.casefold() in names
            ):
                raise ValueError(f"unsafe/duplicate archive entry: {member.name}")
            names.add(member.name.casefold())
        source.extractall(output, members=members, filter="data")


def coff_exports(path: Path) -> set[str]:
    """Read named PE exports directly; the consumer needs no dumpbin/LLVM."""
    data = path.read_bytes()
    probe.image(data)
    header = struct.unpack_from("<I", data, 0x3C)[0]
    rva, size = struct.unpack_from("<II", data, header + 24 + 112)
    if not rva or not size:
        raise ValueError(f"empty exports: {path}")
    directory = _off(data, header, rva)
    count = struct.unpack_from("<I", data, directory + 24)[0]
    table = struct.unpack_from("<I", data, directory + 32)[0]
    if count == 0 or count > len(data) // 4:
        raise ValueError(f"invalid export count: {path}")
    start = _off(data, header, table)
    result = set()
    for index in range(count):
        address = struct.unpack_from("<I", data, start + 4 * index)[0]
        result.add(_cstr(data, _off(data, header, address)))
    if len(result) != count or "" in result:
        raise ValueError(f"invalid export names: {path}")
    return result


def normalized(tests: list[dict], replacements: dict[str, str]) -> list[dict]:
    def replace(value):
        if isinstance(value, str):
            for old, new in replacements.items():
                value = value.replace(old, new)
            return value
        if isinstance(value, list):
            return [replace(item) for item in value]
        if isinstance(value, dict):
            return {key: replace(item) for key, item in value.items()}
        return value

    result = []
    for test in tests:
        result.append(
            {
                "name": test["name"],
                "command": replace(test["command"]),
                "properties": sorted(
                    replace(test.get("properties", [])), key=lambda prop: prop["name"]
                ),
            }
        )
    return sorted(result, key=lambda test: test["name"])


def compare_inventory(expected: list[dict], actual: list[dict]) -> None:
    for tests in (expected, actual):
        if not tests or len({test["name"] for test in tests}) != len(tests):
            raise ValueError("empty/duplicate native inventory")
    if normalized(expected, {}) != normalized(actual, {}):
        wanted, found = {t["name"] for t in expected}, {t["name"] for t in actual}
        raise ValueError(
            f"native inventory differs: missing={sorted(wanted - found)}, extra={sorted(found - wanted)}; commands/properties must match"
        )


def runtime_command(command: list[str], root: Path, manifest: dict) -> None:
    """A full receipt must replace runtime-build fixtures with prepared executables."""
    executable = Path(command[0])
    if command in manifest.get("prepared_fixture_commands", []):
        return
    if (
        executable.suffix.lower() != ".exe"
        or executable.stem.lower() in COMPILERS
        or not executable.resolve().is_relative_to(root.resolve())
    ):
        raise ValueError(
            f"unsupported runtime command (needs Linux preparation): {command}"
        )
    probe.contained(root, executable)


def shadowing_properties(test: dict) -> list[Path]:
    directories = []
    for prop in test.get("properties", []):
        name, value = prop["name"], prop["value"]
        if name == "WORKING_DIRECTORY":
            directories.append(Path(value))
        elif name == "ENVIRONMENT" and any(
            x.upper().startswith("PATH=") for x in value
        ):
            raise ValueError(f"test {test['name']} overrides PATH")
        elif name == "ENVIRONMENT_MODIFICATION":
            for item in value:
                if not item.upper().startswith("PATH="):
                    continue
                if item.startswith(
                    ("PATH=path_list_prepend:", "PATH=path_list_append:")
                ):
                    directories.extend(
                        Path(x) for x in item.split(":", 1)[1].split(";")
                    )
                else:
                    raise ValueError(f"test {test['name']} changes PATH unexpectedly")
    return directories


def check_shadowing(root: Path, tests: list[dict]) -> None:
    directories = [root]
    for test in tests:
        directories.append(Path(test["command"][0]).parent)
        directories.extend(shadowing_properties(test))
    for directory in directories:
        if directory.exists() and any(
            p.name.lower() == "obs.dll" for p in directory.iterdir()
        ):
            raise ValueError(f"shadowing obs.dll in {directory}")


def verify(root: Path, source_sha: str, source: Path | None = None) -> dict:
    manifest = json.loads((root / "manifest.json").read_text())
    if manifest.get("schema") != 2 or manifest.get("complete") is not True:
        raise ValueError("require complete Windows Tier 2 schema 2 bundle")
    if manifest.get("source_sha") != source_sha or not re.fullmatch(
        r"[a-f0-9]{40}", source_sha
    ):
        raise ValueError("bundle source SHA differs from checkout")
    if (
        manifest.get("target") != probe.TARGET
        or manifest.get("coverage") != "native-full"
    ):
        raise ValueError("require full AMD64 native coverage")
    verify_files(root, manifest)
    verify_cores(root, manifest)
    verify_contract(manifest, source_sha)
    verify_renderer_exports(root, manifest)
    return prepare_ctest(root, manifest, source)


def verify_files(root: Path, manifest: dict) -> None:
    files = manifest["files"]
    actual = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file()}
    if actual != set(files) | {"manifest.json"}:
        raise ValueError("unexpected/missing bundle files")
    paths = []
    for name, expected in files.items():
        path = root / relative(name)
        probe.contained(root, path)
        if path.is_symlink() or probe.digest(path) != expected:
            raise ValueError(f"changed artifact: {name}")
        if path.suffix.lower() in (".exe", ".dll"):
            if path.stem.lower() in COMPILERS:
                raise ValueError(f"compiler in runtime payload: {name}")
            probe.image(path.read_bytes())
            paths.append(path)
    probe.check_runtime_contract(manifest, paths, None)


def verify_cores(root: Path, manifest: dict) -> None:
    files = manifest["files"]
    cores = manifest["cores"]
    if set(cores) != set(PASSES) or len(set(cores.values())) != 4:
        raise ValueError("require four isolated CMake/Cargo cores")
    symbols = None
    for name in PASSES:
        core = root / relative(cores[name])
        if cores[name] not in files or core.name.lower() != "obs.dll":
            raise ValueError("unhashed core")
        exports = coff_exports(core)
        symbols = exports if symbols is None else set(probe.parity(symbols, exports))
    if sorted(symbols) != manifest["exports"]:
        raise ValueError("transported exports differ from producer")


def verify_contract(manifest: dict, source_sha: str) -> None:
    files = manifest["files"]
    baseline = manifest["native_baseline"]
    if (
        baseline["source_sha"] != source_sha
        or baseline.get("clean_configure") is not True
    ):
        raise ValueError("require independent same-commit native configure provenance")
    compare_inventory(baseline["tests"], manifest["tests"])
    contract = manifest["native_contract"]
    if contract.get("bundled_obs_sys", {}).get(
        "source_sha"
    ) != source_sha or not contract["bundled_obs_sys"].get("passed"):
        raise ValueError("missing independent Linux bundled obs-sys check")
    for key in ("loader_probe", "graphics_probe"):
        if contract[key] not in files:
            raise ValueError("unhashed native contract probe")
    for owner in ("cmake", "cargo"):
        if set(contract["renderers"][owner]) != {"D3D11", "OPENGL"}:
            raise ValueError("missing native renderer")
        for name in contract["renderers"][owner].values():
            if name not in files:
                raise ValueError("unhashed renderer")


def verify_renderer_exports(root: Path, manifest: dict) -> None:
    renderers = manifest["native_contract"]["renderers"]
    for renderer in ("D3D11", "OPENGL"):
        reference = coff_exports(root / relative(renderers["cmake"][renderer]))
        cargo = coff_exports(root / relative(renderers["cargo"][renderer]))
        if not {"device_create", "device_destroy"} <= cargo:
            raise ValueError(f"{renderer} lacks required device API exports")
        probe.parity(reference, cargo)


def prepare_ctest(root: Path, manifest: dict, source: Path | None) -> dict:
    for directory in manifest["runtime_directories"]:
        relative(directory)
        if any((root / directory).glob("[oO][bB][sS].[dD][lL][lL]")):
            raise ValueError("runtime directory shadows selected core")
    source = (source or root / "source").absolute()
    replacements = {
        probe.BUILD: (root / "build").as_posix(),
        probe.SOURCE: source.as_posix(),
    }
    tests = normalized(manifest["tests"], replacements)
    prepare_fixtures(root, source, manifest, tests)
    for test in tests:
        runtime_command(test["command"], root, manifest)
    check_shadowing(root / "build", tests)
    write_registry(root / "build", tests)
    output = subprocess.check_output(
        [
            "ctest",
            "--test-dir",
            str(root / "build"),
            "-C",
            "RelWithDebInfo",
            "--show-only=json-v1",
        ],
        text=True,
    )
    actual_tests = json.loads(output)["tests"]
    compare_inventory(tests, actual_tests)
    manifest["tests"] = actual_tests
    return manifest


def prepare_fixtures(
    root: Path, source: Path, manifest: dict, tests: list[dict]
) -> None:
    manifest["prepared_fixture_commands"] = []
    receipts = manifest.get("fixture_receipts", {})
    if set(receipts) - {"test_core_module_manifest_source"}:
        raise ValueError("unsupported runtime-build fixture")
    for test in tests:
        if test["name"] not in receipts:
            continue
        if Path(test["command"][-1]).name != "check_core_module_manifest_source.cmake":
            raise ValueError("unexpected original fixture command")
        name = receipts[test["name"]]
        if name not in manifest["files"]:
            raise ValueError("unhashed Linux fixture receipt")
        command = [
            shutil.which("cmake") or "cmake",
            f"-DRECEIPT={root / relative(name)}",
            f"-DEXPECTED_SOURCE_SHA={manifest['source_sha']}",
            f"-DSOURCE_ROOT={source}",
            "-P",
            str(source / "ci/windows_tier2_assert_fixture.cmake"),
        ]
        manifest["prepared_fixture_commands"].append(command)
        test["command"] = command


def write_registry(build: Path, tests: list[dict]) -> None:
    """Flatten directory registration while retaining every CTest property."""

    def quote(value):
        value = str(value)
        delimiter = "="
        while f"]{delimiter}]" in value:
            delimiter += "="
        return f"[{delimiter}[{value}]{delimiter}]"

    def property_value(value):
        if isinstance(value, list):
            return ";".join(str(item).replace(";", r"\;") for item in value)
        if isinstance(value, bool):
            return "TRUE" if value else "FALSE"
        if isinstance(value, (str, int, float)):
            return str(value)
        raise ValueError(f"unsupported CTest property: {value}")

    lines = []
    for test in tests:
        lines.append(
            "add_test("
            + " ".join(quote(x) for x in [test["name"], *test["command"]])
            + ")"
        )
        for prop in test.get("properties", []):
            lines.append(
                f"set_tests_properties({quote(test['name'])} PROPERTIES {quote(prop['name'])} {quote(property_value(prop['value']))})"
            )
    build.mkdir(parents=True, exist_ok=True)
    (build / "CTestTestfile.cmake").write_text("\n".join(lines) + "\n")


def native_controls(root: Path, manifest: dict, selected: str, env: dict) -> None:
    if not selected.startswith("cargo-"):
        return
    contract = manifest["native_contract"]

    def execute(key, args, expected=0):
        command = [str(root / relative(contract[key])), *args]
        status = subprocess.run(
            command, env=env, cwd=root / "build", check=False
        ).returncode
        if status != expected:
            raise RuntimeError(f"native {key} expected {expected}, got {status}")

    execute("loader_probe", [str(root / manifest["cores"][selected])])
    execute("loader_probe", [str(root / manifest["cores"]["cmake-off"])], 14)
    for renderer, wrong in (("D3D11", "OPENGL"), ("OPENGL", "D3D11")):
        cargo = str(root / relative(contract["renderers"]["cargo"][renderer]))
        execute("graphics_probe", [cargo, renderer])
        execute("graphics_probe", [cargo, wrong], 14)
        execute(
            "graphics_probe",
            [str(root / relative(contract["renderers"]["cmake"][renderer])), renderer],
        )


def crash_stacks(root: Path, env: dict) -> None:
    """Retain cdb crash diagnostics without introducing build tools into PATH."""
    cdb = (
        Path(os.environ.get("ProgramFiles(x86)", ""))
        / "Windows Kits/10/Debuggers/x64/cdb.exe"
    )
    failed = root / "build/Testing/Temporary/LastTestsFailed.log"
    if not cdb.is_file() or not failed.is_file():
        return
    for line in failed.read_text().splitlines():
        name = line.split(":", 1)[-1]
        matches = list(root.rglob(name + ".exe"))
        if matches:
            dumps = sorted(
                (root / "build/crash-dumps").glob(name + "*.dmp"),
                key=lambda path: path.stat().st_mtime,
                reverse=True,
            )
            args = ["-z", str(dumps[0])] if dumps else ["-g", "-G", str(matches[0])]
            result = subprocess.run(
                [str(cdb), "-lines", "-c", ".ecxr;kn 40;q", *args],
                env=env,
                capture_output=True,
                text=True,
                timeout=120,
                check=False,
            )
            print((result.stdout + result.stderr)[-16000:])


@contextmanager
def crash_dumps(root: Path):
    """Keep the existing validator's original-process WER dump policy."""
    import winreg

    key = r"Software\Microsoft\Windows\Windows Error Reporting\LocalDumps\test_scene_canvas_characterization.exe"
    created = False
    try:
        try:
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
                pass
        except FileNotFoundError:
            directory = root / "build/crash-dumps"
            directory.mkdir(parents=True, exist_ok=True)
            with winreg.CreateKey(winreg.HKEY_CURRENT_USER, key) as handle:
                created = True
                winreg.SetValueEx(
                    handle, "DumpFolder", 0, winreg.REG_EXPAND_SZ, str(directory)
                )
                winreg.SetValueEx(handle, "DumpType", 0, winreg.REG_DWORD, 2)
                winreg.SetValueEx(handle, "DumpCount", 0, winreg.REG_DWORD, 1)
        yield
    finally:
        if created:
            winreg.DeleteKey(winreg.HKEY_CURRENT_USER, key)


def run_passes(root: Path, manifest: dict, ctest: str, environ: dict) -> None:
    jobs = environ.get("OBS_CTEST_JOBS", "1")
    if not jobs.isdecimal() or int(jobs) < 1:
        raise ValueError("OBS_CTEST_JOBS must be positive")
    check_shadowing(root / "build", manifest["tests"])
    for selected in PASSES:
        check_shadowing(root / "build", manifest["tests"])
        env = dict(environ)
        env["PATH"] = os.pathsep.join(
            [
                str(root / manifest["cores"][selected]).rsplit(os.sep, 1)[0],
                *(str(root / relative(x)) for x in manifest["runtime_directories"]),
                env.get("PATH", ""),
            ]
        )
        native_controls(root, manifest, selected, env)
        print(f"== {selected}: complete unchanged CTest suite", flush=True)
        command = [
            ctest,
            "--test-dir",
            str(root / "build"),
            "-C",
            "RelWithDebInfo",
            "--output-on-failure",
            "--no-tests=error",
            "-j",
            jobs,
        ]
        result = subprocess.run(command, env=env, cwd=root / "build", check=False)
        if result.returncode:
            crash_stacks(root, env)
            raise RuntimeError(f"{selected} CTest failed ({result.returncode})")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifact-id", required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--source", type=Path)
    args = parser.parse_args()
    artifact_identity(args.artifact_id, args.sha256)
    if probe.digest(args.archive) != args.sha256:
        raise ValueError("archive SHA256 mismatch")
    extract(args.archive, args.output)
    manifest = verify(args.output.absolute(), args.source_sha, args.source)
    if os.name != "nt":
        raise ValueError(
            "execution requires hosted Windows; Linux inspection is not execution proof"
        )
    ctest = shutil.which("ctest")
    if not ctest:
        raise ValueError("CTest is required")
    # Only runtime directories enter PATH. Compiler toolsets on the hosted image
    # are neither provisioned nor reachable by name in the test environment.
    system = Path(os.environ["SystemRoot"])
    env = dict(os.environ)
    env["PATH"] = os.pathsep.join([str(system / "System32"), str(system)])
    for name in COMPILERS:
        if shutil.which(name, path=env["PATH"]):
            raise ValueError(f"compiler unexpectedly available: {name}")
    with crash_dumps(args.output.absolute()):
        run_passes(args.output.absolute(), manifest, ctest, env)


if __name__ == "__main__":
    main()
