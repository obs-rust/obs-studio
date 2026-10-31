#!/usr/bin/env -S uv run --script
"""Derive exact installed runtime packages from a trusted Tier 2 build tree.

ELF dependencies include transitive libraries; explicit tools retain services,
software graphics drivers and VLC plugins loaded by name rather than DT_NEEDED.
Missing libraries or ownership metadata fail the producer instead of shrinking
coverage. This is a package manifest, never a test attestation.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import subprocess

# Interpreter package used by fixture scripts; project commands use uv run.
PYTHON_RUNTIME_PACKAGE = 'python3'  # uv run supplies project execution.
RUNTIME_TOOLS = (
    # The unchanged manifest-source CTest configures and compiles a C fixture.
    'cmake', 'ninja-build', 'gcc', 'libc6-dev', PYTHON_RUNTIME_PACKAGE, 'git', 'file', 'zstd', 'xvfb', 'xauth',
    # Tests dlopen the unversioned libvlc.so name provided by this package.
    'libvlc-dev',
    'pipewire', 'dbus-daemon', 'vlc-plugin-base', 'libgl1-mesa-dri', 'libegl-mesa0', 'libglx-mesa0',
    # test_v4l2_vmic_server starts a private pipewire-pulse and records with pactl/pw-record (#3186).
    'pipewire-pulse', 'wireplumber', 'pulseaudio-utils',
)
PACKAGE = re.compile(r'[a-z0-9][a-z0-9+.-]*(?::[a-z0-9][a-z0-9-]*)?\Z')
VERSION = re.compile(r'[0-9][a-zA-Z0-9.+:~_-]*\Z')


def library_paths(text: str) -> set[str]:
    if 'not found' in text:
        raise ValueError(f'Unresolved runtime library: {text.strip()}')
    result = set()
    for line in text.splitlines():
        match = re.search(r'=> (/.*?) \(0x[0-9a-fA-F]+\)', line)
        if not match:
            match = re.match(r'\s*(/.*?) \(0x[0-9a-fA-F]+\)', line)
        if match:
            result.add(match[1])
    return result


def package_names(text: str) -> set[str]:
    names = set()
    for line in text.splitlines():
        if re.fullmatch(r'diversion by [a-z0-9][a-z0-9+.:~-]* (?:from|to): /[^\n]+', line):
            continue
        owner, separator, _ = line.partition(': /')
        if not separator or not PACKAGE.fullmatch(owner):
            raise ValueError(f'Unrecognized package ownership: {line!r}')
        names.add(owner)
    if not text.strip():
        raise ValueError('Empty package ownership')
    return names


def pinned_package(name: str, version: str) -> str:
    if not PACKAGE.fullmatch(name) or not VERSION.fullmatch(version):
        raise ValueError(f'Invalid package metadata: {name!r}, {version!r}')
    return f'{name}={version}'


def elf_files(root: Path):
    for path in sorted(root.rglob('*')):
        if 'cargo' in path.relative_to(root).parts or path.suffix in {'.o', '.a', '.rlib', '.rmeta'}:
            continue
        if path.is_symlink() or not path.is_file():
            continue
        with path.open('rb') as stream:
            if stream.read(4) == b'\x7fELF':
                yield path


def query(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run(args, capture_output=True, text=True, env={**os.environ, 'LC_ALL': 'C'})


def owners(path: str) -> set[str]:
    for candidate in dict.fromkeys((path, str(Path(path).resolve()))):
        result = query('dpkg-query', '-S', candidate)
        if result.returncode == 0:
            names = package_names(result.stdout)
            if names:
                return names
    raise ValueError(f'No installed package owns runtime library {path}')


def ownership_packages(paths: list[str]) -> set[str]:
    if not paths:
        return set()
    # dpkg reads the same ownership database for every request. Resolve normal
    # paths once, retaining the original per-path alias/diversion fallback.
    result = query('dpkg-query', '-S', *paths)
    if result.returncode not in (0, 1):
        raise ValueError(f'Package ownership query failed: {result.stderr.strip()}')
    records = {}
    for line in result.stdout.splitlines():
        names = package_names(line)
        if names:
            _, _, suffix = line.partition(': /')
            records.setdefault('/' + suffix, set()).update(names)
    packages = set()
    for path in paths:
        packages.update(records.get(path) or owners(path))
    return packages


def collect(root: Path) -> list[str]:
    root = root.resolve(strict=True)
    paths = set()
    binaries = list(elf_files(root))
    if not binaries:
        raise ValueError(f'No runtime ELF files under {root}')
    env = {**os.environ, 'LC_ALL': 'C',
           'LD_LIBRARY_PATH': f'{root}/libobs:{root}/libobs-c-reference'}
    for binary in binaries:
        result = subprocess.run(['ldd', str(binary)], capture_output=True, text=True, env=env)
        output = result.stdout + result.stderr
        if result.returncode and not any(marker in output for marker in ('not a dynamic executable', 'statically linked')):
            raise ValueError(f'ldd failed for {binary}: {output.strip()}')
        paths.update(library_paths(output))
    packages = set(RUNTIME_TOOLS)
    external = [path for path in sorted(paths) if not Path(path).resolve().is_relative_to(root)]
    packages.update(ownership_packages(external))
    pinned = []
    for package in sorted(packages):
        result = query('dpkg-query', '-W', '-f=${Version}', package)
        if result.returncode:
            raise ValueError(f'Runtime package is not installed: {package}')
        pinned.append(pinned_package(package, result.stdout.strip()))
    return pinned


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('tree', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    try:
        packages = collect(args.tree)
    except (ValueError, OSError) as error:
        parser.exit(1, f'Runtime package manifest: {error}\n')
    args.output.write_text('\n'.join(packages) + '\n')
    print(f'Runtime package manifest: {len(packages)} exact package versions')


if __name__ == '__main__':
    main()
