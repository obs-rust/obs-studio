#!/bin/bash
# Usage: rust/tools/linux-validate/run.sh [--clean|--clean-all] [--wait]
#                                         [--repeat N] [--tests REGEX]
#   --clean        start this checkout's build and zccache volumes over
#   --clean-all    also drop the shared crate caches
#   --wait         if this checkout's Tier 2 is already running, wait for it
#                  (otherwise a second run refuses: exit 75)
#   --repeat N     after the normal ctest run, rerun the tests (or those
#                  matching --tests) up to N times in both builds, stopping
#                  at the first failure (ctest --repeat until-fail:N)
#   --tests REGEX  limit the repeat run to tests matching REGEX (ctest -R)
# e.g. run.sh --repeat 100 --tests 'test_threading|test_task'
#
# Runs validate.sh through ci/docker-task.sh, the shim into ci/docker_task.py
# and the stand-in for zackees/bosn#666: an ephemeral container named
# obs-rust-linux-validate-<checkout id>, one at a time per checkout, removed
# after the run; the checkout mounted read-only at /ro and built out of
# source; volumes the next run reattaches:
#   obs-rust-linux-validate-build-<id>  build trees: CMake, objects, cargo's
#                                       target dir (override: OBS_TIER2_VOLUME)
#   obs-rust-linux-validate-zccache-<id>
#                                       compiler cache (zccache)
#   obs-rust-linux-validate-cargo-registry, -cargo-git
#                                       shared crate downloads
# The image is tagged by content hash; only the newest three are kept. Each
# volume has a size budget (build 8G, zccache 3G, crates 2G each): one over it
# after a run is removed, so the next run starts it cold instead of growing.
# After every run, Corrosion's cargo target dirs keep only their newest
# artifacts (soldr cache prune-target --keep-latest); incremental compilation
# stays on (the prune keeps each crate's newest incremental data).
# <id> is a hash of the checkout path. Volumes of deleted checkouts:
#   ci/docker-task.sh gc --name obs-rust-linux-validate --image obs-rust-linux-validate [--apply]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
# In a git worktree, .git is a file whose (and whose submodules') git dirs
# resolve only on the host, so `git describe` fails inside the container.
# Resolve the OBS version here instead and hand it to validate.sh.
# The same goes for submodules: the container cannot initialize them from a
# worktree (obs-rust/obs-studio#32), so initialize any empty one here.
for sub in plugins/obs-browser plugins/obs-websocket; do
  if [ -z "$(ls -A "$repo/$sub" 2>/dev/null)" ]; then
    git -C "$repo" submodule update --init --depth 1 "$sub"
  fi
done
if [ -f "$repo/.git" ]; then
  version_override=$(git -C "$repo" describe --always --tags --match "[0-9]*.[0-9]*.[0-9]*")
fi
# The build volume is per checkout: a shared one let a run link another
# branch's stale objects or Rust staticlib (obs-rust/obs-studio#135).
# OBS_TIER2_VOLUME overrides its name.
task=(--stack-volume "build:/build${OBS_TIER2_VOLUME:+:$OBS_TIER2_VOLUME}")
# OBS_ZCCACHE_DIR: host directory for the zccache compiler cache (CI restores/saves it
# with actions/cache). Unset, each checkout has its own zccache volume: zccache
# keeps its daemon socket in the cache dir and allows one daemon per cache, so
# with a shared volume one checkout's compiles ran in another checkout's
# container, against that container's /ro (#2028).
if [ -n "${OBS_ZCCACHE_DIR:-}" ]; then
  mkdir -p "$OBS_ZCCACHE_DIR"
  task+=(--mount "$OBS_ZCCACHE_DIR:/zccache")
else
  task+=(--stack-volume zccache:/zccache)
fi
task+=(--env ZCCACHE_CACHE_DIR=/zccache)
# OBS_CTEST_JOBS: ctest parallelism (default 1). CI sets it: the 247 tests run
# twice (OFF, ON) and serially took ~11 min each of a ~25 min job.
if [ -n "${OBS_CTEST_JOBS:-}" ]; then
  task+=(--env "OBS_CTEST_JOBS=$OBS_CTEST_JOBS")
fi
# OBS_TIER2_MODES: "OFF", "ON" or "OFF ON" (default); CI runs one per job.
if [ -n "${OBS_TIER2_MODES:-}" ]; then
  task+=(--env "OBS_TIER2_MODES=$OBS_TIER2_MODES")
fi
# OBS_TIER2_EXPORTS_DIR: host directory that receives exports-<mode>.txt, the
# libobs.so dynamic symbols, for the CI job that compares OFF with ON.
if [ -n "${OBS_TIER2_EXPORTS_DIR:-}" ]; then
  mkdir -p "$OBS_TIER2_EXPORTS_DIR"
  task+=(--mount "$OBS_TIER2_EXPORTS_DIR:/exports")
fi
# OBS_TIER2_STAGE (all/build/test), OBS_CTEST_SHARD (<i>/<n>) and
# OBS_TIER2_PACK_DIR (a host dir for the build-tree tarballs) reproduce the
# CI build and shard jobs (#937); see validate.sh.
for var in OBS_TIER2_STAGE OBS_CTEST_SHARD OBS_TIER2_SHARDS; do
  if [ -n "${!var:-}" ]; then
    task+=(--env "$var=${!var}")
  fi
done
if [ -n "${OBS_TIER2_PACK_DIR:-}" ]; then
  mkdir -p "$OBS_TIER2_PACK_DIR"
  task+=(--mount "$OBS_TIER2_PACK_DIR:/pack" --env OBS_TIER2_PACK=/pack)
fi
# OBS_EXTRA_CFLAGS: extra C/C++ compiler flags for the CMake builds, e.g. to
# measure a stricter warning set (obs-rust/obs-studio#516).
if [ -n "${OBS_EXTRA_CFLAGS:-}" ]; then
  task+=(--env "OBS_EXTRA_CFLAGS=$OBS_EXTRA_CFLAGS")
fi
# OBS_EXTRA_CMAKE_ARGS: extra CMake configure arguments, space-separated, e.g.
# -DCMAKE_COMPILE_WARNING_AS_ERROR=OFF to collect every warning in one run.
if [ -n "${OBS_EXTRA_CMAKE_ARGS:-}" ]; then
  task+=(--env "OBS_EXTRA_CMAKE_ARGS=$OBS_EXTRA_CMAKE_ARGS")
fi
[ -z "${version_override:-}" ] || task+=(--env "OBS_VERSION_OVERRIDE=$version_override")

# run.sh's own flags; the rest go to validate.sh.
args=()
for arg in "$@"; do
  case $arg in
  --clean | --clean-all | --wait) task+=("$arg") ;;
  *) args+=("$arg") ;;
  esac
done

# Bake the toolchain pinned in rust-toolchain.toml into the image.
rust_toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo/rust-toolchain.toml")
exec bash "$repo/ci/docker-task.sh" run --name obs-rust-linux-validate --repo "$repo" \
  --image obs-rust-linux-validate --dockerfile "$here/Dockerfile" --context "$here" \
  --build-arg "RUST_TOOLCHAIN=$rust_toolchain" \
  --source ro --source-dest /ro \
  --mount "$here/validate.sh:/validate.sh:ro" \
  --machine-volume cargo-registry:/root/.cargo/registry \
  --machine-volume cargo-git:/root/.cargo/git \
  --budget build=8G --budget zccache=3G --budget cargo-registry=2G --budget cargo-git=2G \
  --prune-cargo /build/tree/cargo/build --prune-cargo /build/tree-debug/cargo/build \
  "${task[@]}" -- /validate.sh ${args[@]+"${args[@]}"}
