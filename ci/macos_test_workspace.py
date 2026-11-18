#!/usr/bin/env -S uv run --script
"""Place both hosts at the same real checkout path and transport the full bundle."""
from __future__ import annotations

import argparse
import hashlib
import os
import shutil
import subprocess
import tarfile
from pathlib import Path


def place(source: Path, destination: Path) -> None:
    if (not source.is_absolute() or source.resolve() != source or not source.is_dir()
            or not destination.is_absolute() or destination.resolve() != destination
            or destination.exists() or destination.is_relative_to(source)):
        raise ValueError("checkout placement requires distinct real absolute paths and an absent destination")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(source, destination)
    # Actions and its post steps still address GITHUB_WORKSPACE. Rust and
    # Python resolve this link to the common real path before compiling.
    source.symlink_to(destination, target_is_directory=True)


def restore(source: Path, destination: Path) -> None:
    if not source.is_symlink() or source.resolve() != destination or not destination.is_dir():
        raise ValueError("refusing to restore an unrelated workspace")
    source.unlink()
    shutil.move(destination, source)
    try:
        destination.parent.rmdir()
    except PermissionError:
        # Setup owns COMMON_PARENT, but /opt itself remains root-owned on
        # hosted macOS. Removing its empty child needs the setup privilege.
        # Never recurse or suppress a failed removal (including ENOTEMPTY).
        subprocess.run(["sudo", "-n", "rmdir", "--", str(destination.parent)], check=True)


def checksum(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def pack(bundle: Path, archive: Path) -> None:
    if not (bundle / "manifest.json").is_file() or archive.exists():
        raise ValueError("complete manifest and new archive required")
    with tarfile.open(archive, "w:gz") as tar:
        tar.add(bundle, arcname=".")  # Includes hidden build-script files and permissions.
    archive.with_suffix(archive.suffix + ".sha256").write_text(checksum(archive) + "\n")


def unpack(archive: Path, root: Path) -> None:
    if checksum(archive) != archive.with_suffix(archive.suffix + ".sha256").read_text().strip():
        raise ValueError("macOS bundle archive checksum mismatch")
    with tarfile.open(archive) as tar:
        tar.extractall(root, filter="data")
    if not (root / "manifest.json").is_file():
        raise ValueError("macOS bundle has no manifest")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    for action in ("place", "restore"):
        command = sub.add_parser(action)
        command.add_argument("--source", type=Path, required=True)
        command.add_argument("--destination", type=Path, required=True)
    command = sub.add_parser("pack")
    command.add_argument("--bundle", type=Path, required=True)
    command.add_argument("--archive", type=Path, required=True)
    command = sub.add_parser("unpack")
    command.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()
    if args.action in ("place", "restore"):
        {"place": place, "restore": restore}[args.action](args.source, args.destination)
        if args.action == "place" and os.environ.get("GITHUB_OUTPUT"):
            with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
                output.write(f"common-parent={args.destination.parent}\n")
    elif args.action == "pack":
        pack(args.bundle, args.archive)
    else:
        unpack(args.archive, Path(__file__).resolve().parents[1])


if __name__ == "__main__":
    main()
