#!/bin/bash
# Usage: rust/tools/linux-validate/run.sh [--repeat N] [--tests REGEX]
#   --repeat N     after the normal ctest run, rerun the tests (or those
#                  matching --tests) up to N times in both builds, stopping
#                  at the first failure (ctest --repeat until-fail:N)
#   --tests REGEX  limit the repeat run to tests matching REGEX (ctest -R)
# e.g. run.sh --repeat 100 --tests 'test_threading|test_task'
#
# Each checkout builds in its own Docker volume,
# obs-rust-linux-validate-build-<hash of the checkout path> (override with
# OBS_TIER2_VOLUME); the ccache volume obs-rust-linux-validate-ccache is
# shared. List or remove old ones with:
#   docker volume ls --filter name=obs-rust-linux-validate-
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
repo=$(cd "$here/../../.." && pwd)
# In a git worktree, .git is a file whose (and whose submodules') git dirs
# resolve only on the host, so `git describe` fails inside the container.
# Resolve the OBS version here instead and hand it to validate.sh.
env=()
# The same goes for submodules: the container cannot initialize them from a
# worktree (obs-rust/obs-studio#32), so initialize any empty one here.
for sub in plugins/obs-browser plugins/obs-websocket; do
  if [ -z "$(ls -A "$repo/$sub" 2>/dev/null)" ]; then
    git -C "$repo" submodule update --init --depth 1 "$sub"
  fi
done
if [ -f "$repo/.git" ]; then
  env=(-e "OBS_VERSION_OVERRIDE=$(git -C "$repo" describe --always --tags)")
fi
# The build volume is per checkout: a shared one let a run link another
# branch's stale objects or Rust staticlib (obs-rust/obs-studio#135).
# OBS_TIER2_VOLUME overrides the name.
if command -v sha256sum >/dev/null 2>&1; then
  checkout_id=$(printf '%s' "$repo" | sha256sum | cut -c1-12)
else
  checkout_id=$(printf '%s' "$repo" | shasum -a 256 | cut -c1-12)
fi
build_volume="${OBS_TIER2_VOLUME:-obs-rust-linux-validate-build-$checkout_id}"
# OBS_CCACHE_DIR: host directory for the compiler cache (CI restores/saves it
# with actions/cache). Unset, every checkout shares one ccache volume, so a
# new checkout does not start cold.
if [ -n "${OBS_CCACHE_DIR:-}" ]; then
  mkdir -p "$OBS_CCACHE_DIR"
  env+=(-v "$OBS_CCACHE_DIR:/ccache" -e CCACHE_DIR=/ccache)
else
  env+=(-v obs-rust-linux-validate-ccache:/ccache -e CCACHE_DIR=/ccache)
fi
# Bake the toolchain pinned in rust-toolchain.toml into the image.
rust_toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$repo/rust-toolchain.toml")
# CI builds the image itself with a layer cache and sets OBS_TIER2_IMAGE_PREBUILT.
if [ -z "${OBS_TIER2_IMAGE_PREBUILT:-}" ]; then
  docker build -q --build-arg "RUST_TOOLCHAIN=$rust_toolchain" -t obs-rust-linux-validate "$here" >/dev/null
fi
# bash 3.2 (macOS /bin/bash) errors on "${env[@]}" when the array is empty
# and set -u is on. Ubuntu bash does not. A normal checkout leaves it empty.
run=(docker run --rm -v "$repo:/ro:ro")
if [ "${#env[@]}" -gt 0 ]; then
  run+=("${env[@]}")
fi
run+=(
  -v "$here/validate.sh:/validate.sh:ro"
  -v "$build_volume:/build"
  obs-rust-linux-validate /validate.sh "$@"
)
"${run[@]}"
