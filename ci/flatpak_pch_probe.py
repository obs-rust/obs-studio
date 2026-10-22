#!/usr/bin/env -S uv run --script
"""Exercise PCH cache freshness; isolated fixture, not full Flatpak evidence."""
import argparse
import hashlib
import json
import os
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--output', required=True, type=Path, help='new directory for isolated cache and retained evidence')
args = parser.parse_args()
repo = Path(__file__).resolve().parent.parent
manifest_path = repo / 'build-aux/com.obsproject.Studio.json'
manifest = json.loads(manifest_path.read_text())
obs = next(module for module in manifest['modules'] if module['name'] == 'obs')
root = args.output.resolve()
root.mkdir(parents=True, exist_ok=False)
source = root / 'source'
source.mkdir()
build = root / 'build'
compiler = root / 'compiler'
compiler.write_text('#!/bin/sh\nexec /usr/bin/c++ -DTOOLCHAIN_VALUE=1 "$@"\n')
compiler.chmod(0o755)
header = source / 'value.hpp'
header.write_text('#define VALUE 42\n')
(source / 'pch.hpp').write_text('#pragma once\n#include <vector>\n#include <map>\n#include <string>\n#include <algorithm>\n#include "value.hpp"\n')
main = source / 'main.cpp'
main.write_text('#include <iostream>\n#ifndef MULTIPLIER\n#define MULTIPLIER 1\n#endif\nint main() { std::cout << VALUE * MULTIPLIER * TOOLCHAIN_VALUE + 0 << "\\n"; }\n')
(source / 'CMakeLists.txt').write_text('''cmake_minimum_required(VERSION 3.28)
project(flatpak_pch_fixture CXX)
set(_OBS_BUILD_SPEED_DEFERRED TRUE)
include("@REPO@/cmake/common/ccache.cmake")
include("@REPO@/cmake/common/pch.cmake")
add_executable(probe main.cpp)
target_precompile_headers(probe PRIVATE pch.hpp)
if(DEFINED MULTIPLIER)
 target_compile_definitions(probe PRIVATE MULTIPLIER=${MULTIPLIER})
endif()
_obs_pch_ccache(probe)
'''.replace('@REPO@', repo.as_posix()))
env = {k: v for k, v in os.environ.items() if not k.startswith('CCACHE_')}
env.update({k: v for k, v in obs['build-options']['env'].items() if k.startswith('CCACHE_')})
env.update(CCACHE_DIR=str(root / 'cache'), CCACHE_CONFIGPATH='/dev/null')
def run(args):
    try:
        return subprocess.check_output(args, env=env, stderr=subprocess.STDOUT, text=True, timeout=120)
    except subprocess.CalledProcessError as error:
        with (root / 'failed-command.log').open('a') as log:
            log.write(json.dumps({'argv': args, 'returncode': error.returncode}) + '\n')
            log.write(error.output)
        raise
def replace_preserving_times(path, before, after):
    stat = path.stat()
    assert len(before) == len(after)
    text = path.read_text()
    assert before in text
    path.write_text(text.replace(before, after))
    os.utime(path, ns=(stat.st_atime_ns, stat.st_mtime_ns))
configure = ['cmake', '-S', str(source), '-B', str(build), '-G', 'Ninja', f'-DCMAKE_CXX_COMPILER={compiler}', next((option for option in obs['config-opts'] if option.startswith('-DENABLE_CCACHE=')), '-DENABLE_CCACHE=OFF')]
(root / 'configure.log').write_text(run(configure))
report = {'scope': __doc__, 'manifest_sha256': hashlib.sha256(manifest_path.read_bytes()).hexdigest(), 'compiler': run(['/usr/bin/c++', '--version']), 'ccache': run(['ccache', '--version']), 'trials': []}
trials = [('cold', 42), ('warm', 42), ('header-same-stat', 43), ('header-warm', 43), ('source-same-stat', 44), ('source-warm', 44), ('configuration', 87), ('configuration-warm', 87), ('compiler-same-stat', 173), ('compiler-warm', 173)]
for name, expected in trials:
    if name == 'header-same-stat':
        replace_preserving_times(header, '42', '43')
    if name == 'source-same-stat':
        replace_preserving_times(main, '+ 0', '+ 1')
    if name == 'configuration':
        run([*configure, '-DMULTIPLIER=2'])
    if name == 'compiler-same-stat':
        replace_preserving_times(compiler, 'TOOLCHAIN_VALUE=1', 'TOOLCHAIN_VALUE=2')
    if name != 'cold':
        run(['cmake', '--build', str(build), '--target', 'clean'])
    run(['ccache', '--zero-stats'])
    start = time.monotonic()
    log = run(['cmake', '--build', str(build), '--parallel', '2'])
    seconds = time.monotonic() - start
    actual = int(run([str(build / 'probe')]).strip())
    stats = dict(line.split() for line in run(['ccache', '--print-stats']).splitlines() if len(line.split()) == 2)
    (root / f'{name}.log').write_text(log)
    row = dict(trial=name, expected=expected, actual=actual, seconds=seconds, stats=stats)
    report['trials'].append(row)
    (root / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(row), flush=True)
    assert actual == expected, row
    hits = int(stats['direct_cache_hit']) + int(stats['preprocessed_cache_hit'])
    if name.endswith('warm'):
        assert hits == 2, row
    elif name != 'source-same-stat':
        assert hits == 0, row
