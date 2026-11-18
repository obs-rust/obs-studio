#!/usr/bin/env -S uv run --script
"""Collect Linux-linked macOS tests and run them without Cargo (#5008).

Build and execution must use the SAME absolute source and target paths:
CARGO_MANIFEST_DIR and CARGO_BIN_EXE are also compiled into the tests.
Runtime environment relocation cannot rewrite those constants. This helper
fails closed until its caller has established that source-path contract.

    uv run --no-project ci/macos_test_artifact.py build --output build_macos_tests
    uv run --no-project ci/macos_test_artifact.py run --manifest manifest.json
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shlex
import shutil
import struct
import subprocess
import sys
import uuid
from collections.abc import Callable
from pathlib import Path

from rust_scope import affected

TARGET = "aarch64-apple-darwin"
ROOT = Path(__file__).resolve().parents[1]


def within(root: Path, value: str) -> Path:
    path = root / value
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"artifact path escapes root: {value}")
    return path


def macho_signature(path: Path, data: bytes, allowed_filetypes: tuple[int, ...] = (2,)) -> tuple[bytes, int]:
    magic, cpu, _, kind, commands, size, _, _ = struct.unpack_from("<8I", data)
    if (magic, cpu) != (0xFEEDFACF, 0x100000C) or kind not in allowed_filetypes:
        raise ValueError(f"not an arm64 Mach-O executable: {path}")
    offset = 32
    for _ in range(commands):
        command, length = struct.unpack_from("<2I", data, offset)
        if length < 8 or offset + length > 32 + size:
            raise ValueError(f"invalid Mach-O load command: {path}")
        if command == 0x1D:
            start, count = struct.unpack_from("<2I", data, offset + 8)
            return data[start:start + count], start
        offset += length
    raise ValueError(f"missing arm64 code signature: {path}")


def verify_directory(path: Path, directory: bytes, data: bytes, signature_start: int) -> None:
    magic, length, _, flags, hashes, _, _, pages, limit = struct.unpack_from(">9I", directory)
    hash_size, hash_type, _, page_shift = struct.unpack_from("4B", directory, 36)
    if magic != 0xFADE0C02 or not flags & 2 or (hash_size, hash_type) != (32, 2):
        raise ValueError(f"missing ad-hoc SHA256 CodeDirectory: {path}")
    if page_shift > 20:
        raise ValueError(f"invalid signed page size: {path}")
    page_size = 1 << page_shift
    if limit != signature_start or not pages or pages != (limit + page_size - 1) // page_size:
        raise ValueError(f"invalid signed page table: {path}")
    if hashes + pages * hash_size > length or length > len(directory):
        raise ValueError(f"truncated signed page table: {path}")
    for page in range(pages):
        digest = hashlib.sha256(data[page * page_size:min((page + 1) * page_size, limit)]).digest()
        if directory[hashes + page * hash_size:hashes + (page + 1) * hash_size] != digest:
            raise ValueError(f"invalid code signature page {page}: {path}")


def verify_macho(path: Path, allowed_filetypes: tuple[int, ...] = (2,)) -> None:
    """Verify arm64 Mach-O and every ad-hoc SHA256 CodeDirectory page.

    ld64.lld signs arm64 by default. Inspect the load command and actual page
    hashes, rather than treating an arbitrary LC_CODE_SIGNATURE as proof.
    Execute-only by default; dependency callers explicitly allow dylibs/bundles.
    """
    data = path.read_bytes()
    signature, signature_start = macho_signature(path, data, allowed_filetypes)
    magic, length, count = struct.unpack_from(">3I", signature)
    # Apple's linker aligns LC_CODE_SIGNATURE with zero padding; ld64.lld
    # normally uses the exact blob size. Padding is outside the signed blob.
    if magic != 0xFADE0CC0 or length > len(signature) or any(signature[length:]):
        raise ValueError(f"invalid signature superblob: {path}")
    for index in range(count):
        slot, start = struct.unpack_from(">2I", signature, 12 + index * 8)
        if slot == 0:
            verify_directory(path, signature[start:], data, signature_start)
            return
    raise ValueError(f"missing CodeDirectory: {path}")


def packages_for(files: list[str] | None, metadata: dict) -> list[str]:
    if files is None:
        return ["--workspace"]
    crates = {
        p["name"]: (str(Path(p["manifest_path"]).parent.relative_to(ROOT)),
                    {d["name"] for d in p["dependencies"]})
        for p in metadata["packages"]
    }
    mode, names = affected(files, crates)
    if mode == "skip":
        return []
    return ["--workspace"] if mode == "workspace" else [argument for name in names for argument in ("-p", name)]


def package_environment(package: dict, target_name: str, cwd: str, manifest: str) -> dict[str, str]:
    env = {"CARGO_MANIFEST_DIR": "{root}/" + cwd,
           "CARGO_MANIFEST_PATH": "{root}/" + manifest,
           "CARGO_PKG_AUTHORS": ":".join(package["authors"]),
           "CARGO_PKG_NAME": package["name"], "CARGO_PKG_VERSION": package["version"],
           "CARGO_CRATE_NAME": target_name.replace("-", "_")}
    version, _, pre = package["version"].partition("-")
    for key, value in zip(("MAJOR", "MINOR", "PATCH"), version.split("."), strict=True):
        env["CARGO_PKG_VERSION_" + key] = value
    env["CARGO_PKG_VERSION_PRE"] = pre
    for field in ("description", "homepage", "repository", "license", "license_file", "readme", "rust_version"):
        env["CARGO_PKG_" + field.upper()] = package.get(field) or ""
    return env


def capture_doctest(binary: Path, args: list[str], root: Path, bundle: Path,
                    cwd: Path, env: dict[str, str], *, target: str = TARGET,
                    verify_executable: Callable[[Path], None] | None = None) -> dict:
    """Stable rustdoc runtool: retain the binary before rustdoc removes it.

    This acknowledges compilation only. The manifest runner must execute the
    retained harness, including its should_panic cases, on macOS. Compile-fail
    and no_run cases are checked by rustdoc on the producer as usual.
    """
    (verify_executable or verify_macho)(binary)
    identity = uuid.uuid4().hex
    windows = target == "x86_64-pc-windows-msvc"
    relative = f"target/{'windows' if windows else 'macos'}-doctests/{identity}/rust_out{'.exe' if windows else ''}"
    destination = within(bundle, relative)
    destination.parent.mkdir(parents=True)
    shutil.copy2(binary, destination)
    cargo_env = {key: value.replace(str(binary), "{root}/" + relative).replace(str(root), "{root}")
                 for key, value in env.items()
                 if key.startswith(("CARGO_PKG_", "CARGO_BIN_EXE_", "RUSTDOC_DOCTEST_"))
                 or key in ("CARGO_MANIFEST_DIR", "CARGO_MANIFEST_PATH", "CARGO_CRATE_NAME", "OUT_DIR")}
    entry = {"package": env["CARGO_PKG_NAME"], "binary": relative,
             "cwd": str(cwd.relative_to(root)), "env": cargo_env, "args": args, "kind": "doctest"}
    records = bundle / "doctest-records"
    records.mkdir(exist_ok=True)
    (records / f"{identity}.json").write_text(json.dumps(entry) + "\n")
    return entry


def doctest_environment(root: Path, bundle: Path, env: dict[str, str], *, target: str = TARGET) -> dict[str, str]:
    # soldr's Cargo target rustflags do not reach rustdoc's test linker. Ask
    # soldr for its SDK-aware driver rather than using a system clang.
    linker = subprocess.run(["soldr", "cc", "--target", target, "--print-linker"],
                            cwd=root, env=env, capture_output=True, text=True, check=True).stdout.strip()
    if not linker:
        raise ValueError(f"soldr did not provide the {target} linker")
    if target == "x86_64-pc-windows-msvc":
        from windows_cross_tools import linker_driver

        linker = linker_driver(linker, root, bundle, env)
    runtool = ["uv", "run", "--no-project", "--script", str(root / "ci/macos_test_artifact.py"), "capture-doctest"]
    flags = ["-C", f"linker={linker}", "--test-runtool", runtool[0]]
    for argument in runtool[1:]:
        flags.extend(["--test-runtool-arg", argument])
    return env | {"RUSTDOCFLAGS": shlex.join(flags), "OBS_MACOS_DOCTEST_ROOT": str(root),
                   "OBS_MACOS_DOCTEST_BUNDLE": str(bundle), "OBS_CROSS_DOCTEST_TARGET": target}


def compile_doctests(root: Path, output: Path, env: dict, packages: list[str], manifest: dict,
                     *, target: str = TARGET) -> None:
    result = subprocess.run(["soldr", "cargo", "test", "--doc", "--target", target, *packages, "--locked"],
                            cwd=root, env=doctest_environment(root, output, env, target=target))
    result.check_returncode()
    manifest["tests"].extend(json.loads(path.read_text()) for path in sorted((output / "doctest-records").glob("*.json")))
    manifest["doctests_complete"] = True


def finalize(bundle: Path, manifest: dict) -> None:
    manifest["files"] = {str(path.relative_to(bundle)): hashlib.sha256(path.read_bytes()).hexdigest()
                         for path in sorted(bundle.rglob("*")) if path.is_file() and path != bundle / "manifest.json"}
    manifest["executables"] = [name for name in manifest["files"] if os.access(bundle / name, os.X_OK)]
    (bundle / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


def copy_artifact(root: Path, bundle: Path, path: str) -> str:
    rel = str(Path(path).relative_to(root))
    source = within(root, rel)
    destination = within(bundle, rel)
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source.is_dir():
        # Validate descendants too; copytree follows file/directory symlinks.
        for child in source.rglob("*"):
            within(root, str(child.relative_to(root)))
        shutil.copytree(source, destination, dirs_exist_ok=True)
    else:
        shutil.copy2(source, destination)
    return rel


def collect_build_scripts(events: list[dict], root: Path, bundle: Path, target_directory: Path,
                          *, target: str = TARGET) -> dict:
    scripts = {}
    for event in events:
        if event.get("reason") != "build-script-executed":
            continue
        output = event.get("out_dir")
        if output:
            copy_artifact(root, bundle, output)
        # A build dependency can execute the same package's build script for
        # both hosts. Retain both outputs, but use the target's env regardless of
        # Cargo event ordering. Pure host dependencies have no target event.
        if event["package_id"] not in scripts or (output and Path(output).is_relative_to(target_directory / target)):
            scripts[event["package_id"]] = event
    return scripts


def collect(metadata: dict, events: list[dict], root: Path, bundle: Path, args: list[str],
            *, target: str = TARGET, verify_executable: Callable[[Path], None] | None = None) -> dict:
    if not events or events[-1] != {"reason": "build-finished", "success": True}:
        raise ValueError("Cargo did not report a complete successful build")
    packages = {p["id"]: p for p in metadata["packages"]}
    scripts = collect_build_scripts(events, root, bundle, Path(metadata.get("target_directory", root / "target")), target=target)
    tests = []

    def relative(path: str) -> str:
        return str(Path(path).relative_to(root))

    def portable(value: str) -> str:
        return value.replace(str(root), "{root}")

    for event in events:
        if event.get("reason") != "compiler-artifact" or not event.get("executable"):
            continue
        executable = event["executable"]
        (verify_executable or verify_macho)(Path(executable))
        binary = copy_artifact(root, bundle, executable)
        if not event["profile"]["test"]:
            continue  # helper executable used by CARGO_BIN_EXE, still transported
        package = packages[event["package_id"]]
        cwd = relative(str(Path(package["manifest_path"]).parent))
        env = package_environment(package, event["target"]["name"], cwd, relative(package["manifest_path"]))
        script = scripts.get(event["package_id"], {})
        env.update({key: portable(value) for key, value in script.get("env", [])})
        if script.get("out_dir"):
            env["OUT_DIR"] = "{root}/" + relative(script["out_dir"])
        tests.append({"package": package["name"], "binary": binary, "cwd": cwd, "env": env, "args": args})
    result = {"version": 1, "target": target, "build_root": str(root), "tests": tests}
    bundle.mkdir(parents=True, exist_ok=True)
    (bundle / "manifest.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def check_test_inventory(metadata: dict, events: list[dict], packages: list[str]) -> bool:
    """Match Cargo's successful result to every selected default test target."""
    selected = set(metadata["workspace_members"]) if packages == ["--workspace"] else {
        package["id"] for package in metadata["packages"] if package["name"] in packages[1::2]}
    expected = {(package["id"], target["name"]) for package in metadata["packages"] if package["id"] in selected
                for target in package["targets"] if target["test"] and "bench" not in target["kind"]}
    actual = {(event["package_id"], event["target"]["name"]) for event in events
              if event.get("reason") == "compiler-artifact" and event["profile"]["test"] and event.get("executable")}
    if actual != expected:
        raise ValueError(f"test target inventory mismatch: missing={sorted(expected - actual)}, extra={sorted(actual - expected)}")
    return bool(expected)


def validate_native_fixtures(root: Path) -> None:
    from macos_native_fixtures import TESTS

    index = json.loads((root / "native-fixtures/index.json").read_text())
    expected = ["native_build::" + name for name in TESTS]
    if (index.get("version") != 1 or index.get("target") != TARGET
            or index.get("complete") is not True or index.get("tests") != expected or not index.get("signed")):
        raise ValueError("native fixture inventory is incomplete")
    for name in expected:
        manifest = json.loads((root / "native-fixtures" / name.replace("::", "--") / "manifest.json").read_text())
        if (manifest.get("complete") is not True or manifest.get("target") != TARGET
                or "linux" not in manifest.get("host", "") or not manifest.get("events")):
            raise ValueError(f"missing native fixture evidence: {name}")
    for name in index["signed"]:
        verify_macho(within(root / "native-fixtures", name), allowed_filetypes=(2, 6, 8))


def validate_inventory(data: dict, root: Path) -> None:
    if not data.get("doctests_complete"):
        raise ValueError("doctest compilation was not completed")
    validate_files(data, root)


def validate_files(data: dict, root: Path) -> None:
    """Shared checksummed transport; coverage is checked by each lane's verifier."""
    for name, digest in data["files"].items():
        path = within(root, name)
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError(f"artifact checksum mismatch: {path}")
    for name in data.get("executables", []):
        path = within(root, name)
        if name not in data["files"]:
            raise ValueError(f"executable missing from artifact inventory: {path}")
        path.chmod(path.stat().st_mode | 0o111)
    if data.get("native_fixtures") and data["target"] == TARGET:
        validate_native_fixtures(root)


def validate_qt_inventory(data: dict) -> None:
    from macos_qt_artifact import CONTRACTS

    actual = {entry["package"].removeprefix("Qt/") for entry in data["tests"] if entry.get("kind") == "qt-contract"}
    if data.get("qt_contracts_complete") != list(CONTRACTS) or actual != set(CONTRACTS):
        raise ValueError("required Qt contract payload is incomplete")


def entry_environment(entry: dict, root: Path) -> dict[str, str]:
    env = dict(os.environ)
    env.update({key: value.replace("{root}", str(root)) for key, value in entry["env"].items()})
    for key, values in entry.get("env_path_prepend", {}).items():
        paths = [value.replace("{root}", str(root)) for value in values]
        if env.get(key):
            paths.append(env[key])
        env[key] = os.pathsep.join(paths)
    return env


def execute(manifest: Path, root: Path, *, require_qt: bool = False, target: str = TARGET,
            validate: Callable[[dict, Path], None] | None = None,
            kinds: set[str] | None = None,
            root_validator: Callable[[str, Path], None] | None = None) -> int:
    data = json.loads(manifest.read_text())
    if data["version"] != 1 or data["target"] != target or (root_validator is None and data["build_root"] != str(root)):
        raise ValueError("artifact target/version/source root mismatch (compiled paths cannot be relocated)")
    if root_validator is not None:
        root_validator(data["build_root"], root)
    if not data["tests"] and not (data.get("scope_skipped", False) or data.get("scope_no_tests", False)):
        raise ValueError("empty test manifest without an explicit scope skip")
    if require_qt:
        validate_qt_inventory(data)
    (validate or validate_inventory)(data, root)
    for entry in data["tests"]:
        if kinds is not None and entry.get("kind") not in kinds:
            continue
        binary = within(root, entry["binary"])
        if entry["binary"] not in data["files"]:
            raise ValueError(f"test missing from artifact inventory: {binary}")
        if not binary.is_file():
            raise FileNotFoundError(binary)
        cwd = within(root, entry["cwd"])
        if not cwd.is_dir():
            raise FileNotFoundError(cwd)
        binary.chmod(binary.stat().st_mode | 0o111)
        env = entry_environment(entry, root)
        print(f"Running {entry['package']}: {binary}", flush=True)
        arguments = [value.replace("{root}", str(root)) for value in entry["args"]]
        status = subprocess.run([str(binary), *arguments], cwd=cwd, env=env, timeout=entry.get("timeout")).returncode
        if status:
            return status if status > 0 else 128 - status
    return 0


def build(output: Path, base: str | None) -> int:
    import local_gate

    if output.exists() and any(output.iterdir()):
        raise ValueError("build output must be empty (stale artifacts cannot prove coverage)")
    env = local_gate.soldr_env({"SOLDR_PORCELAIN": "0", "SOLDR_VERDICTS": "off"})
    meta = json.loads(subprocess.run(["soldr", "cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
                                     cwd=ROOT, env=env, capture_output=True, text=True, check=True).stdout)
    files = None if base is None else subprocess.run(
        ["git", "diff", "--name-only", base, "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    packages = packages_for(files, meta)
    if not packages:
        output.mkdir(parents=True, exist_ok=True)
        finalize(output, {"version": 1, "target": TARGET, "build_root": str(ROOT),
                          "tests": [], "scope_skipped": True, "doctests_complete": True})
        return 0
    local_gate.add_target(TARGET)
    command = ["soldr", "cargo", "test", "--no-run", "--target", TARGET, *packages, "--locked", "--message-format=json"]
    # Do not pipe Cargo through a parser: the producer exit code is authoritative.
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
    sys.stderr.write(result.stderr)
    result.check_returncode()
    events = [json.loads(line) for line in result.stdout.splitlines()]
    has_tests = check_test_inventory(meta, events, packages)
    manifest = collect(meta, events, ROOT, output, [])
    manifest["scope_no_tests"] = not has_tests
    # Stable cross doctests use rustdoc's runtool to capture merged 2024
    # harnesses before temporary cleanup. A standalone should_panic case that
    # cannot be merged fails rustdoc rather than silently losing its assertion.
    compile_doctests(ROOT, output, env, packages, manifest)
    if any(entry["package"] == "obs-studio-rust" for entry in manifest["tests"]):
        from macos_native_fixtures import prepare

        prepare(ROOT, output, env)
        manifest["native_fixtures"] = True
        for entry in manifest["tests"]:
            if entry["package"] == "obs-studio-rust":
                entry["env"]["OBS_NATIVE_FIXTURES"] = "{root}/native-fixtures"
    finalize(output, manifest)
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("prepare-target")
    producer = sub.add_parser("build")
    producer.add_argument("--output", type=Path, required=True)
    producer.add_argument("--base")
    runner = sub.add_parser("run")
    runner.add_argument("--manifest", type=Path, required=True)
    runner.add_argument("--root", type=Path, default=ROOT)
    runner.add_argument("--require-qt", action="store_true")
    capture = sub.add_parser("capture-doctest")
    capture.add_argument("binary", type=Path)
    capture.add_argument("args", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command == "prepare-target":
        import local_gate

        local_gate.add_target(TARGET)
        return 0
    if args.command == "build":
        return build(args.output.resolve(), args.base)
    if args.command == "capture-doctest":
        target = os.environ.get("OBS_CROSS_DOCTEST_TARGET", TARGET)
        verifier = None
        if target == "x86_64-pc-windows-msvc":
            from windows_test_artifact import verify_pe

            verifier = verify_pe
        elif target != TARGET:
            raise ValueError(f"unsupported cross-doctest target: {target}")
        capture_doctest(args.binary, args.args, Path(os.environ["OBS_MACOS_DOCTEST_ROOT"]),
                        Path(os.environ["OBS_MACOS_DOCTEST_BUNDLE"]), Path.cwd(), dict(os.environ),
                        target=target, verify_executable=verifier)
        return 0
    return execute(args.manifest, args.root.resolve(), require_qt=args.require_qt)


if __name__ == "__main__":
    sys.exit(main())
