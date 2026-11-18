#!/usr/bin/env -S uv run --script
"""Configure Tier 2, reusing only an unchanged, complete kept Ninja tree.

The stamp lives INSIDE tree/, so tier2_kept_tree.py moves it with the cache.
Ninja still owns source/CMake dependency and CONFIGURE_DEPENDS regeneration.
See docs/ci-tooling.md for the conservative input contract and version caveat.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys

STAMP = '.tier2-configure.json'
ENV_NAMES = {'PATH', 'HOME', 'CC', 'CXX', 'CFLAGS', 'CXXFLAGS', 'CPPFLAGS', 'LDFLAGS',
             'DISPLAY', 'LANG', 'LC_ALL', 'TZ', 'SDKROOT', 'MACOSX_DEPLOYMENT_TARGET',
             'CPATH', 'C_INCLUDE_PATH', 'CPLUS_INCLUDE_PATH', 'LIBRARY_PATH', 'LD_LIBRARY_PATH',
             'AS', 'AR', 'ASM', 'ASMFLAGS', 'CUDACXX', 'CUDAHOSTCXX', 'CUDAFLAGS', 'VIRTUAL_ENV'}
ENV_PREFIXES = ('CMAKE_', 'PKG_CONFIG', 'OBS_', 'SOLDR_', 'RUST', 'CARGO_', 'CCACHE_', 'ZCCACHE_', 'UV_', 'PYTHON')


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def installed_packages() -> str:
    # Tier 2 runs in Ubuntu, including on the NixOS Dell. Unsupported hosts
    # conservatively configure; a failed inventory never authorizes reuse.
    return subprocess.run(['dpkg-query', '-W', '-f=${Package}=${Version}\\n'],
                              capture_output=True, check=True, text=True).stdout


def option(args: list[str], flag: str) -> str | None:
    """Read -S/-B and -DNAME[:TYPE]=value without flattening argv boundaries."""
    value = None
    for index, arg in enumerate(args):
        if arg == flag and index + 1 < len(args):
            value = args[index + 1]
        elif flag.startswith('-D') and arg.split('=', 1)[0].split(':', 1)[0] == flag:
            value = arg.split('=', 1)[1] if '=' in arg else None
        elif flag in ('-S', '-B', '-C') and arg.startswith(flag) and arg != flag:
            value = arg[len(flag):]
    return value


def configure_tools(args: list[str]) -> dict:
    tools = {}
    commands = [args[0], 'ninja', 'cc', 'c++', 'soldr', 'pkg-config', 'git', 'uv', 'python3', 'ld', 'ar']
    for name in ('CC', 'CXX'):
        if os.environ.get(name):
            commands.append(shlex.split(os.environ[name])[0])
    for name in ('CMAKE_C_COMPILER', 'CMAKE_CXX_COMPILER', 'CMAKE_MAKE_PROGRAM',
                 'CMAKE_C_COMPILER_LAUNCHER', 'CMAKE_CXX_COMPILER_LAUNCHER'):
        value = option(args, '-D' + name) or os.environ.get(name)
        if value:
            commands.append(value.split(';')[0])
    for command in commands:
        resolved = shutil.which(command)
        tools[command] = [str(Path(resolved).resolve()), digest(Path(resolved))] if resolved else None
    return tools


def configure_files(args: list[str]) -> tuple[dict, str | None]:
    files = {str(Path(__file__).resolve()): digest(Path(__file__))}
    for flag in ('-DCMAKE_TOOLCHAIN_FILE', '-C'):
        if path := option(args, flag):
            files[path] = digest(Path(path))
    version = None
    if src := option(args, '-S'):
        root = Path(src)
        for name in ('rust-toolchain.toml', '.cargo/config.toml'):
            if (root / name).exists():
                files[str(root / name)] = digest(root / name)
        if (root / '.git').exists() and option(args, '-DOBS_VERSION_OVERRIDE') is None:
            version = subprocess.run(['git', '-C', src, 'describe', '--always', '--tags',
                                      '--match', '[0-9]*.[0-9]*.[0-9]*', '--dirty=-modified'],
                                     check=True, capture_output=True, text=True).stdout.strip()
    return files, version


def fingerprint(args: list[str]) -> str:
    """Hash argv, configure environment, installed tools/packages and git version.

    Do not include HEAD unconditionally: git describe's effective output is the
    version input. A source archive or a fixed OBS_VERSION_OVERRIDE needs no git.
    """
    # Preload scripts (and their arbitrary includes) and presets are not Ninja
    # regeneration dependencies. Preserve configure for these uncommon overrides.
    if any(arg.startswith(('-C', '--preset', '--fresh')) for arg in args):
        raise ValueError('preload scripts, presets and fresh configure cannot be reused')
    env = {key: value for key, value in os.environ.items()
           if key in ENV_NAMES or key.startswith(ENV_PREFIXES)}
    packages = installed_packages()
    files, version = configure_files(args)
    tools = configure_tools(args)
    payload = {'args': args, 'env': env, 'tools': tools, 'packages': packages,
               'files': files, 'version': version, 'cwd': str(Path.cwd())}
    return hashlib.sha256(json.dumps(payload, sort_keys=True).encode()).hexdigest()


def tree_state(build: Path) -> dict[str, str]:
    """Detect removed or externally modified cache/graph, including failed runs."""
    if not re.search(r'^build build\.ninja[^\n]*: RERUN_CMAKE', (build / 'build.ninja').read_text(), re.M):
        raise ValueError('Ninja regeneration is disabled')
    state = {name: digest(build / name) for name in ('CMakeCache.txt', 'build.ninja', 'CMakeFiles/rules.ninja')}
    for name, value in re.findall(r'^(CMAKE_(?:C_COMPILER|CXX_COMPILER|MAKE_PROGRAM|TOOLCHAIN_FILE)):[^=]+=(.+)$',
                                  (build / 'CMakeCache.txt').read_text(), re.M):
        state[name] = digest(Path(value))
    return state


def configure(build: Path, args: list[str]) -> bool:
    """Return True when configure ran. Failure/uncertainty never authorizes reuse."""
    stamp = build / STAMP
    key = None
    if os.environ.get('OBS_TIER2_KEEP_CONFIGURE') == '1' and not os.environ.get('OBS_LIBOBS_MODULE_CACHE_DIR'):
        try:
            key = fingerprint(args)
            recorded = json.loads(stamp.read_text())
            if recorded == {'input': key, 'tree': tree_state(build)}:
                print('== Tier 2 configure: matching kept tree; Ninja owns regeneration', flush=True)
                return False
        except (OSError, ValueError, subprocess.SubprocessError) as error:
            print(f'== Tier 2 configure: no reusable fingerprint ({type(error).__name__})', flush=True)
    # Invalidate BEFORE invoking CMake, including on hosted/module-cache paths.
    stamp.unlink(missing_ok=True)
    print('== Tier 2 configure: configure required', flush=True)
    subprocess.run(args, check=True)
    if key is not None:
        try:
            record = {'input': key, 'tree': tree_state(build)}
            temporary = stamp.with_suffix('.tmp')
            temporary.write_text(json.dumps(record, sort_keys=True) + '\n')
            temporary.replace(stamp)
        except (OSError, ValueError):
            pass  # An unwritable/incomplete tree cannot become a skip next time.
    return True


def main(argv: list[str]) -> int:
    build, separator, *args = argv
    if separator != '--' or not args:
        raise ValueError('usage: tier2_configure.py BUILD -- cmake ARGS...')
    try:
        configure(Path(build), args)
    except subprocess.CalledProcessError as error:
        return error.returncode
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
