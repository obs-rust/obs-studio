#!/usr/bin/env -S uv run --script
"""Check the packaged Windows zip the way a user meets it (#626, #627).

    uv run ci/windows_zip_check.py check  <zip> [--run]
    uv run ci/windows_zip_check.py split-symbols <zip>

check: reads the DLL imports of every .exe/.dll in the zip (pure-Python PE
parser, so it needs no dumpbin and works on any host) and fails when one is a
Visual C++ runtime DLL (vcruntime140, msvcp140, ...) that the zip does not
carry. The CI runner has the redistributable installed, so merely launching
there cannot catch a missing runtime; the import check can. It also fails when
the zip carries PDBs (they belong in the separate symbols zip), has no
bin/64bit/obs64.exe or no plugins under core/. With --run it extracts the zip
to an empty directory and runs `obs64.exe --version` from bin/64bit.

split-symbols: moves every .pdb out of the zip into <zip stem>-symbols.zip.
"""

from __future__ import annotations

import argparse
import os
import re
import struct
import subprocess
import sys
import tempfile
import zipfile
from pathlib import PurePosixPath

# DLLs a Windows machine does not necessarily have: the Visual C++ runtime.
VC_RUNTIME = re.compile(r"^(vcruntime|msvcp|concrt|vcomp|vccorlib|mfc|vcamp)\d+(_\d+)?(d)?\.dll$")
EXPECTED_EXE = "bin/64bit/obs64.exe"


def _sections(data: bytes, pe: int):
    nsec = struct.unpack_from("<H", data, pe + 6)[0]
    optsize = struct.unpack_from("<H", data, pe + 20)[0]
    base = pe + 24 + optsize
    for i in range(nsec):
        vsize, va, rawsize, raw = struct.unpack_from("<IIII", data, base + 40 * i + 8)
        yield va, max(vsize, rawsize), raw


def _off(data: bytes, pe: int, rva: int) -> int:
    for va, size, raw in _sections(data, pe):
        if va <= rva < va + size:
            return rva - va + raw
    raise ValueError(f"RVA {rva:#x} is in no section")


def _cstr(data: bytes, off: int) -> str:
    return data[off : data.index(b"\0", off)].decode("ascii", "replace")


def imports(data: bytes) -> list[str]:
    """Lower-cased DLL names a PE file imports (normal and delay-load)."""
    if data[:2] != b"MZ":
        return []
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe : pe + 4] != b"PE\0\0":
        return []
    magic = struct.unpack_from("<H", data, pe + 24)[0]
    dirs = pe + 24 + (96 if magic == 0x10B else 112)
    ndirs = struct.unpack_from("<I", data, dirs - 4)[0]
    names: list[str] = []
    for index, entry, name_at in ((1, 20, 12), (13, 32, 4)):
        if index >= ndirs:
            continue
        rva, _ = struct.unpack_from("<II", data, dirs + 8 * index)
        if not rva:
            continue
        off = _off(data, pe, rva)
        while True:
            fields = data[off : off + entry]
            if len(fields) < entry or not any(fields):
                break
            name_rva = struct.unpack_from("<I", fields, name_at)[0]
            if name_rva:
                names.append(_cstr(data, _off(data, pe, name_rva)).lower())
            off += entry
    return names


def problems(zf: zipfile.ZipFile) -> list[str]:
    names = [n for n in zf.namelist() if not n.endswith("/")]
    lower = {PurePosixPath(n).name.lower() for n in names}
    out: list[str] = []
    if EXPECTED_EXE not in names:
        out.append(f"{EXPECTED_EXE} is not in the zip")
    if not any(n.startswith("core/") and n.lower().endswith(".dll") for n in names):
        out.append("no plugin DLLs under core/")
    pdbs = [n for n in names if n.lower().endswith(".pdb")]
    if pdbs:
        out.append(f"{len(pdbs)} PDBs in the zip (first: {pdbs[0]}); ship them in the symbols zip")
    for n in names:
        if not n.lower().endswith((".exe", ".dll")):
            continue
        for dep in sorted(set(imports(zf.read(n)))):
            if VC_RUNTIME.match(dep) and dep not in lower:
                out.append(f"{n} imports {dep}, which the zip does not contain")
    return out


def check(zip_path: str, run: bool) -> int:
    with zipfile.ZipFile(zip_path) as zf:
        found = problems(zf)
        for p in found:
            print(f"::error::{p}")
        if found:
            return 1
        if run:
            with tempfile.TemporaryDirectory() as clean:
                zf.extractall(clean)
                bindir = os.path.join(clean, "bin", "64bit")
                env = dict(os.environ)
                env["PATH"] = os.path.join(os.environ.get("SystemRoot", r"C:\Windows"), "System32")
                r = subprocess.run(
                    [os.path.join(bindir, "obs64.exe"), "--version"],
                    cwd=bindir,
                    env=env,
                    capture_output=True,
                    text=True,
                    timeout=120,
                )
                print(r.stdout, r.stderr)
                if r.returncode != 0 or "OBS" not in r.stdout:
                    print(f"::error::obs64.exe --version exited {r.returncode}: {r.stdout!r}")
                    return 1
    print("zip ok")
    return 0


def split_symbols(zip_path: str) -> int:
    stem, _ = os.path.splitext(zip_path)
    sym, slim = f"{stem}-symbols.zip", f"{stem}.slim.tmp"
    with (
        zipfile.ZipFile(zip_path) as src,
        zipfile.ZipFile(slim, "w", zipfile.ZIP_DEFLATED) as keep,
        zipfile.ZipFile(sym, "w", zipfile.ZIP_DEFLATED) as pdb,
    ):
        for info in src.infolist():
            dst = pdb if info.filename.lower().endswith(".pdb") else keep
            dst.writestr(info, src.read(info), compress_type=zipfile.ZIP_DEFLATED)
    os.replace(slim, zip_path)
    print(f"symbols: {sym}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("command", choices=["check", "split-symbols"])
    ap.add_argument("zip")
    ap.add_argument("--run", action="store_true")
    a = ap.parse_args()
    return check(a.zip, a.run) if a.command == "check" else split_symbols(a.zip)


if __name__ == "__main__":
    sys.exit(main())
