#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# ///
"""Build the host-library loader into an AppDir (obs-rust/obs-studio#959).

    uv run build-aux/appimage/hostlib/build.py <AppDir>

For every *optional* library in host-libs.json that a bundled ELF file links:
  1. the symbols the bundle imports from it (undefined in a consumer, defined
     by the build host's copy) go into a symbol list;
  2. Implib.so generates lazy trampolines for them, and shim.cpp supplies the
     callbacks: find the host's copy (hostlib.c), or fall back to type-exact
     stubs (decltype of the library's own declarations) that fail cleanly;
  3. the shim is built as usr/lib/libobs-<name>.so.N, and every consumer's
     DT_NEEDED is switched to it (patchelf --replace-needed), with the symbol
     versions of those imports cleared (the shim has none; the host's default
     version is the one dlsym finds).
Then usr/bin/obs-hostlib-check is built from the *required* libraries, for
AppRun's preflight. The build fails rather than guess: a consumer importing a
symbol the build host's library does not define, or a symbol the stubs header
cannot type, stops it.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
import os
import re
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent


@dataclass(frozen=True)
class Install:
    debian: str
    fedora: str
    arch: str
    suse: str
    nixos: str


@dataclass(frozen=True)
class HostLib:
    """One entry of host-libs.json, parsed eagerly and strictly (no dictionaries past parse_host_libs)."""

    name: str
    kind: str  # required | optional
    feature: str
    sonames: tuple[str, ...]
    symbols: tuple[str, ...]
    headers: tuple[str, ...]
    install: Install


def parse_host_libs(path: Path) -> tuple[HostLib, ...]:
    doc = json.loads(path.read_text())
    if not isinstance(doc, dict) or set(doc) - {"_comment", "libraries"}:
        raise SystemExit(f"{path}: expected an object with 'libraries'")
    out = []
    for i, raw in enumerate(doc["libraries"]):
        where = f"{path.name}: libraries[{i}]"
        keys = {"name", "kind", "feature", "sonames", "symbols", "headers", "install"}
        if not isinstance(raw, dict) or set(raw) - keys or {"name", "kind", "feature", "sonames", "symbols",
                                                             "install"} - set(raw):
            raise SystemExit(f"{where}: needs exactly {sorted(keys)} (headers only for optional libraries)")
        inst = raw["install"]
        if not isinstance(inst, dict) or set(inst) != {"debian", "fedora", "arch", "suse", "nixos"}:
            raise SystemExit(f"{where}.install: needs debian, fedora, arch, suse and nixos")
        lib = HostLib(raw["name"], raw["kind"], raw["feature"], tuple(raw["sonames"]), tuple(raw["symbols"]),
                      tuple(raw.get("headers", ())), Install(**inst))
        if lib.kind not in ("required", "optional") or (lib.kind == "optional" and not lib.headers):
            raise SystemExit(f"{where}: kind must be required or optional, and optional needs headers")
        out.append(lib)
    return tuple(out)


# Host libraries a bundled ELF file may link without a host-libs.json entry.
# The C and C++ runtime is the base system (check-glibc-floor.sh checks its
# version; above the floor it is bundled anyway).
HOST_RUNTIME = re.compile(r"(ld-linux-x86-64|libc|libm|libmvec|libpthread|libdl|librt|libresolv|libnss_[a-z]+"
                          r"|libutil|libanl|libBrokenLocale|libthread_db|libstdc\+\+|libgcc_s)\.so\.\d+")


def unchecked_host_needs(deps: dict[str, list[str]], bundled: set[str],
                         libs: tuple[HostLib, ...]) -> dict[str, list[str]]:
    """Libraries the AppImage takes from the host that the preflight knows nothing about.

    deps maps each bundled ELF file to its DT_NEEDED entries, bundled holds the
    file names in the AppDir. Every needed library that is not bundled must be
    in host-libs.json (or be the C runtime): otherwise a host without it gets
    the dynamic loader's error, or "Graphics module not found" for a dlopen()ed
    module, instead of obs-hostlib-check's message (obs-rust/obs-studio#2298,
    #2337). Returns soname -> consumers.
    """
    known = {s for lib in libs for s in lib.sonames}
    out: dict[str, list[str]] = {}
    for consumer, names in sorted(deps.items()):
        for soname in names:
            if soname in bundled or soname in known or HOST_RUNTIME.fullmatch(soname):
                continue
            out.setdefault(soname, []).append(consumer)
    return out


IMPLIB_COMMIT = "a3e167cf978a5b8a38ec0f710d8c4331e8dcdd9a"
IMPLIB_SHA256 = "c4fdcae8a6a7db45eb636e5b862be3da914e666d66a803944aedbeeefe0c70aa"
IMPLIB_URL = f"https://github.com/yugr/Implib.so/archive/{IMPLIB_COMMIT}.tar.gz"


def run(*cmd: str | Path, **kw) -> str:
    r = subprocess.run([str(c) for c in cmd], text=True, capture_output=True, **kw)
    if r.returncode:
        raise SystemExit(f"hostlib: {' '.join(str(c) for c in cmd)}\nexited {r.returncode}:\n{r.stderr}{r.stdout}")
    return r.stdout


def is_elf(p: Path) -> bool:
    try:
        with p.open("rb") as f:
            return f.read(4) == b"\x7fELF"
    except OSError:
        return False


def needed(f: Path) -> list[str]:
    out = run("patchelf", "--print-needed", f)
    return out.split()


def undefined_symbols(f: Path) -> set[str]:
    out = run("nm", "-D", "--undefined-only", f)
    syms = set()
    for line in out.splitlines():
        parts = line.split()
        if parts and parts[0] == "U":
            syms.add(parts[1].split("@")[0])
    return syms


def defined_symbols(lib: Path) -> set[str]:
    out = run("nm", "-D", "--defined-only", lib)
    return {line.split()[2].split("@")[0] for line in out.splitlines() if len(line.split()) == 3}


def required_versions(consumer: Path, soname: str) -> set[str]:
    """The version names consumer requires from soname (objdump -p "Version References")."""
    names, inside = set(), False
    for line in run("objdump", "-p", consumer).splitlines():
        if line.strip().startswith("required from "):
            inside = line.strip() == f"required from {soname}:"
        elif inside and line.strip():
            names.add(line.split()[-1])
    return names


def host_copy(soname: str) -> Path:
    """The build host's copy of soname (from the ldconfig cache)."""
    for line in run("ldconfig", "-p").splitlines():
        m = re.match(rf"\s*{re.escape(soname)} \(libc6,x86-64\) => (\S+)", line)
        if m:
            return Path(m.group(1))
    raise SystemExit(f"hostlib: {soname} is not installed on the build host; implib-gen needs it")


# The glibc 2.28 build image has Implib.so from obs-rust/obs-studio-libs (deps.lock.json).
PREBUILT_IMPLIB = Path("/opt/obs-deps/implib/implib-gen.py")


def implib(tmp: Path) -> Path:
    if PREBUILT_IMPLIB.is_file():
        return PREBUILT_IMPLIB
    archive = tmp / "implib.tar.gz"
    with urllib.request.urlopen(IMPLIB_URL, timeout=60) as r:
        archive.write_bytes(r.read())
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    if digest != IMPLIB_SHA256:
        raise SystemExit(f"hostlib: Implib.so archive sha256 {digest}, expected {IMPLIB_SHA256}")
    with tarfile.open(archive) as t:
        t.extractall(tmp, filter="data")
    return tmp / f"Implib.so-{IMPLIB_COMMIT}" / "implib-gen.py"


def c_array(name: str, items: list[str]) -> str:
    return f"static const char *const {name}[] = {{" + "".join(f'"{x}", ' for x in items) + "NULL};\n"


def spec_decl(lib: HostLib, ident: str) -> tuple[str, str]:
    """(array declarations, initializer) for a hostlib_spec; valid C and C++."""
    arrays = c_array(f"{ident}_sonames", list(lib.sonames)) + c_array(f"{ident}_symbols", list(lib.symbols))
    return arrays, f'{{"{lib.name}", "{lib.feature}", {ident}_sonames, {ident}_symbols}}'


def shim_soname(soname: str, name: str) -> str:
    # libva.so.2 -> libobs-va.so.2: never the real name (glibc matches loaded
    # libraries by soname, so dlopen of the host's would return the shim).
    major = soname.split(".so", 1)[1]
    return f"libobs-{name}.so{major}"


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("appdir", type=Path)
    p.add_argument("--cc", default=os.environ.get("CC", "cc"))
    p.add_argument("--cxx", default=os.environ.get("CXX", "c++"))
    args = p.parse_args(argv)
    appdir = args.appdir.resolve()
    libdir = appdir / "usr/lib"
    libs = parse_host_libs(HERE / "host-libs.json")

    elves = [f for f in (appdir / "usr").rglob("*") if f.is_file() and not f.is_symlink() and is_elf(f)]
    deps = {f: needed(f) for f in elves}
    bundled = {f.name for f in appdir.rglob("*") if f.is_file()}
    unchecked = unchecked_host_needs({str(f.relative_to(appdir)): n for f, n in deps.items()}, bundled, libs)
    if unchecked:
        raise SystemExit("hostlib: these libraries come from the host, and host-libs.json does not list them:\n"
                         + "".join(f"  {s} (linked by {', '.join(c)})\n" for s, c in unchecked.items()))

    with tempfile.TemporaryDirectory(prefix="hostlib-") as tmp_s:
        tmp = Path(tmp_s)
        gen = None
        for lib in (x for x in libs if x.kind == "optional"):
            for soname in lib.sonames:
                consumers = [f for f, n in deps.items() if soname in n]
                if not consumers:
                    print(f"hostlib: nothing links {soname}; no shim")
                    continue
                gen = gen or implib(tmp)
                real = host_copy(soname)
                provided = defined_symbols(real)
                imported = sorted(set().union(*(undefined_symbols(c) for c in consumers)) & provided)
                if not imported:
                    raise SystemExit(f"hostlib: {soname} is linked but nothing from it is imported")
                work = tmp / lib.name
                work.mkdir()
                (work / "symbols.txt").write_text("\n".join(imported) + "\n")
                run(sys.executable, gen, "--target", "x86_64", "--lazy-load", "--outdir", work,
                    "--dlopen-callback", "hostlib_dlopen_cb", "--dlsym-callback", "hostlib_dlsym_cb",
                    "--symbol-list", work / "symbols.txt", "--library-load-name", soname, real)
                arrays, init = spec_decl(lib, "spec")
                (work / "spec.h").write_text(arrays + f"static const struct hostlib_spec HOSTLIB_SPEC = {init};\n")
                # The consumers still list the version names they required from the real
                # library (VA_API_0.33.0, ...) against its new name: the shim defines them,
                # or the loader warns "no version information available" on every start.
                versions = sorted(set().union(*(required_versions(c, soname) for c in consumers)))
                (work / "versions.map").write_text("".join(f"{v} {{ }};\n" for v in versions))
                (work / "decls.h").write_text("".join(f"#include <{h}>\n" for h in lib.headers))
                (work / "stubs.h").write_text("".join(f"HOSTLIB_STUB({s})\n" for s in imported))
                target = shim_soname(soname, lib.name)
                status_fn = "obs_hostlib_" + lib.name.replace("-", "_") + "_available"
                objs = []
                for src in sorted(work.glob("*.S")) + sorted(work.glob("*.c")):
                    obj = work / (src.name + ".o")
                    # Exported: the shim is a library whose symbols the consumers bind to.
                    run(args.cc, "-O2", "-fPIC", "-DIMPLIB_EXPORT_SHIMS", "-c", src, "-o", obj)
                    objs.append(obj)
                for src, out in ((HERE / "hostlib.c", work / "hostlib.o"), (HERE / "shim.cpp", work / "shim.o")):
                    comp = args.cc if src.suffix == ".c" else args.cxx
                    std = ["-std=c11"] if src.suffix == ".c" else ["-std=c++17", "-fno-exceptions", "-fno-rtti"]
                    run(comp, *std, "-O2", "-fPIC", "-fvisibility=hidden", f"-I{HERE}", f"-I{work}",
                        f'-DHOSTLIB_SPEC_HEADER="{work / "spec.h"}"', f'-DHOSTLIB_STUBS_HEADER="{work / "stubs.h"}"',
                        f'-DHOSTLIB_DECLS_HEADER="{work / "decls.h"}"',
                        f"-DHOSTLIB_STATUS_FN={status_fn}", "-c", src, "-o", out)
                    objs.append(out)
                shim = libdir / target
                run(args.cxx, "-shared", "-o", shim, *objs, f"-Wl,-soname,{target}", "-Wl,--no-undefined",
                    *([f"-Wl,--version-script={work / 'versions.map'}"] if versions else []),
                    "-static-libstdc++", "-static-libgcc", "-ldl", "-lpthread")
                for c in consumers:
                    run("patchelf", "--replace-needed", soname, target, c)
                    for s in imported:
                        if s in undefined_symbols(c):
                            run("patchelf", "--clear-symbol-version", s, c)
                print(f"hostlib: {soname} -> {target} ({len(imported)} symbols, {len(consumers)} consumers)")

        req = [x for x in libs if x.kind == "required"]
        header = tmp / "required.h"
        decls, rows = [], []
        for n, l in enumerate(req):
            arrays, init = spec_decl(l, f"req{n}")
            decls.append(arrays)
            i = l.install
            rows.append(f'  {{{init}, "{i.debian}", "{i.fedora}", "{i.arch}", "{i.suse}", "{i.nixos}"}},\n')
        header.write_text("".join(decls) + "static const struct required_lib required[] = {\n" + "".join(rows) + "};\n")
        check = appdir / "usr/bin/obs-hostlib-check"
        run(args.cc, "-std=c11", "-O2", "-fPIE", "-pie", f"-I{HERE}", f'-DHOSTLIB_REQUIRED_HEADER="{header}"',
            HERE / "hostlib-check.c", HERE / "hostlib.c", "-o", check, "-ldl")
        print(f"hostlib: {check.relative_to(appdir)} checks {len(req)} required libraries")
    return 0


if __name__ == "__main__":
    sys.exit(main())
