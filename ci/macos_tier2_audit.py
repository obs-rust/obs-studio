#!/usr/bin/env -S uv run --script
"""Verify signed arm64 Tier 2 images and their recorded Darwin load closure."""

from __future__ import annotations

import argparse
import json
import struct
from pathlib import Path

import macos_tier2_bundle as bundle


def loads(path: Path) -> list[tuple[int, str]]:
    data = bundle.arm64_slice(path.read_bytes())
    bundle.signature(data)
    count, size = struct.unpack_from("<II", data, 16)
    offset, result = 32, []
    for _ in range(count):
        command, length = struct.unpack_from("<II", data, offset)
        if length < 8 or offset + length > 32 + size:
            raise ValueError(f"invalid load command: {path}")
        if command in (0xC, 0x80000018, 0x8000001F, 0x80000023, 0x8000001C):
            start = struct.unpack_from("<I", data, offset + 8)[0]
            if start < 12 or start >= length:
                raise ValueError(f"invalid load path: {path}")
            result.append((command, data[offset + start:offset + length].split(b"\0", 1)[0].decode()))
        offset += length
    return result


def closure(images: list[Path], search: list[Path], verify) -> dict:
    pending = list(images)
    checked = {}
    while pending:
        image = pending.pop().resolve()
        if str(image) in checked:
            continue
        verify(image)
        commands = loads(image)
        rpaths = [value for command, value in commands if command == 0x8000001C]
        dependencies = []
        for command, value in commands:
            if command == 0x8000001C or value.startswith(("/usr/lib/", "/System/Library/")):
                continue
            def expand(path):
                return Path(path.replace("@loader_path", str(image.parent)).replace("@executable_path", str(image.parent)))
            if value.startswith("@rpath/"):
                suffix = value.removeprefix("@rpath/")
                candidates = [expand(path) / suffix for path in rpaths] + [root / suffix for root in search]
            elif value.startswith(("@loader_path/", "@executable_path/")):
                candidates = [expand(value)]
            elif value.startswith("/"):
                candidate = Path(value)
                candidates = [candidate] if any(candidate.is_relative_to(root) for root in search) else []
            else:
                candidates = []
            found = next((path.resolve() for path in candidates if path.is_file()), None)
            if found is None:
                raise ValueError(f"unresolved Darwin load in {image}: {value}")
            dependencies.append({"load": value, "path": str(found)})
            pending.append(found)
        checked[str(image)] = dependencies
    return checked


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", required=True, type=Path)
    parser.add_argument("--search", action="append", type=Path, default=[])
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    verify = bundle.verifier(Path(__file__).with_name("macos_test_artifact.py"))
    images = [Path(line) for line in args.inventory.read_text().splitlines() if line]
    result = closure(images, [path.resolve() for path in args.search], verify)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(f"Verified {len(images)} roots and {len(result)} total signed arm64 images")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
