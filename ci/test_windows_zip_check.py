#!/usr/bin/env -S uv run --script
"""windows_zip_check.py catches a Windows zip without the VC runtime (#626).

uv run python -m unittest ci/test_windows_zip_check.py
"""

import io
import struct
import sys
import unittest
import zipfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import windows_zip_check as w  # noqa: E402


def pe(dlls: list[str]) -> bytes:
    """A minimal PE32+ whose import table names `dlls`."""
    va, raw = 0x1000, 0x200
    blob = b""
    name_off = (len(dlls) + 1) * 20
    table = b""
    for d in dlls:
        table += struct.pack("<IIIII", 0, 0, 0, va + name_off + len(blob), 0)
        blob += d.encode() + b"\0"
    table += b"\0" * 20
    sect = table + blob
    opt = bytearray(240)
    struct.pack_into("<H", opt, 0, 0x20B)
    struct.pack_into("<I", opt, 108, 16)
    struct.pack_into("<II", opt, 112 + 8, va, len(table))
    coff = struct.pack("<HHIIIHH", 0x8664, 1, 0, 0, 0, len(opt), 0)
    sec = struct.pack(
        "<8sIIIIIIHHI", b".idata", len(sect), va, len(sect), raw, 0, 0, 0, 0, 0
    )
    head = bytearray(0x40) + b"PE\0\0" + coff + bytes(opt) + sec
    head[:2] = b"MZ"
    struct.pack_into("<I", head, 0x3C, 0x40)
    return bytes(head).ljust(raw, b"\0") + sect


def make(files: dict[str, bytes]) -> zipfile.ZipFile:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        for k, v in files.items():
            z.writestr(k, v)
    return zipfile.ZipFile(buf)


class T(unittest.TestCase):
    def good(self):
        return {
            "bin/64bit/obs64.exe": pe(["VCRUNTIME140.dll", "KERNEL32.dll"]),
            "core/x/x.dll": pe(["msvcp140.dll"]),
            "bin/64bit/vcruntime140.dll": b"",
            "bin/64bit/msvcp140.dll": b"",
        }

    def test_imports(self):
        self.assertEqual(
            w.imports(pe(["VCRUNTIME140.dll", "kernel32.dll"])),
            ["vcruntime140.dll", "kernel32.dll"],
        )

    def test_ok(self):
        self.assertEqual(w.problems(make(self.good())), [])

    def test_missing_runtime(self):
        f = self.good()
        del f["bin/64bit/msvcp140.dll"]
        self.assertTrue(any("msvcp140.dll" in p for p in w.problems(make(f))))

    def test_runtime_in_unrelated_plugin_directory_does_not_satisfy_executable(self):
        f = self.good()
        f["core/unrelated/vcruntime140.dll"] = f.pop("bin/64bit/vcruntime140.dll")
        self.assertTrue(
            any("obs64.exe imports vcruntime140.dll" in p for p in w.problems(make(f)))
        )

    def test_pdb_and_layout(self):
        self.assertEqual(len(w.problems(make({"bin/64bit/obs64.pdb": b""}))), 3)

    def test_version_banner(self):
        v = "0.1.0-beta.20261013"
        banner = f"OBS-Studio-Rust {v} (based on OBS 33.0.0-beta6-947-gc553fd90a)\n"
        self.assertIsNone(w.version_problem(banner, v))
        self.assertIsNone(w.version_problem("OBS 33.0.0\n", None))

    def test_version_banner_wrong(self):
        v = "0.1.0-beta.20261013"
        self.assertIsNotNone(
            w.version_problem("OBS-Studio-Rust 0.1.0 (based on OBS 33.0.0)\n", v)
        )
        self.assertIsNotNone(w.version_problem("OBS 33.0.0\n", v))
        # A longer version that merely starts with the expected one is not a match.
        self.assertIsNotNone(
            w.version_problem(f"OBS-Studio-Rust {v}3 (based on OBS 33.0.0)\n", v)
        )


if __name__ == "__main__":
    unittest.main()
