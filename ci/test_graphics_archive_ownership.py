#!/usr/bin/env -S uv run --script
"""Check the release archive's private graphics dependency boundary (#3549).

uv run ci/test_graphics_archive_ownership.py --archive PATH --nm llvm-nm

The unchanged native cmocka *_rust executables remain the link/runtime gate.
This focused probe diagnoses accidental cross-crate generic instantiations
before those links: obs-graphics must not pull obs-util archive members,
which also define the allocator and other exports provided by shared libobs.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import unittest
from pathlib import Path


def foreign_graphics_references(symbols: str) -> list[str]:
    """Return undefined graphics references whose implementation is in obs-util."""
    references = []
    for line in symbols.splitlines():
        if not re.search(r"obs_graphics[-.]", line):
            continue
        record = re.fullmatch(r"(.+):\s+(\S+)\s+([A-Za-z?])(?:\s+\S+)*", line)
        if record is None:
            raise ValueError(f"unrecognized graphics symbol record: {line}")
        member, symbol, kind = record.groups()
        if re.search(r"obs_graphics[-.]", member) and kind == "U" and "obs_util" in symbol:
            references.append(line)
    return references


class GraphicsArchiveOwnershipTests(unittest.TestCase):
    def test_generic_instantiation_in_util_is_rejected(self) -> None:
        line = "archive:obs_graphics-abc.cgu.o: _R_Vec_grow_one_obs_util U 0 0"
        self.assertEqual(foreign_graphics_references(line), [line])

    def test_unrecognized_graphics_record_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "unrecognized graphics symbol record"):
            foreign_graphics_references("archive:obs_graphics-abc.cgu.o: incompatible nm output")

    def test_private_bridge_and_other_crates_are_allowed(self) -> None:
        symbols = "\n".join(
            [
                "archive:obs_graphics-abc.cgu.o: obs_graphics_bmalloc U 0 0",
                "archive:obs_core-abc.cgu.o: _R_obs_util_helper U 0 0",
                "archive:obs_graphics-abc.cgu.o: _R_obs_util_helper T 0 0",
            ]
        )
        self.assertEqual(foreign_graphics_references(symbols), [])


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path)
    parser.add_argument("--nm", default="llvm-nm")
    args, rest = parser.parse_known_args()
    if args.archive is None:
        unittest.main(argv=[__file__, *rest])
    else:
        result = subprocess.run(
            [args.nm, "-A", "--undefined-only", "--format=posix", str(args.archive)],
            check=True,
            capture_output=True,
            text=True,
        )
        # Fail closed if an incompatible nm format or an empty archive was used.
        if not re.search(r"obs_graphics[-.]", result.stdout):
            parser.error("archive contains no obs-graphics members")
        try:
            references = foreign_graphics_references(result.stdout)
        except ValueError as error:
            parser.error(str(error))
        if references:
            parser.exit(1, "obs-graphics pulls obs-util archive members:\n" + "\n".join(references) + "\n")
        print("obs-graphics archive ownership: clean")
