#!/usr/bin/env bash
# Configure and build libobs with ENABLE_CORE_MODULES=OFF (obs-rust/obs-studio#2280).
# Builds each backend (C and the default HYBRID, with the C reference) in
# its own build tree under BUILD_ROOT, then checks the generated core module
# list is empty: obs_core_modules_count 0 and a NULL-only array.
#
#   ci/core_modules_off.sh BUILD_ROOT [extra cmake args...]
#
# Extra args go to every configure (e.g. -DRust_TOOLCHAIN=..., -DCMocka_DIR=...).
# Env: BACKENDS (default "C HYBRID"), JOBS (ninja -j, default: ninja's own).
set -euo pipefail
[ $# -ge 1 ] || { echo "usage: core_modules_off.sh BUILD_ROOT [extra cmake args...]" >&2; exit 2; }
root=$1
shift
src=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root"

for backend in ${BACKENDS:-C HYBRID}; do
  build="$root/$backend"
  targets=(libobs)
  case $backend in
    C) ref=OFF ;;
    HYBRID) ref=ON targets+=(libobs-c-reference) ;;
    *) echo "unknown backend $backend" >&2; exit 2 ;;
  esac
  echo "== $backend: configure"
  cmake -S "$src" -B "$build" -G Ninja \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DOBS_LIBOBS_BACKEND="$backend" -DOBS_BUILD_C_REFERENCE="$ref" \
    -DENABLE_CORE_MODULES=OFF -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_UNIT_TESTS=OFF \
    "$@"
  echo "== $backend: build ${targets[*]}"
  ninja ${JOBS:+-j "$JOBS"} -C "$build" "${targets[@]}"

  list="$build/libobs/obs-core-modules.c"
  if ! grep -q '^const unsigned int obs_core_modules_count = 0;$' "$list" \
    || ! grep -Pzq 'obs_core_modules\[\] = \{\n\tNULL\n\};' "$list"; then
    echo "$backend: $list is not an empty, NULL-terminated list:" >&2
    cat "$list" >&2
    exit 1
  fi
  echo "== $backend: ok"
done
