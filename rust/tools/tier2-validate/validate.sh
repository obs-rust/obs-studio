#!/bin/bash
# Tier 2 validation (docs/rust-port/testing-policy.md) on macOS, run directly
# on the host (no Docker). Builds libobs and the cmocka tests with
# ENABLE_RUST_LIBOBS=OFF and =ON, runs the unchanged C tests in both, and
# fails unless both builds export exactly the same dynamic symbols from
# libobs. Mirrors rust/tools/linux-validate/validate.sh.
#
# Prerequisites: cmake, Xcode, and `brew install cmocka`. The macos-ci preset
# downloads the pre-built obs-deps during configure.
#
# Usage: validate.sh [--repeat N] [--tests REGEX]
#   --repeat N     after the normal ctest run, rerun the tests (or those
#                  matching --tests) up to N times in both builds, stopping
#                  at the first failure (ctest --repeat until-fail:N)
#   --tests REGEX  limit the repeat run to tests matching REGEX (ctest -R)
set -euo pipefail

repeat=0
tests=
while [ $# -gt 0 ]; do
  case "$1" in
  --repeat)
    repeat="${2:-}"
    shift 2 || { echo "--repeat needs a count" >&2; exit 2; }
    ;;
  --tests)
    tests="${2:-}"
    shift 2 || { echo "--tests needs a regex" >&2; exit 2; }
    ;;
  *)
    echo "usage: validate.sh [--repeat N] [--tests REGEX]" >&2
    exit 2
    ;;
  esac
done
if ! [[ "$repeat" =~ ^[0-9]+$ ]]; then
  echo "--repeat must be a non-negative integer, got '$repeat'" >&2
  exit 2
fi
if [ -n "$tests" ] && [ "$repeat" -eq 0 ]; then
  echo "--tests only applies with --repeat" >&2
  exit 2
fi

cd "$(dirname "$0")/../../.."

# The macos-ci preset reads this env var for the Xcode compilation cache.
export XCODE_CAS_PATH="${XCODE_CAS_PATH:-$PWD/build_rust_xcode_cas}"
mkdir -p "$XCODE_CAS_PATH"

cmocka_prefix="$(brew --prefix cmocka)"

for mode in OFF ON; do
  build="build_rust_$mode"
  echo "== [$mode] configure + build"
  cmake --preset macos-ci -B "$build" \
    -DENABLE_UNIT_TESTS=ON \
    -DENABLE_RUST_LIBOBS="$mode" \
    -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
    -DENABLE_WEBSOCKET=OFF \
    -DCMAKE_PREFIX_PATH="$cmocka_prefix" \
    >"$build.configure.log" 2>&1 || { tail -40 "$build.configure.log"; exit 1; }
  cmake --build "$build" --config RelWithDebInfo \
    --target libobs cmocka-tests \
    >"$build.build.log" 2>&1 || { grep -E ": (fatal )?error|BUILD FAILED" "$build.build.log" | sort -u | head -40; exit 1; }

  lib=$(find "$build" -path '*libobs.framework*' -name libobs -type f | head -1)
  if [ -z "$lib" ]; then
    echo "FAIL: libobs binary not found inside libobs.framework in $build"
    exit 1
  fi

  # The test executables load @rpath/libobs.framework, and their build rpath
  # (@executable_path/../Frameworks) only holds it in an app bundle, so point
  # dyld at the framework's build directory (absolute: ctest runs each test
  # from its own directory).
  framework_dir="$PWD/${lib%%/libobs.framework/*}"
  # libobs in turn loads @rpath/libavcodec.dylib etc. from the pre-built
  # obs-deps that the preset unpacked under .deps.
  avcodec=$(find "$PWD/.deps" -path '*/lib/libavcodec*.dylib' | head -1)
  if [ -z "$avcodec" ]; then
    echo "FAIL: obs-deps libavcodec not found under .deps"
    exit 1
  fi

  echo "== [$mode] ctest"
  DYLD_FRAMEWORK_PATH="$framework_dir" DYLD_LIBRARY_PATH="$(dirname "$avcodec")" \
    ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure

  if [ "$repeat" -gt 0 ]; then
    echo "== [$mode] ctest --repeat until-fail:$repeat${tests:+ -R $tests}"
    DYLD_FRAMEWORK_PATH="$framework_dir" DYLD_LIBRARY_PATH="$(dirname "$avcodec")" \
      ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure \
      --repeat "until-fail:$repeat" ${tests:+-R "$tests"}
  fi

  nm -gU "$lib" | awk '{print $3}' | sort >"exports-$mode.txt"
done

echo "== libobs exported symbols, OFF vs ON"
diff exports-OFF.txt exports-ON.txt
echo "IDENTICAL ($(wc -l <exports-ON.txt | tr -d ' ') symbols)"
