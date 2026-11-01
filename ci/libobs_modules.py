#!/usr/bin/env -S uv run --script
"""The libobs lib-module cache (#3262): restore unchanged modules, store new ones.

    uv run ci/libobs_modules.py restore --build <dir> [--cache <dir>]
    uv run ci/libobs_modules.py store   --build <dir> [--cache <dir>]
    uv run ci/libobs_modules.py keys    --build <dir>

libobs/cmake/modules.cmake builds each module of libobs/build/modules.d as a
static archive, libobs-mod-<module> (#3261). A module's key is a blake2b
digest of everything that decides its objects:

- each of its groups' compile command, with the build and source dirs
  replaced by placeholders, so two build dirs of one checkout agree;
- the content of every file the group reads: its header closure, from the
  preprocessor (`-M -MG`, no compile), relative to the source or build dir;
- the compiler's `--version` banner.

`restore` runs on a configured tree (it reads compile_commands.json). For a
module whose `<cache>/<key>.a` exists, it copies the archive into
`<build>/libobs-modcache/` and lists it in `hits.cmake`; reconfiguring with
`-DOBS_LIBOBS_MODULE_CACHE=<build>/libobs-modcache` then imports it in place
of compiling it. `store` copies each built module archive to its key.

The cache dir defaults to `$OBS_LIBOBS_MODULE_CACHE_DIR`, else `modcache`
next to the zccache root (`$ZCCACHE_CACHE_DIR/..`), else
`~/.cache/obs-libobs-modules`.
"""

from __future__ import annotations

import argparse
import fnmatch
import hashlib
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MODULES_DIR = ROOT / "libobs" / "build" / "modules.d"
GROUP_RE = re.compile(r"libobs\.([^+/\\]+)\+\.h\.(c|m)$")
HITS_DIR = "libobs-modcache"
LAUNCHERS = ("zccache", "ccache", "sccache")


def default_cache() -> Path:
    if env := os.environ.get("OBS_LIBOBS_MODULE_CACHE_DIR"):
        return Path(env)
    if zc := os.environ.get("ZCCACHE_CACHE_DIR"):
        return Path(zc).parent / "modcache"
    return Path.home() / ".cache" / "obs-libobs-modules"


def module_patterns(modules_dir: Path = MODULES_DIR) -> dict[str, list[str]]:
    return {
        p.stem: json.loads(p.read_text(encoding="utf-8"))["groups"] for p in sorted(modules_dir.glob("*.json"))
    }


def module_of(group: str, patterns: dict[str, list[str]]) -> str | None:
    for name, globs in patterns.items():
        if any(fnmatch.fnmatchcase(group, g) for g in globs):
            return name
    return None


def arguments(entry: dict) -> list[str]:
    if "arguments" in entry:
        return list(entry["arguments"])
    return shlex.split(entry["command"])


def normalise(text: str, build: Path, source: Path) -> str:
    # Longest first: the build dir often sits inside the source dir.
    for path, tag in sorted(((str(build), "<B>"), (str(source), "<S>")), key=lambda x: -len(x[0])):
        text = text.replace(path, tag)
    # A cache hit compiles nothing; its unbuilt probe target carries the
    # same commands under another name (libobs/cmake/modules.cmake).
    return text.replace("-keyprobe", "")


def compile_args(args: list[str]) -> list[str]:
    """The compile command without its output and dependency-file flags."""
    out: list[str] = []
    skip = False
    # A launcher (zccache, ccache) does not change the objects.
    while args and Path(args[0]).name in LAUNCHERS:
        args = args[1:]
    for arg in args:
        if skip:
            skip = False
            continue
        if arg in ("-o", "-MF", "-MT", "-MQ"):
            skip = True
            continue
        if arg in ("-c", "-MD", "-MMD") or arg.startswith(("-o", "/Fo", "/Fd", "-MF")):
            continue
        out.append(arg)
    return out


def header_closure(entry: dict) -> list[Path]:
    """Every file the group reads, from the preprocessor's dependency list."""
    args = compile_args(arguments(entry))
    if Path(args[0]).stem.lower() in ("cl", "clang-cl"):
        raise SystemExit("libobs_modules.py: MSVC compile commands are not supported yet")
    done = subprocess.run(
        [*args, "-M", "-MG"], cwd=entry["directory"], capture_output=True, text=True, check=False
    )
    if done.returncode != 0:
        raise SystemExit(f"libobs_modules.py: dependency scan failed for {entry['file']}:\n{done.stderr}")
    deps = done.stdout.replace("\\\n", " ").split(":", 1)[1].split()
    base = Path(entry["directory"])
    return sorted({(base / d).resolve() for d in deps})


def compiler_banner(compiler: str, cache: dict[str, str]) -> str:
    if compiler not in cache:
        done = subprocess.run([compiler, "--version"], capture_output=True, text=True, check=False)
        cache[compiler] = done.stdout.splitlines()[0] if done.stdout else compiler
    return cache[compiler]


def module_entries(build: Path) -> dict[str, list[dict]]:
    patterns = module_patterns()
    entries = json.loads((build / "compile_commands.json").read_text(encoding="utf-8"))
    out: dict[str, list[dict]] = {}
    for entry in entries:
        file = entry["file"].replace("\\", "/")
        match = GROUP_RE.search(file)
        if not match or "/libobs/build/" not in file:
            continue
        # The explicit C targets compile the ported groups again; only the
        # module libraries' commands belong to a module.
        if "libobs-mod-" not in entry.get("output", "") + entry.get("command", "") + " ".join(
            entry.get("arguments", [])
        ):
            continue
        module = module_of(match.group(1), patterns)
        if module:
            out.setdefault(module, []).append(entry)
    return out


def keys(build: Path, source: Path = ROOT) -> dict[str, str]:
    banners: dict[str, str] = {}
    out = {}
    for module, entries in sorted(module_entries(build).items()):
        digest = hashlib.blake2b(digest_size=20)
        digest.update(f"module {module}\n".encode())
        for entry in sorted(entries, key=lambda e: e["file"]):
            args = compile_args(arguments(entry))
            digest.update(compiler_banner(args[0], banners).encode() + b"\n")
            digest.update(normalise(" ".join(args), build, source).encode() + b"\n")
            for path in header_closure(entry):
                digest.update(normalise(str(path), build, source).encode() + b"\0")
                digest.update(path.read_bytes() if path.is_file() else b"<missing>")
                digest.update(b"\n")
        out[module] = digest.hexdigest()
    return out


RUST_JSON = "libobs-modcache-rust.json"
RUST_CRATE = "libobs-rust"
# Files outside the linked crates that their build scripts read.
RUST_EXTRA_INPUTS = ("libobs/obs-hotkeys.h", "rust/obs-c-oracle/vendor/jansson/src")
# Crate files that never reach the staticlib.
RUST_SKIP_DIRS = {"tests", "benches", "examples", "proptest-regressions", "target"}


def manifest_deps(manifest: dict, workspace: dict) -> list[str]:
    """Path dependencies (normal and build, any target) of one Cargo.toml."""
    tables = [manifest.get("dependencies", {}), manifest.get("build-dependencies", {})]
    for target in manifest.get("target", {}).values():
        tables += [target.get("dependencies", {}), target.get("build-dependencies", {})]
    paths = []
    for table in tables:
        for name, spec in table.items():
            if isinstance(spec, dict) and spec.get("workspace"):
                spec = workspace.get(spec.get("package", name), {})
            if isinstance(spec, dict) and "path" in spec:
                paths.append(spec["path"])
    return paths


def linked_crate_dirs(source: Path = ROOT) -> list[Path]:
    """The path crates libobs-rust builds with, from the Cargo.toml files.

    Registry crates are pinned by Cargo.lock, which is keyed whole. A crate
    listed for a target or feature the build does not use only makes the
    key stricter.
    """
    workspace = tomllib.loads((source / "Cargo.toml").read_text(encoding="utf-8"))
    ws_deps = {
        name: {**spec, "path": str(source / spec["path"])} if isinstance(spec, dict) and "path" in spec else spec
        for name, spec in workspace.get("workspace", {}).get("dependencies", {}).items()
    }
    seen: set[Path] = set()
    todo = [source / "rust" / RUST_CRATE]
    while todo:
        crate = todo.pop().resolve()
        if crate in seen:
            continue
        seen.add(crate)
        manifest = tomllib.loads((crate / "Cargo.toml").read_text(encoding="utf-8"))
        todo += [crate / dep for dep in manifest_deps(manifest, ws_deps)]
    return sorted(seen)


def tree_files(path: Path) -> list[Path]:
    if path.is_file():
        return [path]
    return sorted(
        f for f in path.rglob("*")
        if f.is_file() and not RUST_SKIP_DIRS.intersection(f.relative_to(path).parts)
    )


def tool_line(args: list[str]) -> str:
    try:
        done = subprocess.run(args, capture_output=True, text=True, check=False)
    except FileNotFoundError:
        return "<absent>"
    return done.stdout.strip() if done.returncode == 0 else "<absent>"


def rust_key(build: Path, source: Path = ROOT, crate_dirs: list[Path] | None = None) -> str | None:
    """The staticlib's key, or None when the tree builds no Rust libobs."""
    config_file = build / RUST_JSON
    if not config_file.is_file():
        return None
    config = json.loads(config_file.read_text(encoding="utf-8"))
    config.pop("location", None)
    config["cc"] = Path(config.get("cc", "")).name
    digest = hashlib.blake2b(digest_size=20)
    digest.update(json.dumps(config, sort_keys=True).encode() + b"\n")
    rustc = os.environ.get("RUSTC") or ("soldr" if shutil.which("soldr") else "rustc")
    digest.update(tool_line([rustc, "rustc", "-vV"] if rustc == "soldr" else [rustc, "-vV"]).encode())
    digest.update(tool_line(["pkg-config", "--modversion", "gio-2.0"]).encode())
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CFLAGS"):
        digest.update(f"{name}={os.environ.get(name, '')}\n".encode())
    inputs = [source / "Cargo.lock", source / "Cargo.toml", source / "rust-toolchain.toml"]
    inputs += [source / extra for extra in RUST_EXTRA_INPUTS]
    for crate in crate_dirs if crate_dirs is not None else linked_crate_dirs(source):
        inputs.append(crate)
    for top in inputs:
        for file in tree_files(top) if top.exists() else []:
            digest.update(file.relative_to(source).as_posix().encode() + b"\0")
            digest.update(file.read_bytes())
    return digest.hexdigest()


def rust_location(build: Path) -> Path | None:
    config_file = build / RUST_JSON
    if not config_file.is_file():
        return None
    return Path(json.loads(config_file.read_text(encoding="utf-8"))["location"])


def built_archive(build: Path, module: str) -> Path | None:
    for pattern in (f"liblibobs-mod-{module}.a", f"libobs-mod-{module}.lib"):
        found = sorted(build.rglob(pattern))
        if found:
            return found[0]
    return None


def restore(build: Path, cache: Path) -> tuple[list[str], list[str]]:
    hits_dir = build / HITS_DIR
    hits_dir.mkdir(parents=True, exist_ok=True)
    hits: list[str] = []
    misses: list[str] = []
    lines: list[str] = []
    for module, key in keys(build).items():
        stored = cache / f"{key}.a"
        if stored.is_file():
            local = hits_dir / f"{module}.a"
            shutil.copyfile(stored, local)
            lines.append(f'set(LIBOBS_MODULE_HIT_{module} "{local.as_posix()}")')
            hits.append(module)
        else:
            misses.append(module)
    key = rust_key(build)
    if key and (cache / f"{key}.rust.a").is_file():
        local = hits_dir / "libobs-rust.a"
        shutil.copyfile(cache / f"{key}.rust.a", local)
        lines.append(f'set(LIBOBS_RUST_HIT "{local.as_posix()}")')
        hits.append(RUST_CRATE)
    elif key:
        misses.append(RUST_CRATE)
    (hits_dir / "hits.cmake").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return hits, misses


def store(build: Path, cache: Path, prune: bool = False) -> list[str]:
    """Copies each built module archive to its key. With `prune`, removes
    every other archive, so a cache saved per commit stays one set."""
    cache.mkdir(parents=True, exist_ok=True)
    stored = []
    module_keys = keys(build)
    if prune:
        keep = {f"{key}.a" for key in module_keys.values()}
        for old in cache.glob("*.a"):
            if old.name not in keep:
                old.unlink()
    for module, key in module_keys.items():
        target = cache / f"{key}.a"
        if target.is_file():
            continue
        archive = built_archive(build, module)
        if archive is None or HITS_DIR in archive.parts:
            continue
        tmp = target.with_suffix(f".tmp{os.getpid()}")
        shutil.copyfile(archive, tmp)
        tmp.replace(target)
        stored.append(module)
    key = rust_key(build)
    archive = rust_location(build)
    target = cache / f"{key}.rust.a"
    if key and archive and archive.is_file() and not target.is_file():
        tmp = target.with_suffix(f".tmp{os.getpid()}")
        shutil.copyfile(archive, tmp)
        tmp.replace(target)
        stored.append(RUST_CRATE)
    return stored


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("command", choices=("keys", "restore", "store"))
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--cache", type=Path, default=None)
    parser.add_argument("--prune", action="store_true", help="store: keep only the current keys' archives")
    args = parser.parse_args(argv)
    build = args.build.resolve()
    cache = (args.cache or default_cache()).resolve()
    if args.command == "keys":
        for module, key in keys(build).items():
            print(f"{module} {key}")
        print(f"{RUST_CRATE} {rust_key(build)}")
    elif args.command == "restore":
        hits, misses = restore(build, cache)
        print(f"libobs modules: {len(hits)} restored ({' '.join(hits)}), {len(misses)} to build ({' '.join(misses)})")
        print(f"reconfigure with -DOBS_LIBOBS_MODULE_CACHE={(build / HITS_DIR).as_posix()}")
    else:
        stored = store(build, cache, prune=args.prune)
        print(f"libobs modules: {len(stored)} stored in {cache} ({' '.join(stored)})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
