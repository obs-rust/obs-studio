#!/usr/bin/env -S uv run --script
"""Run a command inside the glibc 2.28 build image, as a workflow step would.

    uv run --script build-aux/appimage/manylinux/run.py <command...>

The image is built from `Dockerfile` beside this file and tagged
`obs-appimage-228` by the workflow. The workspace and the runner's temp
directory are mounted at their own paths, so `GITHUB_ENV`, `GITHUB_OUTPUT` and
the ccache directory work unchanged; the step's environment is forwarded, and
the cargo registry is the runner's (restored and saved by actions/cache). The
configured zccache directory comes along, being inside the mounted workspace.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

# The step environment a build inside the image may need. Everything else stays
# on the host, so a runner-only setting cannot change what the image sees.
_FORWARDED = re.compile(
    r"^(GITHUB_[A-Z_]*|RUNNER_[A-Z_]*|CCACHE_[A-Z_]*|ZCCACHE_CACHE_DIR|RUST_LIBOBS|"
    r"QSV|OBS_RUST_VERSION|PUBLISH_CONTINUOUS|CI)$"
)
DEFAULT_IMAGE = "obs-appimage-228"


def docker_argv(argv: list[str], environ: dict[str, str]) -> list[str]:
    """The `docker run` invocation that carries this step into the image."""
    workspace = environ.get("GITHUB_WORKSPACE") or os.getcwd()
    temp = environ.get("RUNNER_TEMP") or "/tmp"
    registry = Path(environ.get("HOME") or os.path.expanduser("~")) / ".cargo" / "registry"
    registry.mkdir(parents=True, exist_ok=True)
    args = ["docker", "run", "--rm",
            "-v", f"{workspace}:{workspace}",
            "-v", f"{temp}:{temp}",
            "-v", f"{registry}:/usr/local/cargo/registry",
            "-w", workspace]
    for name in sorted(name for name in environ if _FORWARDED.match(name)):
        args += ["-e", name]
    args.append(environ.get("OBS_APPIMAGE_IMAGE") or DEFAULT_IMAGE)
    return args + argv


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__, file=sys.stderr)
        return 2
    args = docker_argv(argv[1:], dict(os.environ))
    try:
        os.execvp(args[0], args)
    except OSError as error:
        print(f"manylinux run: {error}", file=sys.stderr)
        return 1
    return 0  # Unreachable: exec replaces this process.


if __name__ == "__main__":
    sys.exit(main(sys.argv))
