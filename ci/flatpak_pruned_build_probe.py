#!/usr/bin/env -S uv run --script
"""Opt-in isolated rebuild without downloaded sources or compiler cache."""

from __future__ import annotations

import argparse
import configparser
from itertools import chain
import json
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from urllib.parse import quote

MARKER = 'OBS_PRUNED_REBUILD_EXECUTED'
REUSE_MARKER = 'OBS_COMPILER_REUSE_EXECUTED'
EXCEPTIONS = Path(__file__).resolve().parent.parent / '.github/actions/flatpak-builder-lint/exceptions.json'


def run(command, seconds):
    bounded = ['timeout', '--kill-after=10s', str(seconds) + 's', *map(str, command)]
    print('+ ' + shlex.join(bounded), file=sys.stderr)
    result = subprocess.run(bounded, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    print(result.stdout, end='', file=sys.stderr)
    result.check_returncode()
    return result.stdout


def cache_branch(manifest):
    # Match flatpak-builder1.4.9's g_uri_escape_string(..., allow_utf8=TRUE)
    # branch under --state-dir. Keep UTF-8 characters, escape ASCII punctuation.
    identity = 'x86_64-' + str(Path(manifest).resolve())[1:]
    return ''.join(char if ord(char) > 127 else quote(char, safe='') for char in identity).replace('%', '_')


def measure(state, manifest, temp_root, execute=run, *, compiler_reuse=False):
    state = Path(state).resolve(strict=True)
    manifest = Path(manifest).resolve(strict=True)
    scratch = Path(temp_root).resolve(strict=True)
    if scratch == state or state in scratch.parents:
        raise ValueError('temporary state must be outside original state')
    recipe = json.loads(manifest.read_text())
    modules = recipe['modules']
    if not modules or modules[-1].get('name') != 'obs':
        raise ValueError('expected final application module obs')
    binary = recipe['command']
    if not isinstance(binary, str) or not binary or Path(binary).name != binary:
        raise ValueError('expected application command basename')
    dependencies = [module['name'] for module in modules[:-1]]
    modules[-1].setdefault('post-install', []).extend([
        'test -x ' + shlex.quote('/app/bin/' + binary),
        "printf '%s\\n' " + MARKER,
    ])
    # Only the dependency OSTree repository is a transport input. Build/checksum
    # scaffolding is regenerated and may contain transient checkout links.
    retained = [state / 'cache']
    if not (state / 'cache').is_dir():
        raise ValueError('expected populated dependency cache')
    symbolic_objects = 0
    for path in retained:
        for child in chain([path], path.rglob('*')):
            if not child.is_symlink():
                continue
            relative = child.relative_to(state).as_posix()
            # OSTree bare repositories may encode symlink payloads as link
            # objects. Preserve those links as data; never follow them. All
            # repository control paths must remain ordinary files/directories.
            if not re.fullmatch(r'cache/objects/[0-9a-f]{2}/[0-9a-f]{62}\.file', relative):
                raise ValueError('retained dependency cache contains structural symbolic link: ' + relative[:250])
            symbolic_objects += 1
    reuse_report = None
    # Keeping the temporary manifest beside its source preserves relative paths.
    # Both contexts clean up even when a build or validation fails.
    with tempfile.TemporaryDirectory(prefix='flatpak-pruned-build-', dir=scratch) as directory:
        with tempfile.NamedTemporaryFile(mode='w', prefix='flatpak-pruned-', suffix='.json', dir=manifest.parent) as prepared:
            json.dump(recipe, prepared)
            prepared.flush()
            root = Path(directory)
            isolated = root / 'state'
            isolated.mkdir()
            for source in retained:
                destination = isolated / source.name
                if source.is_dir():
                    shutil.copytree(source, destination, symlinks=True)
                else:
                    shutil.copy2(source, destination)
            # The prepared manifest changes only the final application's checksum,
            # but its path also changes the repository's branch namespace. Alias
            # the original stage refs in this private copy before validating it.
            heads = isolated / 'cache/refs/heads'
            source_branch = cache_branch(manifest)
            probe_branch = cache_branch(prepared.name)
            source_refs = heads / source_branch
            if not source_refs.is_dir():
                raise ValueError('original manifest dependency refs missing: ' + source_branch)
            shutil.copytree(source_refs, heads / probe_branch, symlinks=True)
            ostree_version = execute(['ostree', '--version'], 30).strip()
            configuration = configparser.ConfigParser()
            configuration.read(isolated / 'cache/config')
            repository_mode = configuration.get('core', 'mode', fallback='unknown')
            fsck_start = time.monotonic()
            execute(['ostree', '--repo=' + str(isolated / 'cache'), 'fsck'], 120)
            fsck_seconds = time.monotonic() - fsck_start
            build, repo = root / 'build', root / 'repo'
            start = time.monotonic()
            build_command = [
                'xvfb-run', '--auto-servernum', 'flatpak-builder',
                '--disable-rofiles-fuse', '--install-deps-from=flathub', '--force-clean',
                '--default-branch=master', '--arch=x86_64', '--ccache',
                '--mirror-screenshots-url=https://dl.flathub.org/media', '--compose-url-policy=full',
                '--repo=' + str(repo), '--state-dir', str(isolated), str(build), prepared.name,
            ]
            output = execute(build_command, 1800)
            elapsed = time.monotonic() - start
            if MARKER not in output.splitlines() or 'Cache hit for obs, skipping build' in output:
                raise ValueError('application rebuild signal missing or application was cached')
            for name in dependencies:
                if 'Cache hit for ' + name + ', skipping build' not in output:
                    raise ValueError('dependency module reuse missing: ' + name)
            def validate_output():
                for relative in (
                    'bin/' + binary,
                    'share/app-info/icons/flatpak/128x128/' + recipe['id'] + '.png',
                    'share/app-info/xmls/' + recipe['id'] + '.xml.gz',
                ):
                    if not (build / 'files' / relative).is_file():
                        raise ValueError('isolated output missing: ' + relative)
                # Match the pinned action's screenshot publication inside this repo.
                execute(['ostree', 'commit', '--repo=' + str(repo), '--canonical-permissions',
                         '--branch=screenshots/x86_64', str(build / 'files/share/app-info/media')], 120)
                for kind, path in (
                    ('appstream', build / 'files/share/metainfo' / (recipe['id'] + '.metainfo.xml')),
                    ('builddir', build), ('repo', repo),
                ):
                    execute(['flatpak-builder-lint', '--exceptions', '--user-exceptions', str(EXCEPTIONS), kind, str(path)], 120)
            validate_output()
            if compiler_reuse:
                compiler_cache = isolated / 'ccache'
                if not compiler_cache.is_dir():
                    raise ValueError('isolated cold build did not create compiler cache')
                sdk = recipe['sdk'] + '//' + recipe['runtime-version']
                ccache = ['flatpak', 'run', '--command=ccache', '--filesystem=' + str(compiler_cache),
                          '--env=CCACHE_DIR=' + str(compiler_cache), sdk]
                compiler_version = execute([*ccache, '--version'], 30).strip()
                execute([*ccache, '--zero-stats'], 30)
                # Only the post-install marker changes. This forces the OBS
                # module to rebuild with identical source/toolchain/configuration.
                modules[-1]['post-install'].append("printf '%s\\n' " + REUSE_MARKER)
                prepared.seek(0)
                json.dump(recipe, prepared)
                prepared.truncate()
                prepared.flush()
                warm_start = time.monotonic()
                warm_output = execute(build_command, 1800)
                warm_seconds = time.monotonic() - warm_start
                if REUSE_MARKER not in warm_output.splitlines() or 'Cache hit for obs, skipping build' in warm_output:
                    raise ValueError('warm compiler rebuild signal missing or application was cached')
                for name in dependencies:
                    if 'Cache hit for ' + name + ', skipping build' not in warm_output:
                        raise ValueError('warm compiler dependency reuse missing: ' + name)
                stats = dict(line.split() for line in execute([*ccache, '--print-stats'], 30).splitlines()
                             if len(line.split()) == 2)
                hits = int(stats.get('direct_cache_hit', 0)) + int(stats.get('preprocessed_cache_hit', 0))
                misses = int(stats.get('cache_miss', 0))
                # The measured app has 330 cacheable compilations. A handful
                # of configure checks cannot qualify full application reuse.
                if hits < 100 or hits / max(1, hits + misses) < 0.8:
                    raise ValueError(f'insufficient full application compiler reuse: {hits} hits, {misses} misses')
                validate_output()
                reuse_report = {'compiler': compiler_version, 'hits': hits, 'misses': misses, 'stats': stats,
                                'isolated_build_seconds': warm_seconds, 'application_rebuilt': True,
                                'input_cache': 'generated by the isolated cold rebuild, not copied from original state',
                                'limits': 'One same-source warm rebuild; not transport/retention or source-change freshness proof. '
                                          'SDK queries and output validation excluded from this build timer.'}
    return {
        'status': 'success', 'architecture': 'x86_64', 'application_rebuilt': True,
        'ostree_version': ostree_version, 'source_repository_mode': repository_mode,
        'copied_symbolic_objects': symbolic_objects, 'repository_fsck_seconds': fsck_seconds,
        'source_cache_branch': source_branch, 'probe_cache_branch': probe_branch,
        'reused_dependency_modules': dependencies, 'isolated_build_seconds': elapsed,
        'compiler_reuse': reuse_report,
        'retained_state': ['cache'], 'omitted_state': ['downloads', 'ccache', 'transient builder state'],
        'limits': ('Two isolated application rebuilds with output validation, ' if compiler_reuse else
                   'One isolated application rebuild with output validation, ') +
                  'not runtime/profile validation, Actions cache transport savings, representative cost or production adoption. '
                  'Copy/setup, repository fsck and validation excluded from build timer; original state only read (may affect atime).',
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--state', type=Path, default=Path('.flatpak-builder'))
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--temp-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--compiler-reuse', action='store_true', help='also force a warm app rebuild using the isolated compiler results')
    args = parser.parse_args()
    try:
        result = measure(args.state, args.manifest, args.temp_root, compiler_reuse=args.compiler_reuse)
    except (OSError, ValueError, KeyError, configparser.Error, subprocess.CalledProcessError) as error:
        args.output.write_text(json.dumps({'status': 'failed', 'error': str(error)}, indent=2) + '\n')
        raise
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
