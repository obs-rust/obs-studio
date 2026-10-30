#!/usr/bin/env -S uv run --no-project --script
"""The macOS analysis lane is one Python call per step (#3040, GEN-005).

`.github/workflows/analyze-project.yaml`'s macOS job carried three multi-line
zsh `run:` steps -- *Set Up Environment*, *Build OBS Studio* and *Compile
Analytics Data* -- so the workflow produced a GEN-005 approved exception for
each, and the disabled PVS-Studio Windows job carried a fourth (a
`defaults.run.shell: pwsh` that no step used, since every step is `uses:`).
Each step is now a single call:

    uv run --no-project --script ci/analyze_project.py setup
    uv run --no-project --script ci/analyze_project.py build
    uv run --no-project --script ci/analyze_project.py merge-sarif

`setup` switches Xcode, uninstalls the Homebrew formulas in
`UNWANTED_FORMULAS` (currently none -- the step's list is empty upstream),
creates the DerivedData compilation-cache directory and writes
`xcodeCasPath=` to `$GITHUB_OUTPUT`, which the build step's `XCODE_CAS_PATH`
reads.

`build` assembles the analyzer argv (`--config Debug --target macos-arm64
--codesign --analyze`, plus `--debug` when `RUNNER_DEBUG` is set), refreshes
`origin` and runs `.github/scripts/build-macos` with it.

`merge-sarif` flattens `analytics/StaticAnalyzer/obs-studio/**/*.plist` to
`analytics/<name>.sarif` and merges them into `analytics/merged.sarif`: the
first document's `$schema`/`version`, the first `tool.driver` without its
`rules`, the concatenated de-duplicated `rules` and `artifacts`, every
`results` entry, and `codeFlows[].threadFlows[].locations[].location
.physicalLocation.region.endLine`/`endColumn` dropped where it is `0` (LLVM
writes zero-sized regions there and the codeql uploader rejects them). This is
the previous `jq -s` program, kept equivalent by
`ci/test_analyze_project.py`.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[1]
ANALYTICS = ROOT / 'analytics'
BUILD_SCRIPT = ROOT / '.github' / 'scripts' / 'build-macos'
XCODE = '/Applications/Xcode_26.5.app/Contents/Developer'
UNWANTED_FORMULAS: tuple[str, ...] = ()
CAS_RELATIVE = Path('Library/Developer/Xcode/DerivedData/CompilationCache.noindex')
MERGED = 'merged.sarif'

Runner = Callable[[list[str], Path | None], int]


def run_command(argv: list[str], cwd: Path | None = None) -> int:
    """Run a command, echoing it the way `setopt XTRACE` did for RUNNER_DEBUG."""

    print('+ ' + ' '.join(argv), flush=True)
    return subprocess.run(argv, cwd=cwd, check=False).returncode


def write_output(output: Path | None, values: dict[str, str]) -> None:
    """Append `key=value` lines to `$GITHUB_OUTPUT`, or print them when unstamped."""

    text = ''.join(f'{key}={value}\n' for key, value in values.items())
    if output is None or not str(output):
        print(text, end='', flush=True)
        return
    with output.open('a', encoding='utf-8') as handle:
        handle.write(text)


def brew_prefix(environ: dict[str, str]) -> Path:
    return Path(environ.get('HOMEBREW_PREFIX') or '/opt/homebrew')


def setup(
    run: Runner = run_command,
    environ: dict[str, str] | None = None,
    output: Path | None = None,
) -> int:
    """Switch Xcode, clean Homebrew, create the compilation cache, emit its path."""

    environment = dict(os.environ if environ is None else environ)
    destination = Path(environment['GITHUB_OUTPUT']) if output is None and environment.get('GITHUB_OUTPUT') else output

    print('::group::Enable Xcode 26.5', flush=True)
    status = run(['sudo', 'xcode-select', '--switch', XCODE], None)
    print('::endgroup::', flush=True)
    if status != 0:
        return status

    print('::group::Clean Homebrew Environment', flush=True)
    prefix = brew_prefix(environment)
    remove = [formula for formula in UNWANTED_FORMULAS if (prefix / 'Cellar' / formula).is_dir()]
    if remove:
        status = run(['brew', 'uninstall', '--ignore-dependencies', *remove], None)
    print('::endgroup::', flush=True)
    if status != 0:
        return status

    cas_path = Path(environment.get('HOME') or Path.home()) / CAS_RELATIVE
    cas_path.mkdir(parents=True, exist_ok=True)
    write_output(destination, {'xcodeCasPath': str(cas_path)})
    return 0


def build_argv(environ: dict[str, str]) -> list[str]:
    argv = ['--config', 'Debug', '--target', 'macos-arm64', '--codesign', '--analyze']
    if environ.get('RUNNER_DEBUG'):
        argv.append('--debug')
    return argv


def build(run: Runner = run_command, environ: dict[str, str] | None = None) -> int:
    """Refresh origin, then run the analysis build script with the analyzer argv."""

    environment = dict(os.environ if environ is None else environ)
    status = run(['git', 'fetch', 'origin', '--no-tags', '--no-recurse-submodules', '-q'], ROOT)
    if status != 0:
        return status
    return run([str(BUILD_SCRIPT), *build_argv(environment)], ROOT)


def flatten_plists(analytics: Path) -> list[Path]:
    """Move every analyzer `*.plist` beside `merged.sarif` as `*.sarif`."""

    source_root = analytics / 'StaticAnalyzer' / 'obs-studio'
    flattened: list[Path] = []
    for plist in sorted(source_root.glob('**/*.plist')):
        target = analytics / plist.name.replace('plist', 'sarif')
        print(f'{plist} -> {target}', flush=True)
        plist.replace(target)
        flattened.append(target)
    return flattened


def load_documents(analytics: Path) -> list[dict[str, Any]]:
    documents: list[dict[str, Any]] = []
    for path in sorted(analytics.glob('*.sarif')):
        if path.name == MERGED:
            continue
        documents.append(json.loads(path.read_text(encoding='utf-8')))
    return documents


def unique(values: list[Any]) -> list[Any]:
    """`jq unique`: sort by value, then drop the duplicates."""

    def canonical(value: Any) -> str:
        return json.dumps(value, sort_keys=True, ensure_ascii=False)

    ordered = sorted(values, key=canonical)
    result: list[Any] = []
    previous: str | None = None
    for value in ordered:
        key = canonical(value)
        if key != previous:
            result.append(value)
            previous = key
    return result


def runs(document: dict[str, Any]) -> list[dict[str, Any]]:
    value = document.get('runs')
    return value if isinstance(value, list) else []


def driver_of(document: dict[str, Any]) -> dict[str, Any] | None:
    for run in runs(document):
        driver = (run.get('tool') or {}).get('driver')
        if driver is not None:
            return driver
    return None


def first_driver(documents: list[dict[str, Any]]) -> dict[str, Any]:
    for document in documents:
        driver = driver_of(document)
        if driver is not None:
            return driver
    return {}


def collect(documents: list[dict[str, Any]], path: tuple[str, ...]) -> list[Any]:
    """Concatenate a nested array across every run of every document."""

    values: list[Any] = []
    for document in documents:
        for run in runs(document):
            node: Any = run
            for step in path:
                node = node.get(step) if isinstance(node, dict) else None
            if isinstance(node, list):
                values.extend(node)
    return values


def strip_zero_regions(results: list[Any]) -> list[Any]:
    """Drop zero `endLine`/`endColumn`, which LLVM writes for code-flow regions."""

    for result in results:
        if not isinstance(result, dict):
            continue
        for code_flow in result.get('codeFlows') or []:
            for thread_flow in code_flow.get('threadFlows') or []:
                for entry in thread_flow.get('locations') or []:
                    location = (entry.get('location') or {}).get('physicalLocation') or {}
                    region = location.get('region')
                    if not isinstance(region, dict):
                        continue
                    for key in ('endLine', 'endColumn'):
                        if region.get(key) == 0:
                            del region[key]
    return results


def merge(documents: list[dict[str, Any]]) -> dict[str, Any]:
    """The `jq -s` program the shell step ran, on parsed documents."""

    first = documents[0]
    driver = {key: value for key, value in first_driver(documents).items() if key != 'rules'}
    driver['rules'] = unique(collect(documents, ('tool', 'driver', 'rules')))
    merged: dict[str, Any] = {}
    if '$schema' in first:
        merged['$schema'] = first['$schema']
    merged['version'] = first.get('version')
    merged['runs'] = [
        {
            'tool': {'driver': driver},
            'artifacts': unique(collect(documents, ('artifacts',))),
            'results': strip_zero_regions(collect(documents, ('results',))),
        }
    ]
    return merged


def merge_sarif(analytics: Path = ANALYTICS) -> int:
    flatten_plists(analytics)
    documents = load_documents(analytics)
    if not documents:
        print(f'::error::no *.sarif files under {analytics}', flush=True)
        return 1
    merged = merge(documents)
    target = analytics / MERGED
    target.write_text(json.dumps(merged, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
    print(f'wrote {target} ({len(merged["runs"][0]["results"])} result(s))', flush=True)
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('setup', 'build', 'merge-sarif'))
    arguments = parser.parse_args(argv)
    if arguments.command == 'setup':
        return setup()
    if arguments.command == 'build':
        return build()
    return merge_sarif()


if __name__ == '__main__':
    sys.exit(main())
