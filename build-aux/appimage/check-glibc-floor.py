#!/usr/bin/env -S uv run --script
"""Fail when an ELF file in the AppDir needs a newer glibc than the floor.

    uv run --script check-glibc-floor.py <AppDir> [floor]   (floor 2.28)

The floor is the oldest glibc the AppImage runs on (obs-rust/obs-studio#926).
The C++ runtime of that floor's oldest distribution (AlmaLinux/RHEL 8's
libstdc++ from GCC 8) is checked too, since an AppImage that does not bundle
glibc does not bundle libstdc++ either: GLIBCXX_3.4.25 and CXXABI_1.3.11.

Only the versions a file *requires* count (objdump -p, "Version References"),
so the bundled glibc itself, which defines every GLIBC_2.x up to its own, does
not trip the check.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from pathlib import Path

ELF_MAGIC = b"\x7fELF"
# Symbol version families, and the newest version the floor allows.
GLIBC_FLOOR = "2.28"
GLIBCXX_FLOOR = 25  # GLIBCXX_3.4.25
CXXABI_FLOOR = 11  # CXXABI_1.3.11
FAMILIES = (
    ("GLIBC_2.", "GLIBC_2."),
    ("GLIBCXX_3.4.", "GLIBCXX_3.4."),
    ("CXXABI_1.3.", "CXXABI_1.3."),
)
# objdump's requirement block: from the heading to the first blank line.
_SECTION = re.compile(r"^Version References:.*?(?=\n\n|\Z)", re.MULTILINE | re.DOTALL)


def is_elf(path: Path) -> bool:
    """The magic bytes, read the way `head -c 4` does."""
    try:
        with path.open("rb") as handle:
            return handle.read(len(ELF_MAGIC)) == ELF_MAGIC
    except OSError:
        return False


def version_references(path: Path) -> str:
    """What the file requires from its libraries; empty when objdump fails."""
    result = subprocess.run(["objdump", "-p", str(path)], capture_output=True, text=True)
    match = _SECTION.search(result.stdout)
    return match.group(0) if match else ""


def newest(references: str, prefix: str) -> int | None:
    """The highest version number this file requires in one family."""
    found = [int(number) for number in re.findall(re.escape(prefix) + r"([0-9]+)", references)]
    return max(found) if found else None


def too_new(path: Path, floor_minor: int) -> str:
    """The requirements above the floor, ready to print after the file name."""
    references = version_references(path)
    over = []
    for prefix, newer in FAMILIES:
        need = newest(references, prefix)
        limit = floor_minor if prefix == "GLIBC_2." else GLIBCXX_FLOOR if prefix == "GLIBCXX_3.4." else CXXABI_FLOOR
        if need is not None and need > limit:
            over.append(f"{newer}{need}")
    return " ".join(over)


def check(appdir: Path, floor: str) -> int:
    floor_minor = int(floor.removeprefix("2."))
    total = over = 0
    for root, _, files in os.walk(appdir):
        for name in files:
            path = Path(root) / name
            if not is_elf(path):
                continue
            total += 1
            bad = too_new(path, floor_minor)
            if bad:
                print(f"::error::{path.relative_to(appdir)} needs {bad} "
                      f"(floor: glibc {floor}, GLIBCXX_3.4.{GLIBCXX_FLOOR}, CXXABI_1.3.{CXXABI_FLOOR})",
                      file=sys.stderr)
                over += 1
    if total == 0:
        print(f"::error::check-glibc-floor: no ELF files under {appdir}", file=sys.stderr)
        return 1
    if over:
        print(f"check-glibc-floor: FAILED, {over} of {total} ELF files need a newer glibc"
              f" or C++ runtime than the floor", file=sys.stderr)
        return 1
    print(f"check-glibc-floor: all {total} ELF files run on glibc {floor} with GLIBCXX_3.4.{GLIBCXX_FLOOR}")
    return 0


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    return check(Path(argv[1]).resolve(), argv[2] if len(argv) == 3 else GLIBC_FLOOR)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
