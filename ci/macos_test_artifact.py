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
import shutil
import struct
import subprocess
import sys
from pathlib import Path

from rust_scope import affected

TARGET = "aarch64-apple-darwin"
ROOT = Path(__file__).resolve().parents[1]


def within(root: Path, value: str) -> Path:
    path = root / value
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError(f"artifact path escapes root: {value}")
    return path


def macho_signature(path: Path, data: bytes) -> tuple[bytes, int]:
    magic, cpu, _, kind, commands, size, _, _ = struct.unpack_from("<8I", data)
    if (magic, cpu, kind) != (0xFEEDFACF, 0x100000C, 2):
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


def verify_macho(path: Path) -> None:
    """Verify arm64 MH_EXECUTE and its ad-hoc SHA256 CodeDirectory pages.

    ld64.lld signs arm64 by default. Inspect the load command and actual page
    hashes, rather than treating an arbitrary LC_CODE_SIGNATURE as proof.
    """
    data = path.read_bytes()
    signature, signature_start = macho_signature(path, data)
    magic, length, count = struct.unpack_from(">3I", signature)
    if magic != 0xFADE0CC0 or length != len(signature):
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


def collect(metadata: dict, events: list[dict], root: Path, bundle: Path, args: list[str]) -> dict:
    if not events or events[-1] != {"reason": "build-finished", "success": True}:
        raise ValueError("Cargo did not report a complete successful build")
    packages = {p["id"]: p for p in metadata["packages"]}
    scripts = {e["package_id"]: e for e in events if e.get("reason") == "build-script-executed"}
    tests = []

    def relative(path: str) -> str:
        return str(Path(path).relative_to(root))

    def portable(value: str) -> str:
        return value.replace(str(root), "{root}")

    def copy(path: str) -> str:
        rel = relative(path)
        source = within(root, rel)
        destination = within(bundle, rel)
        destination.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            shutil.copytree(source, destination, dirs_exist_ok=True)
        else:
            shutil.copy2(source, destination)
        return rel

    for event in events:
        if event.get("reason") != "compiler-artifact" or not event.get("executable"):
            continue
        executable = event["executable"]
        verify_macho(Path(executable))
        binary = copy(executable)
        if not event["profile"]["test"]:
            continue  # helper executable used by CARGO_BIN_EXE, still transported
        package = packages[event["package_id"]]
        cwd = relative(str(Path(package["manifest_path"]).parent))
        env = package_environment(package, event["target"]["name"], cwd, relative(package["manifest_path"]))
        script = scripts.get(event["package_id"], {})
        env.update({key: portable(value) for key, value in script.get("env", [])})
        if script.get("out_dir"):
            env["OUT_DIR"] = "{root}/" + copy(script["out_dir"])
        tests.append({"package": package["name"], "binary": binary, "cwd": cwd, "env": env, "args": args})
    result = {"version": 1, "target": TARGET, "build_root": str(root), "tests": tests}
    bundle.mkdir(parents=True, exist_ok=True)
    (bundle / "manifest.json").write_text(json.dumps(result, indent=2) + "\n")
    return result


def execute(manifest: Path, root: Path) -> int:
    data = json.loads(manifest.read_text())
    if data["version"] != 1 or data["target"] != TARGET or data["build_root"] != str(root):
        raise ValueError("artifact target/version/source root mismatch (compiled paths cannot be relocated)")
    if not data["tests"] and not data.get("scope_skipped", False):
        raise ValueError("empty test manifest without an explicit scope skip")
    for entry in data["tests"]:
        binary = within(root, entry["binary"])
        if not binary.is_file():
            raise FileNotFoundError(binary)
        cwd = within(root, entry["cwd"])
        if not cwd.is_dir():
            raise FileNotFoundError(cwd)
        binary.chmod(binary.stat().st_mode | 0o111)
        env = dict(os.environ)
        env.update({key: value.replace("{root}", str(root)) for key, value in entry["env"].items()})
        print(f"Running {entry['package']}: {binary}", flush=True)
        status = subprocess.run([str(binary), *entry["args"]], cwd=cwd, env=env).returncode
        if status:
            return status if status > 0 else 128 - status
    return 0


def build(output: Path, base: str | None) -> int:
    import local_gate

    env = local_gate.soldr_env({"SOLDR_PORCELAIN": "0"})
    meta = json.loads(subprocess.run(["soldr", "cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
                                     cwd=ROOT, env=env, capture_output=True, text=True, check=True).stdout)
    files = None if base is None else subprocess.run(
        ["git", "diff", "--name-only", base, "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True).stdout.split()
    packages = packages_for(files, meta)
    if not packages:
        output.mkdir(parents=True, exist_ok=True)
        (output / "manifest.json").write_text(json.dumps({"version": 1, "target": TARGET, "build_root": str(ROOT),
                                                        "tests": [], "scope_skipped": True}) + "\n")
        return 0
    local_gate.add_target(TARGET)
    command = ["soldr", "cargo", "test", "--no-run", "--target", TARGET, *packages, "--locked", "--message-format=json"]
    # Do not pipe Cargo through a parser: the producer exit code is authoritative.
    result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True)
    sys.stderr.write(result.stderr)
    result.check_returncode()
    collect(meta, [json.loads(line) for line in result.stdout.splitlines()], ROOT, output, [])
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    producer = sub.add_parser("build")
    producer.add_argument("--output", type=Path, required=True)
    producer.add_argument("--base")
    runner = sub.add_parser("run")
    runner.add_argument("--manifest", type=Path, required=True)
    runner.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    if args.command == "build":
        return build(args.output.resolve(), args.base)
    return execute(args.manifest, args.root.resolve())


if __name__ == "__main__":
    sys.exit(main())
