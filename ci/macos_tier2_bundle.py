#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.14"
# ///
"""Run-only macOS Tier 2 bundle (#5009).

Linux cross-builds and signs every CMake/CTest artifact (`ci/cmocka_link_check.py
--alias mac-arm64`). `stage` copies that run closure, with the pinned dependency
dylibs and Qt frameworks, into a sealed bundle; `ci/macos_test_workspace.py
pack/unpack` transports it. macOS only verifies the seal and runs the unchanged
CTest registry twice (C reference, then hybrid libobs). No compiler is invoked.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

import cmocka_link_check
import macos_test_artifact

EXCLUDED_PARTS = {"CMakeFiles", "compiler-cache", "cargo", "corrosion", "_deps", "obs-preflight-cargo", "soldr-tools"}
EXCLUDED_SUFFIXES = (".o", ".a", ".obj")


def sha256(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def dependency_roots(root: Path) -> tuple[Path, Path]:
    libs = sorted((root / ".deps").glob("obs-deps-*-universal/lib"))
    qt = sorted((root / ".deps").glob("obs-deps-qt6-*-universal/lib"))
    libs = [path for path in libs if "qt6" not in path.parent.name]
    if len(libs) != 1 or len(qt) != 1:
        raise ValueError("need exactly one prebuilt dependency tree and one Qt tree under .deps")
    return libs[0], qt[0]


def selected_files(root: Path, work: Path) -> list[Path]:
    libs, qt = dependency_roots(root)
    files: list[Path] = []
    for path in sorted(work.rglob("*")):
        relative = path.relative_to(work)
        if (path.is_dir() or EXCLUDED_PARTS & set(relative.parts) or path.suffix in EXCLUDED_SUFFIXES
                or relative.parts[0].startswith(".ninja") or relative.name in ("build.ninja", "CMakeCache.txt")):
            continue
        files.append(path)
    for tree in (libs, qt):
        files.extend(path for path in sorted(tree.rglob("*")) if path.is_file() or path.is_symlink())
    return files


def stage(root: Path, work: Path, output: Path, *, source_sha: str) -> dict:
    """Seal the signed run closure; refuse unsigned images or an empty registry."""
    registry = (work / "ctest-registry.txt").read_text()
    total = re.search(r"Total Tests: (\d+)", registry)
    if not total or int(total.group(1)) == 0:
        raise ValueError("empty CTest registry")
    cmocka_link_check.verify_signed_images(work)
    if output.exists():
        raise ValueError(f"bundle output already exists: {output}")
    files: dict[str, str] = {}
    for path in selected_files(root, work):
        relative = path.relative_to(root)
        target = output / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        if path.is_symlink():
            target.symlink_to(os.readlink(path))
            continue
        target.write_bytes(path.read_bytes())
        target.chmod(path.stat().st_mode & 0o777)
        files[relative.as_posix()] = sha256(target)
    manifest = {"schema": 1, "source_sha": source_sha, "root": str(root), "work": work.name, "files": files,
                "tests": int(total.group(1))}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=1, sort_keys=True) + "\n")
    return manifest


def verify(directory: Path, *, source_sha: str) -> dict:
    """Check the seal: source, every file hash, no missing or extra regular file."""
    manifest = json.loads((directory / "manifest.json").read_text())
    if manifest.get("schema") != 1 or manifest.get("source_sha") != source_sha:
        raise ValueError(f"bundle source {manifest.get('source_sha')} is not {source_sha}")
    present = {p.relative_to(directory).as_posix() for p in directory.rglob("*")
               if p.is_file() and not p.is_symlink() and p.name != "manifest.json"}
    expected = set(manifest["files"])
    if expected - present:
        raise ValueError(f"missing bundle files: {sorted(expected - present)[:5]}")
    if present - expected:
        raise ValueError(f"extra bundle files: {sorted(present - expected)[:5]}")
    for relative, digest in manifest["files"].items():
        if sha256(directory / relative) != digest:
            raise ValueError(f"hash mismatch: {relative}")
    return manifest


# Configures and compiles a fixture while it runs (check_core_module_manifest_source.cmake):
# a native-only residue (#5848) that rust-tier2-macos runs from its own CMake tree.
RUNTIME_COMPILED_TESTS = "^test_core_module_manifest_source$"
TEST_TIMEOUT_SECONDS = "300"


def ctest_command(work: Path, jobs: int) -> list[str]:
    return ["ctest", "--test-dir", str(work), "--output-on-failure", "-j", str(jobs),
            "--timeout", TEST_TIMEOUT_SECONDS, "-E", RUNTIME_COMPILED_TESTS]


def suite_environment(root: Path, work: Path, variant: str) -> dict[str, str]:
    libs, qt = dependency_roots(root)
    first = work / "libobs-c-reference" if variant == "OFF" else work
    env = dict(os.environ)
    env["DYLD_LIBRARY_PATH"] = os.pathsep.join([str(first), str(work / "sanitizer-runtimes"), str(libs)])
    env["DYLD_FRAMEWORK_PATH"] = str(qt)
    return env


def run_suites(root: Path, work_name: str, *, jobs: int) -> None:
    """Execute the existing CTest registry once per libobs variant."""
    work = root / work_name
    for variant in ("OFF", "ON"):
        print(f"== [{variant}] ctest", flush=True)
        result = subprocess.run(ctest_command(work, jobs), env=suite_environment(root, work, variant))
        if result.returncode:
            sys.exit(result.returncode)


def run_core_suite(root: Path, work_name: str, core: Path, variant: str, *, jobs: int) -> None:
    """Run the prebuilt registry against a natively built Cargo core.

    The tests ask for @rpath/obs.dylib; dyld resolves it by leaf name through
    DYLD_LIBRARY_PATH, so the Cargo-built core is presented under that name and
    a loader probe proves it (and not a bundled libobs) was the image loaded.
    """
    work = root / work_name
    directory = work / f"core-{variant}"
    directory.mkdir(exist_ok=True)
    link = directory / "obs.dylib"
    link.unlink(missing_ok=True)
    link.symlink_to(core.resolve())
    libs, _ = dependency_roots(root)
    env = suite_environment(root, work, "ON")
    env["DYLD_LIBRARY_PATH"] = os.pathsep.join([str(directory), str(work / "sanitizer-runtimes"), str(libs)])
    print(f"== Cargo [{variant}] ctest", flush=True)
    result = subprocess.run(ctest_command(work, jobs), env=env)
    if result.returncode:
        sys.exit(result.returncode)
    probe_env = dict(env, DYLD_PRINT_LIBRARIES="1")
    log = directory / "loader.log"
    with log.open("wb+") as stream:
        result = subprocess.run([str(work / "test/cmocka/test_bitstream")], env=probe_env, stdout=stream,
                                stderr=subprocess.STDOUT)
    if result.returncode:
        sys.exit(result.returncode)
    if str(directory) not in log.read_text(errors="replace"):
        raise ValueError(f"Cargo [{variant}] tests did not load {link}; see {log}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    command = sub.add_parser("stage")
    command.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    command.add_argument("--work", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--source-sha", required=True)
    command = sub.add_parser("ctest-core")
    command.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    command.add_argument("--work-name", default="build_cmocka_link_mac-arm64")
    command.add_argument("--core", type=Path, required=True)
    command.add_argument("--variant", required=True)
    command.add_argument("--jobs", type=int, default=3)
    command = sub.add_parser("run")
    command.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    command.add_argument("--source-sha", required=True)
    command.add_argument("--jobs", type=int, default=3)
    args = parser.parse_args()
    if args.action == "stage":
        manifest = stage(args.root, args.work.resolve(), args.output, source_sha=args.source_sha)
        print(f"staged {len(manifest['files'])} files, {manifest['tests']} tests")
        return 0
    if args.action == "ctest-core":
        run_core_suite(args.root, args.work_name, args.core, args.variant, jobs=args.jobs)
        return 0
    # unpack already extracted the bundle over the common checkout; verify in place.
    manifest = verify_in_place(args.root, args.source_sha)
    run_suites(args.root, manifest["work"], jobs=args.jobs)
    return 0


def verify_in_place(root: Path, source_sha: str) -> dict:
    manifest = json.loads((root / "manifest.json").read_text())
    if manifest.get("source_sha") != source_sha:
        raise ValueError(f"bundle source {manifest.get('source_sha')} is not {source_sha}")
    for relative, digest in manifest["files"].items():
        path = root / relative
        if not path.is_file():
            raise ValueError(f"missing bundle file: {relative}")
        if sha256(path) != digest:
            raise ValueError(f"hash mismatch: {relative}")
    return manifest


if __name__ == "__main__":
    raise SystemExit(main())
