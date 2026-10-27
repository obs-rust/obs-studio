#!/usr/bin/env -S uv run --script
"""Cross-compile check (cmake/cross/) in a container, so the host needs only
Docker: no soldr, LLVM or SDK. Runs through `uv run --script ci/docker_task.py`
(the stand-in for zackees/bosn#666): an ephemeral container per checkout, one at
a time.

    uv run --script cmake/cross/docker.py [--clean|--clean-all] [--wait] <alias>...
    e.g. cmake/cross/docker.py mac-arm64 win-x64
    OBS_CROSS_CMAKE_ARGS="-DOBS_UNITY_BUILD_BATCH_SIZE=0" \\
        uv run --script cmake/cross/docker.py win-x64
      (extra CMake arguments; batch size 0 puts each target in one unity batch,
      so every pair of its sources meets)

Code and caches:
  /src           the checkout, copy-on-write overlay: read-only underneath,
                 writes vanish after the run
  /build         per checkout (obs-cross-build-<id>): one CMake tree per alias,
                 and the soldr CMake downloads
  /root/.soldr   shared (obs-cross-soldr): soldr's LLVM, Apple SDK, xwin,
                 Rust toolchain and compiler cache
  /src/.deps     shared (obs-cross-deps): OBS's pinned prebuilt
                 Windows/macOS dependencies
The first run downloads soldr, LLVM, the SDK and the deps; later runs reattach
the caches and only recompile what changed. Budgets cap each volume (build 6G,
soldr 12G, deps 2G): one over it after a run is removed and rebuilt cold next
time, so nothing grows without bound.
"""

from __future__ import annotations

import os
import re
import sys
from pathlib import Path

TASK_FLAGS = ("--clean", "--clean-all", "--wait")
USAGE = "usage: {name} [--clean|--clean-all] [--wait] <alias>... (win-x64, mac-arm64, mac-x64)"
ALIAS_USAGE = "usage: {name} [--clean|--clean-all] [--wait] <alias>..."
# One container builds every requested alias in turn.
SCRIPT = """set -e; cd /src
for alias in "$@"; do
  echo "== cross $alias"
  # soldr itself is downloaded once into the shared soldr volume, not per alias.
  OBS_CROSS_TARGET="$alias" cmake -S cmake/cross -B "/build/$alias" -G Ninja -DOBS_CROSS_TARGET="$alias" ${OBS_CROSS_CMAKE_ARGS:-} \\
    -DOBS_SOLDR_DOWNLOAD_DIR="/root/.soldr/obs-bootstrap/$SOLDR_VERSION"
  OBS_CROSS_TARGET="$alias" cmake --build "/build/$alias" -- -k 0
done"""


def pinned(path: Path, pattern: str) -> str:
    """The first capture of a pinned version in a CMake or toolchain file."""
    match = re.search(pattern, path.read_text(encoding="utf-8"), re.MULTILINE)
    return match.group(1) if match else ""


def parse(argv: list[str], name: str) -> tuple[list[str], list[str]] | None:
    """The task flags and the aliases, or None when the arguments are wrong."""
    task: list[str] = []
    aliases: list[str] = []
    for arg in argv:
        if arg in TASK_FLAGS:
            task.append(arg)
        elif arg.startswith("-"):
            print(USAGE.format(name=name), file=sys.stderr)
            return None
        else:
            aliases.append(arg)
    if not aliases:
        print(ALIAS_USAGE.format(name=name), file=sys.stderr)
        return None
    return task, aliases


def task_argv(repo: Path, task: list[str], aliases: list[str], extra: str,
              toolchain: str, soldr: str) -> list[str]:
    """The task runner's arguments (uv run --script ci/docker_task.py):
    the volumes, budgets and container command."""
    return [
        "uv", "run", "--script", str(repo / "ci" / "docker_task.py"),
        "run", "--name", "obs-cross", "--repo", str(repo),
        "--image", "obs-rust-linux-validate",
        "--dockerfile", str(repo / "rust" / "tools" / "linux-validate" / "Dockerfile"),
        "--context", str(repo / "rust" / "tools" / "linux-validate"),
        "--build-arg", f"RUST_TOOLCHAIN={toolchain}",
        "--source", "overlay", "--source-dest", "/src",
        "--stack-volume", "build:/build",
        "--machine-volume", "soldr:/root/.soldr",
        "--machine-volume", "deps:/src/.deps",
        "--budget", "build=6G", "--budget", "soldr=12G", "--budget", "deps=2G",
        "--env", f"SOLDR_VERSION={soldr}",
        "--env", f"OBS_CROSS_CMAKE_ARGS={extra}",
        *task, "--", "bash", "-c", SCRIPT, "cross", *aliases,
    ]


def main(argv: list[str]) -> int:
    repo = Path(__file__).resolve().parent.parent.parent
    parsed = parse(argv[1:], os.path.basename(argv[0]))
    if parsed is None:
        return 2
    task, aliases = parsed
    # The image only needs cmake, ninja, git and curl: the Tier 2 one has them.
    toolchain = pinned(repo / "rust-toolchain.toml", r'^channel *= *"(.*)"')
    soldr = pinned(repo / "cmake" / "common" / "soldr.cmake", r"^set\(OBS_SOLDR_VERSION ([0-9.]+)\)$")
    args = task_argv(repo, task, aliases, os.environ.get("OBS_CROSS_CMAKE_ARGS", ""), toolchain, soldr)
    os.execvp("uv", args)
    return 0  # Unreachable: exec replaces this process.


if __name__ == "__main__":
    sys.exit(main(sys.argv))
