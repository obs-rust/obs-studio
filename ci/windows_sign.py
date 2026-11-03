#!/usr/bin/env -S uv run --script
"""Prepare Windows release packages for Azure Artifact Signing.

The Windows release ships ZIPs (application and symbols) and, once the
installer lands, MSIs. Azure Artifact Signing signs loose files listed in a
newline-delimited catalog, so the binaries inside each ZIP are unpacked,
signed in place, and packed back into the same ZIP with the same entry order.

  windows_sign.py unpack --work DIR --catalog FILE PKG...
      extract every *.zip in PKG into DIR/<n>/, and write the catalog of
      every signable file: each PE inside the ZIPs and each PKG that is
      itself signable (.msi, .exe, ...).
  windows_sign.py azure-catalog --catalog FILE --output FILE
      convert the absolute verification/unsigned catalog to Azure's paths
      relative to the output catalog's directory; preserve the input catalog.
  windows_sign.py repack --work DIR PKG...
      write the (now signed) files from DIR/<n>/ back into each ZIP.
"""

from pathlib import Path
import argparse
import os
import sys
import zipfile

# Authenticode-signable file types a Windows package can carry: PE images,
# Python extension modules, drivers and installers.
SIGNABLE = frozenset(
    {".exe", ".dll", ".pyd", ".sys", ".ocx", ".node", ".msi", ".cat"}
)


def signable(name: str) -> bool:
    return Path(name).suffix.lower() in SIGNABLE


def _safe(root: Path, member: str) -> Path:
    dest = (root / member).resolve()
    if root.resolve() not in dest.parents and dest != root.resolve():
        raise ValueError(f"unsafe ZIP entry: {member}")
    return dest


def unpack(work: Path, catalog: Path, packages: list[Path]) -> list[Path]:
    files: list[Path] = []
    for n, pkg in enumerate(packages):
        if pkg.suffix.lower() == ".zip":
            root = work / str(n)
            root.mkdir(parents=True, exist_ok=False)
            with zipfile.ZipFile(pkg) as z:
                for info in z.infolist():
                    dest = _safe(root, info.filename)
                    if info.is_dir():
                        continue
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    dest.write_bytes(z.read(info))
                    if signable(info.filename):
                        files.append(dest)
        elif signable(pkg.name):
            files.append(pkg.resolve())
        else:
            raise ValueError(f"not a ZIP or signable file: {pkg}")
    if not files:
        raise ValueError("no signable Windows binaries found")
    catalog.write_text("".join(f"{f}\n" for f in files))
    print(f"catalog: {len(files)} files -> {catalog}")
    return files


def repack(work: Path, packages: list[Path]) -> None:
    for n, pkg in enumerate(packages):
        if pkg.suffix.lower() != ".zip":
            continue
        root = work / str(n)
        tmp = pkg.with_name(pkg.name + ".signed.tmp")
        with zipfile.ZipFile(pkg) as src, zipfile.ZipFile(
            tmp, "w", zipfile.ZIP_DEFLATED
        ) as dst:
            for info in src.infolist():
                if info.is_dir():
                    dst.writestr(info, b"")
                    continue
                data = _safe(root, info.filename).read_bytes()
                dst.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED)
        os.replace(tmp, pkg)
        print(f"repacked: {pkg}")


def azure_catalog(catalog: Path, output: Path) -> None:
    # Azure prepends the catalog directory even to drive-qualified entries.
    # Keep verification's absolute paths separate from this Azure-only input.
    files = catalog.read_text(encoding="utf-8-sig").splitlines()
    relative = [os.path.relpath(Path(f).resolve(), output.resolve().parent) for f in files]
    output.write_text("".join(f"{f}\n" for f in relative), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("command", choices=["unpack", "repack", "azure-catalog"])
    ap.add_argument("packages", nargs="*", type=Path)
    ap.add_argument("--work", type=Path)
    ap.add_argument("--catalog", type=Path)
    ap.add_argument("--output", type=Path)
    a = ap.parse_args()
    if a.command == "azure-catalog":
        if a.catalog is None or a.output is None:
            ap.error("azure-catalog needs --catalog and --output")
        azure_catalog(a.catalog, a.output)
        return 0
    if not a.packages or a.work is None:
        ap.error("unpack/repack need --work and at least one package")
    if a.command == "unpack":
        if a.catalog is None:
            ap.error("unpack needs --catalog")
        unpack(a.work, a.catalog, a.packages)
    else:
        repack(a.work, a.packages)
    return 0


if __name__ == "__main__":
    sys.exit(main())
