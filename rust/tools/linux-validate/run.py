#!/usr/bin/env -S uv run --script
"""Run the Tier 2 Linux harness (validate.sh) in a container (obs-rust/obs-studio#135).

    uv run --no-project rust/tools/linux-validate/run.py [--clean|--clean-all] [--wait]
        [--repeat N] [--tests REGEX]
      --clean        start this checkout's build and zccache volumes over
      --clean-all    also drop the shared crate caches
      --wait         if this checkout's Tier 2 is already running, wait for it
                     (otherwise a second run refuses: exit 75)
      --repeat N     after the normal ctest run, rerun the tests (or those
                     matching --tests) up to N times in both builds, stopping
                     at the first failure (ctest --repeat until-fail:N)
      --tests REGEX  limit the repeat run to tests matching REGEX (ctest -R)
    e.g. run.py --repeat 100 --tests 'test_threading|test_task'

Runs validate.sh through `uv run --script ci/docker_task.py`, the stand-in for
zackees/bosn#666: an ephemeral container named
obs-rust-linux-validate-<checkout id>, one at a time per checkout, removed
after the run; the checkout mounted read-only at /ro and built out of
source; volumes the next run reattaches:
  obs-rust-linux-validate-build-<id>  build trees: CMake, objects, cargo's
                                      target dir (override: OBS_TIER2_VOLUME)
  obs-rust-linux-validate-zccache-<id>
                                      compiler cache (zccache)
  obs-rust-linux-validate-cargo-registry, -cargo-git
                                      shared crate downloads
The image is tagged by content hash; only the newest three are kept. Each
volume has a size budget (build 8G, zccache 3G, crates 2G each): one over it
after a run is removed, so the next run starts it cold instead of growing.
After every run, Corrosion's cargo target dirs keep only their newest
artifacts (soldr cache prune-target --keep-latest); incremental compilation
stays on (the prune keeps each crate's newest incremental data).
<id> is a hash of the checkout path. Volumes of deleted checkouts:
  uv run --script ci/docker_task.py gc --name obs-rust-linux-validate --image obs-rust-linux-validate [--apply]

Ported off the shell script in #962; the local gate's tier2-linux lane runs
this module through the interpreter it already has.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path

NAME = "obs-rust-linux-validate"
HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent.parent
# run.sh's own flags; the rest go to validate.sh.
RUN_FLAGS = ("--clean", "--clean-all", "--wait")
# Submodules the container cannot initialize from a worktree
# (obs-rust/obs-studio#32), so they are initialized here.
SUBMODULES = ("plugins/obs-browser", "plugins/obs-websocket")


def is_empty(path: Path) -> bool:
    """`[ -z "$(ls -A <path>)" ]`: a missing or unreadable directory counts as empty."""
    try:
        return not any(path.iterdir())
    except OSError:
        return True


def describe_version(repo: Path) -> str:
    """`git describe` here, not in the container: in a git worktree .git is a
    file whose (and whose submodules') git dirs resolve only on the host, so
    `git describe` fails inside the container."""
    result = subprocess.run(
        ["git", "-C", str(repo), "describe", "--always", "--tags", "--match", "[0-9]*.[0-9]*.[0-9]*"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        sys.exit(result.returncode)
    return result.stdout.strip()


def pinned_toolchain(repo: Path) -> str:
    """The channel rust-toolchain.toml pins, for the image to bake in."""
    text = (repo / "rust-toolchain.toml").read_text(encoding="utf-8")
    match = re.search(r'^channel *= *"(.*)"', text, re.MULTILINE)
    return match.group(1) if match else ""


def passthrough(env: Mapping[str, str], names: tuple[str, ...]) -> list[str]:
    """`--env NAME=value` for each knob the caller set; an unset one is omitted."""
    return [item for name in names if env.get(name) for item in ("--env", f"{name}={env[name]}")]


def host_directory(env: Mapping[str, str], name: str, target: str) -> list[str]:
    """A host directory CI mounts in, created first so Docker does not make it root-owned."""
    directory = env[name]
    Path(directory).mkdir(parents=True, exist_ok=True)
    return ["--mount", f"{directory}:{target}"]


def task_arguments(env: Mapping[str, str], version_override: str) -> list[str]:
    """The container's volumes, mounts and environment, in run.sh's order.

    The build volume is per checkout: a shared one let a run link another
    branch's stale objects or Rust staticlib (obs-rust/obs-studio#135).
    OBS_ZCCACHE_DIR: host directory for the zccache compiler cache (CI
    restores/saves it with actions/cache). Unset, each checkout has its own
    zccache volume: zccache keeps its daemon socket in the cache dir and
    allows one daemon per cache, so with a shared volume one checkout's
    compiles ran in another checkout's container, against that container's
    /ro (#2028).
    """
    override = f":{env['OBS_TIER2_VOLUME']}" if env.get("OBS_TIER2_VOLUME") else ""
    arguments = ["--stack-volume", f"build:/build{override}"]
    if env.get("OBS_ZCCACHE_DIR"):
        arguments += host_directory(env, "OBS_ZCCACHE_DIR", "/zccache")
    else:
        arguments += ["--stack-volume", "zccache:/zccache"]
    arguments += ["--env", "ZCCACHE_CACHE_DIR=/zccache"]
    arguments += passthrough(env, ("OBS_CTEST_JOBS", "OBS_TIER2_MODES"))
    if env.get("OBS_TIER2_EXPORTS_DIR"):
        # Receives exports-<mode>.txt, the libobs.so dynamic symbols, for the
        # CI job that compares OFF with ON.
        arguments += host_directory(env, "OBS_TIER2_EXPORTS_DIR", "/exports")
    arguments += passthrough(env, ("OBS_TIER2_STAGE", "OBS_CTEST_SHARD", "OBS_TIER2_SHARDS"))
    # OBS_TIER2_SERIAL_CPUS: run the RUN_SERIAL part beside the suite on its own
    # CPUs (#2773). Host-only knob; CI never sets it.
    arguments += passthrough(env, ("OBS_TIER2_SERIAL_CPUS",))
    if env.get("OBS_TIER2_PACK_DIR"):
        # A host dir for the build-tree tarballs (#937); see validate.sh.
        arguments += host_directory(env, "OBS_TIER2_PACK_DIR", "/pack")
        arguments += ["--env", "OBS_TIER2_PACK=/pack"]
    arguments += passthrough(env, ("OBS_EXTRA_CFLAGS", "OBS_EXTRA_CMAKE_ARGS"))
    if version_override:
        arguments += ["--env", f"OBS_VERSION_OVERRIDE={version_override}"]
    return arguments


def docker_task_command(argv: list[str], env: Mapping[str, str]) -> list[str]:
    """The `uv run --script ci/docker_task.py run … -- /validate.sh <args>` line."""
    for submodule in SUBMODULES:
        if is_empty(REPO / submodule):
            subprocess.run(
                ["git", "-C", str(REPO), "submodule", "update", "--init", "--depth", "1", submodule],
                check=True,
            )
    version_override = describe_version(REPO) if (REPO / ".git").is_file() else ""
    task = task_arguments(env, version_override)
    validate_args = []
    for argument in argv:
        (task if argument in RUN_FLAGS else validate_args).append(argument)
    return [
        "uv",
        "run",
        "--script",
        str(REPO / "ci" / "docker_task.py"),
        "run",
        "--name",
        NAME,
        "--repo",
        str(REPO),
        "--image",
        NAME,
        "--dockerfile",
        str(HERE / "Dockerfile"),
        "--context",
        str(HERE),
        "--build-arg",
        f"RUST_TOOLCHAIN={pinned_toolchain(REPO)}",
        "--source",
        "ro",
        "--source-dest",
        "/ro",
        "--mount",
        f"{HERE}/validate.sh:/validate.sh:ro",
        "--machine-volume",
        "cargo-registry:/root/.cargo/registry",
        "--machine-volume",
        "cargo-git:/root/.cargo/git",
        "--budget",
        "build=8G",
        "--budget",
        "zccache=3G",
        "--budget",
        "cargo-registry=2G",
        "--budget",
        "cargo-git=2G",
        "--prune-cargo",
        "/build/tree/cargo/build",
        "--prune-cargo",
        "/build/tree-debug/cargo/build",
        *task,
        "--",
        "/validate.sh",
        *validate_args,
    ]


def main(argv: list[str]) -> int:
    try:
        os.execvp("uv", docker_task_command(argv, os.environ))
    except OSError as error:
        print(f"run.py: cannot run uv: {error}", file=sys.stderr)
        return 127
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
