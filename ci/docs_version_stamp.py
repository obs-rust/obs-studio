#!/usr/bin/env -S uv run --script
"""Stamp the Sphinx docs version, release and copyright from libobs (#962).

Ports the shell step of `.github/actions/generate-docs/action.yaml` into Python
so the composite action makes one call instead of running a 30-line shell block
(GEN-005's shell budget). The stamping is unchanged:

- `version` and `release` in `docs/sphinx/conf.py` become the libobs API
  version from `libobs/obs-config.h` (`LIBOBS_API_{MAJOR,MINOR,PATCH}_VER`);
- `copyright` (the `2017-<year>, Lain Bailey` pair) takes the current year;
- with `--disable-link-extensions` the link suffix is blanked, so the artifact
  name says "No Extensions";
- the action's `setup` step outputs `artifactName` and the 9-character
  `commitHash`, written to `$GITHUB_OUTPUT` exactly as the shell did.

    uv run --no-project --script ci/docs_version_stamp.py --source-directory .

Standard library only: the docs job installs nothing before this step.
"""

from __future__ import annotations

import argparse
import os
from collections.abc import Mapping
from datetime import date
from pathlib import Path
import re

SEMVER = r'[0-9]+\.[0-9]+\.[0-9]+'
VERSION_DEFINE = re.compile(r'#define LIBOBS_API_(MAJOR|MINOR|PATCH)_VER *([0-9]+)')
VERSION_LINE = re.compile(rf"version = '({SEMVER})'")
RELEASE_LINE = re.compile(rf"release = '({SEMVER})'")
COPYRIGHT_LINE = re.compile(r"copyright = '(2017-[0-9]+, Lain Bailey)'")
LINK_SUFFIX = 'html_link_suffix = None'
DESCRIPTION = 'Stamp the Sphinx docs version, release and copyright from libobs.'
ARTIFACT_NAME = 'OBS Studio Docs'
ARTIFACT_NAME_NO_EXTENSIONS = 'OBS Studio Docs (No Extensions)'


def read_version(source_directory: Path) -> str:
    """The libobs API version from `libobs/obs-config.h`, e.g. `33.0.0`."""
    config = source_directory / 'libobs' / 'obs-config.h'
    try:
        text = config.read_text(encoding='utf-8')
    except OSError as error:
        raise SystemExit(f'{config}: {error.strerror}') from error
    found = {name: value for name, value in VERSION_DEFINE.findall(text)}
    missing = sorted({'MAJOR', 'MINOR', 'PATCH'} - found.keys())
    if missing:
        raise SystemExit(
            f'{config}: no LIBOBS_API_{", LIBOBS_API_".join(missing)}_VER definition'
        )
    return f'{found["MAJOR"]}.{found["MINOR"]}.{found["PATCH"]}'


def stamp_conf(conf: Path, version: str, *, disable_link_extensions: bool) -> None:
    """Rewrite the version, release, copyright and (optionally) link suffix."""
    text = conf.read_text(encoding='utf-8')
    text = VERSION_LINE.sub(lambda _: f"version = '{version}'", text)
    text = RELEASE_LINE.sub(lambda _: f"release = '{version}'", text)
    text = COPYRIGHT_LINE.sub(
        lambda _: f"copyright = '2017-{date.today().year}, Lain Bailey'", text
    )
    if disable_link_extensions:
        text = text.replace(LINK_SUFFIX, "html_link_suffix = ''")
    conf.write_text(text, encoding='utf-8')


def stamp(
    source_directory: Path,
    *,
    disable_link_extensions: bool,
    env: Mapping[str, str] | None = None,
) -> dict[str, str]:
    """Stamp the checkout and return the action's step outputs."""
    environment = os.environ if env is None else env
    version = read_version(source_directory)
    stamp_conf(
        source_directory / 'docs' / 'sphinx' / 'conf.py',
        version,
        disable_link_extensions=disable_link_extensions,
    )
    return {
        'artifactName': (
            ARTIFACT_NAME_NO_EXTENSIONS if disable_link_extensions else ARTIFACT_NAME
        ),
        'commitHash': environment.get('GITHUB_SHA', '')[:9],
    }


def write_outputs(outputs: Mapping[str, str], path: str | None) -> None:
    """Append `name=value` lines to the step's `$GITHUB_OUTPUT` file."""
    if not path:
        return
    with open(path, 'a', encoding='utf-8') as handle:
        for name, value in outputs.items():
            handle.write(f'{name}={value}\n')


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=DESCRIPTION)
    parser.add_argument(
        '--source-directory',
        default='.',
        help='repository checkout holding libobs/obs-config.h and docs/sphinx',
    )
    parser.add_argument(
        '--disable-link-extensions',
        default='false',
        help="'true' blanks the Sphinx link suffix (the action's input)",
    )
    arguments = parser.parse_args(argv)
    outputs = stamp(
        Path(arguments.source_directory),
        disable_link_extensions=arguments.disable_link_extensions == 'true',
    )
    write_outputs(outputs, os.environ.get('GITHUB_OUTPUT'))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
