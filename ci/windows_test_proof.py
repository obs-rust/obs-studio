#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Opt-in Windows run-only qualification; never admits a required CI check.

Linux places and builds the checkout at a drive-less logical root. Windows
places the exact source at that path on C:, and executes from C:. Rust's
compiled rooted paths keep their bytes; only the runtime path boundary adds C:.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import tarfile
import time
from collections import Counter
from pathlib import Path, PurePosixPath, PureWindowsPath

import macos_test_artifact as shared
import macos_test_workspace as transport
import windows_test_artifact as windows

ROOT_PATTERN = re.compile(r"/opt/obs-win-[1-9][0-9]*-[1-9][0-9]*/src\Z")
CATEGORIES = {"obs-util", "obs-data", "obs-graphics", "obs-media-io", "obs-nvenc",
              "obs-gpu-status", "obs-preflight", "obs-studio-rust"}


def windows_root(logical: str) -> str:
    if not ROOT_PATTERN.fullmatch(logical):
        raise ValueError("invalid compiled logical root")
    return "C:" + logical


def check_root(logical: str, real: str, cwd: str) -> None:
    expected = PureWindowsPath(windows_root(logical))
    if PureWindowsPath(real) != expected or PureWindowsPath(cwd).drive != "C:":
        raise ValueError("compiled source root or current drive mismatch")


def validate_runtime_root(logical: str, root: Path) -> None:
    if platform.system() != "Windows":
        raise ValueError("proof execution requires native Windows")
    check_root(logical, str(root.resolve()), str(Path.cwd()))


def execution_environment(root: str, inherited: dict[str, str]) -> dict[str, str]:
    # Hosted checkout/TEMP normally live on D:. Rooted embedded /opt paths
    # resolve against the child's current drive, including CTest re-execs.
    # Keep every tempfile on the explicitly owned C: drive as well.
    return dict(inherited, TEMP=root + "/target/windows-proof-temp", TMP=root + "/target/windows-proof-temp")


def relative_path(name: str) -> None:
    path = PurePosixPath(name)
    parts = name.split("/")
    reserved = {"CON", "PRN", "AUX", "NUL", *(f"COM{i}" for i in range(1, 10)), *(f"LPT{i}" for i in range(1, 10))}
    if (not name or path.is_absolute() or any(p in ("", ".", "..") for p in parts)
            or any("\\" in p or ":" in p or p.endswith((".", " ")) or p.split(".")[0].upper() in reserved for p in parts)):
        raise ValueError(f"nonportable Windows artifact path: {name}")


def check_names(names: list[str]) -> None:
    for name in names:
        relative_path(name)
    if len({name.casefold() for name in names}) != len(names):
        raise ValueError("case-colliding Windows artifact inventory")


def source_inventory(root: Path) -> dict[str, str]:
    """Hash actual tracked source bytes, beyond Git's newline-normalized diff."""
    paths = subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True).stdout
    return {name: transport.checksum(root / name) for name in paths.decode().split("\0")
            if name and (root / name).is_file()}


def git(root: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout.strip()


def check_receipt(receipt: dict, expected: dict) -> None:
    for key, value in expected.items():
        if receipt.get(key) != value:
            raise ValueError(f"producer receipt mismatch: {key}")


def seal(bundle: Path, root: Path, run_id: str, attempt: str) -> None:
    data = windows.verify(bundle / "manifest.json")
    windows_root(str(root))
    if data["build_root"] != str(root):
        raise ValueError("compiled root mismatch at publication")
    receipt = {"run_id": run_id, "run_attempt": attempt, "source_sha": git(root, "rev-parse", "HEAD"),
               "source_tree": git(root, "rev-parse", "HEAD^{tree}"), "logical_root": str(root),
               "sources": source_inventory(root)}
    if str(root) != f"/opt/obs-win-{run_id}-{attempt}/src":
        raise ValueError("root does not belong to producer run/attempt")
    git(root, "diff", "--exit-code", "HEAD", "--")
    (bundle / "producer.json").write_text(json.dumps(receipt, indent=2) + "\n")
    shared.finalize(bundle, data)
    verify_complete(data)
    verify_payload(bundle, require_qt=True)


def verify_complete(data: dict) -> None:
    from windows_qt_artifact import CONTRACTS

    categories = [e["package"] for e in data["tests"] if e.get("kind") not in ("qt-contract", "doctest")]
    if len(categories) != 8 or set(categories) != CATEGORIES or data.get("coverage") != windows.PREPARED_COVERAGE:
        raise ValueError("proof requires all eight categories and full preparation")
    tests = {e["binary"] for e in data["tests"]}
    helpers = {name for name in set(data["images"]) - tests if not name.startswith("target/windows-qt/")}
    if len(helpers) != 3:
        raise ValueError("proof requires all three Cargo helpers")
    observations = Counter(e["package"].removeprefix("Qt/") for e in data["tests"] if e.get("kind") == "qt-contract")
    if observations != dict(zip(CONTRACTS, (3, 11, 4, 30), strict=True)):
        raise ValueError("proof requires all 48 original Windows Qt observations")


def native_destination(name: str, logical: str, roots: list[str]) -> None:
    if not name.startswith(logical + "/target/cross-native-fixture-work/"):
        raise ValueError("native fixture compiled root mismatch")
    relative_path(name.removeprefix(logical + "/"))
    if not any(PurePosixPath(name).is_relative_to(PurePosixPath(root)) for root in roots):
        raise ValueError("native fixture destination escapes recorded roots")


def native_receipts(data: dict, bundle: Path) -> None:
    from windows_native_fixtures import TESTS

    logical = data["build_root"]
    windows_root(logical)
    index = json.loads((bundle / "native-fixtures/index.json").read_text())
    if len(index["images"]) != 55:
        raise ValueError("native history requires all 55 PE snapshots")
    events = 0
    for name in TESTS:
        history = json.loads((bundle / "native-fixtures" / ("native_build::" + name).replace("::", "--") / "manifest.json").read_text())
        if type(history.get("version")) is not int or history["version"] != 1:
            raise ValueError("native history version mismatch")
        roots = history["directories"]
        for root in roots:
            native_destination(root, logical, roots)
        for event in history["events"]:
            events += 1
            for directory in event["directories"]:
                native_destination(directory, logical, roots)
            for entry in event["files"]:
                relative_path(entry["file"])
                native_destination(entry["destination"], logical, roots)
                blob = data["files"].get("native-fixtures/" + entry["file"])
                if blob is None or entry.get("sha256") != blob:
                    raise ValueError("native snapshot digest does not match verified blob")
    if events != 154:
        raise ValueError("native history requires all 154 original events")


def verify_payload(bundle: Path, *, require_qt: bool) -> dict:
    manifest = bundle / "manifest.json"
    data = json.loads(manifest.read_text())
    check_names(list(data["files"]))
    actual = [p.relative_to(bundle).as_posix() for p in bundle.rglob("*") if p.is_file() and p != manifest]
    if any(p.is_symlink() for p in bundle.rglob("*")) or set(actual) != set(data["files"]):
        raise ValueError("artifact full file inventory mismatch")
    # Checksums first, then native histories, every PE/import and Qt closure.
    shared.validate_files(data, bundle)
    windows.verify(manifest)
    if data.get("native_fixtures"):
        native_receipts(data, bundle)
    if require_qt:
        shared.validate_qt_inventory(data)
    for entry in data["tests"]:
        relative_path(entry["binary"])
        if entry["cwd"] != ".":
            relative_path(entry["cwd"])
    return data


def extract(packet: Path, stage: Path, digest: str) -> None:
    if not re.fullmatch(r"[a-f0-9]{64}", digest) or transport.checksum(packet) != digest:
        raise ValueError("immutable archive checksum mismatch")
    if stage.exists():
        raise ValueError("proof extraction requires absent staging directory")
    stage.mkdir(parents=True)
    with tarfile.open(packet) as tar:
        members = tar.getmembers()
        names = []
        for member in members:
            name = member.name.removeprefix("./")
            if name == "." and member.isdir():
                continue
            relative_path(name)
            if not (member.isfile() or member.isdir()):
                raise ValueError("proof archive contains a link or special file")
            names.append(name)
        check_names(names)
        tar.extractall(stage, members=members, filter="data")


def run_verified(manifest: Path, root: Path) -> int:
    return shared.execute(manifest, root, target=windows.TARGET, require_qt=True,
        validate=lambda data, directory: windows.verify(directory / "manifest.json"),
        root_validator=validate_runtime_root)


def qualify_compiled_root(data: dict, root: Path) -> None:
    """Run the embedded-constant probe, then prove runtime env cannot relocate it."""
    app = next(e for e in data["tests"] if e["package"] == "obs-studio-rust")
    env = shared.entry_environment(app, root)
    env["OBS_WINDOWS_PROOF_ROOT"] = data["build_root"]
    command = [str(shared.within(root, app["binary"])), "--exact", "windows_paths::compiled_paths_match_receipt", "--nocapture"]
    for wrong in (False, True):
        env["OBS_WINDOWS_PROOF_ROOT"] = data["build_root"] + ("-wrong" if wrong else "")
        result = subprocess.run(command, cwd=root, env=env, capture_output=True, text=True, timeout=120)
        print(result.stdout + result.stderr, flush=True)
        if ((not wrong and result.returncode) or (wrong and (not result.returncode
                or "compiled root mismatch" not in result.stdout + result.stderr))):
            raise ValueError("compiled path probe or wrong-root negative control failed")


def publish(bundle: Path, root: Path, packet: Path, run_id: str, attempt: str) -> None:
    seal(bundle, root, run_id, attempt)
    transport.pack(bundle, packet, compression="zst")
    if packet.stat().st_size > 64 * 1024 * 1024:
        raise ValueError("complete Windows proof packet exceeds 64 MiB qualification budget")
    values = {"archive-sha": transport.checksum(packet), "logical-root": str(root),
              "run-id": run_id, "attempt": attempt, "source-sha": git(root, "rev-parse", "HEAD")}
    with Path(os.environ["GITHUB_OUTPUT"]).open("a") as stream:
        for key, value in values.items():
            stream.write(f"{key}={value}\n")


def consume(packet: Path, root: Path, stage: Path, digest: str, expected: dict, *, execute: bool) -> int:
    extract(packet, stage, digest)
    data = verify_payload(stage, require_qt=True)
    receipt = json.loads((stage / "producer.json").read_text())
    check_receipt(receipt, expected)
    check_receipt(receipt, {"source_sha": git(root, "rev-parse", "HEAD"),
        "source_tree": git(root, "rev-parse", "HEAD^{tree}"), "sources": source_inventory(root)})
    if data["build_root"] != receipt["logical_root"]:
        raise ValueError("compiled root and producer receipt mismatch")
    verify_complete(data)
    validate_runtime_root(data["build_root"], root)
    # Installation happens only after the entire archive/source/root contract passes.
    for name in data["files"]:
        destination = shared.within(root, name)
        if destination.exists():
            raise ValueError(f"refusing to overwrite checkout file: {name}")
    if (root / "manifest.json").exists():
        raise ValueError("stale installed manifest")
    shutil.copytree(stage, root, dirs_exist_ok=True)
    if execute:
        (root / "target/windows-proof-temp").mkdir(parents=True)
        os.environ.update(execution_environment(root.as_posix(), dict(os.environ)))
        qualify_compiled_root(data, root)
    return run_verified(root / "manifest.json", root) if execute else 0


def place(source: Path, logical: str) -> None:
    destination = Path(windows_root(logical) if platform.system() == "Windows" else logical)
    windows_root(logical)
    if platform.system() not in ("Linux", "Windows"):
        raise ValueError("proof requires Linux producer or Windows consumer")
    transport.place(source, destination)
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with Path(output).open("a") as stream:
            stream.write(f"logical-root={logical}\nreal-root={destination.as_posix()}\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    for name in ("place", "restore"):
        command = sub.add_parser(name)
        command.add_argument("--source", type=Path, required=True)
        command.add_argument("--logical-root", required=True)
    producer = sub.add_parser("seal")
    producer.add_argument("--bundle", type=Path, required=True)
    producer.add_argument("--run-id", required=True)
    producer.add_argument("--attempt", required=True)
    producer.add_argument("--archive", type=Path, required=True)
    consumer = sub.add_parser("consume")
    consumer.add_argument("--archive", type=Path, required=True)
    consumer.add_argument("--stage", type=Path, required=True)
    consumer.add_argument("--sha256", required=True)
    consumer.add_argument("--run-id", required=True)
    consumer.add_argument("--attempt", required=True)
    consumer.add_argument("--source-sha", required=True)
    consumer.add_argument("--logical-root", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    if args.command == "place":
        place(args.source, args.logical_root)
    elif args.command == "restore":
        destination = Path(windows_root(args.logical_root) if platform.system() == "Windows" else args.logical_root)
        transport.restore(args.source, destination)
    elif args.command == "seal":
        publish(args.bundle, root, args.archive, args.run_id, args.attempt)
    else:
        return consume(args.archive, root, args.stage, args.sha256,
            {"run_id": args.run_id, "run_attempt": args.attempt, "source_sha": args.source_sha,
             "logical_root": args.logical_root}, execute=True)
    return 0


if __name__ == "__main__":
    started = time.monotonic()
    result = {"status": "failed"}
    try:
        code = main()
        result.update(status="passed" if code == 0 else "failed", exit_code=code)
    except Exception as error:
        result["error"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        result["seconds"] = time.monotonic() - started
        if "consume" in os.sys.argv:
            (Path(__file__).resolve().parents[1] / "windows-proof-result.json").write_text(json.dumps(result, indent=2) + "\n")
    raise SystemExit(code)
