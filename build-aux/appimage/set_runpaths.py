#!/usr/bin/env -S uv run --script
"""Give every bundled ELF an RPATH that points at <AppDir>/usr/lib relative to
itself, so the bundle works WITHOUT exporting LD_LIBRARY_PATH. (DT_RPATH, not
RUNPATH: it is searched before any LD_LIBRARY_PATH the user's session exports
and also applies to the dependencies of what it loads.) An exported
LD_LIBRARY_PATH leaks into every process OBS spawns (/bin/sh, xdg-open, ...),
and host programs crash when they pick up the bundled, newer glibc.
An optional second argument is an absolute directory appended to every RPATH
(where AppRun puts the bundled glibc, libstdc++ and libgcc_s; it stays empty
when the host's newer ones are used).

    uv run --no-project build-aux/appimage/set_runpaths.py <AppDir> [extra-absolute-dir]

Ported off the shell script in #962; assemble_appdir.py runs it in the AppImage
build image, which has uv on PATH like every other step of that build.
"""

from __future__ import annotations

import os
import subprocess
import sys
from collections.abc import Iterator
from pathlib import Path

# `head -c4` of an ELF file, the magic the shell compared against '177ELF'.
ELF_MAGIC = b"\x7fELF"
# `find -perm -u+x`: the owner's execute bit, which stat reports for the file.
OWNER_EXECUTE = 0o100


def candidate_files(root: Path) -> Iterator[Path]:
    """The files the shell's `find` selected: `*.so`, `*.so.*` or owner-executable.

    Sorted, where `find` returned whatever the filesystem gave it; `-type f`
    excluded symlinks, so a `libfoo.so -> libfoo.so.1` link is not patched twice.
    """
    for directory, dirnames, filenames in os.walk(root):
        dirnames.sort()
        for name in sorted(filenames):
            path = Path(directory) / name
            if path.is_symlink():
                continue
            if name.endswith(".so") or ".so." in name or path.stat().st_mode & OWNER_EXECUTE:
                yield path


def is_elf(path: Path) -> bool:
    try:
        with path.open("rb") as handle:
            return handle.read(len(ELF_MAGIC)) == ELF_MAGIC
    except OSError:
        return False


def rpath(path: Path, libdir: Path, extra: str) -> str:
    """`$ORIGIN` plus the path back to usr/lib, and the extra directory if given."""
    relative = os.path.relpath(libdir, path.parent)
    origin = "$ORIGIN" if relative == "." else f"$ORIGIN/{relative}"
    return origin + extra


def set_runpaths(appdir: Path, extra: str) -> int:
    """Patch every bundled ELF, and report how many patchelf accepted."""
    libdir = appdir / "usr" / "lib"
    count = 0
    for path in candidate_files(appdir / "usr"):
        # ld.so itself and glibc's loader-adjacent bits must stay untouched.
        if not is_elf(path) or path.name.startswith("ld-linux"):
            continue
        patched = subprocess.run(
            ["patchelf", "--force-rpath", "--set-rpath", rpath(path, libdir, extra), str(path)],
            capture_output=True,
            text=True,
        )
        if patched.returncode != 0:
            print(f"patchelf failed: {path}", file=sys.stderr)
            continue
        count += 1
    print(f"RPATH set on {count} ELF files")
    return 0


def main(argv: list[str]) -> int:
    if not argv:
        print("usage: set_runpaths.py <AppDir> [extra-absolute-dir]", file=sys.stderr)
        return 2
    appdir = Path(argv[0]).resolve()
    extra = f":{argv[1]}" if len(argv) > 1 and argv[1] else ""
    return set_runpaths(appdir, extra)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
