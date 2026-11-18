#!/bin/bash
# Tier 2 validation (docs/rust-port/testing-policy.md) on macOS, run directly
# on the host (no Docker). One build makes the cmocka tests and both libobs
# frameworks, libobs (C) and libobs-c-reference (#1065); it runs the unchanged
# C tests against each and fails unless both export exactly the same symbols.
# Mirrors rust/tools/linux-validate/validate.sh.
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

# ctest reports a crashed test only as "Segmentation fault". Run each failed
# test again under lldb (repeatedly, since crashes can be intermittent) and
# print the faulting stack of every thread, so a macOS-only crash can be
# diagnosed from the log.
show_crash_stacks() {
  local build="$1" failed line name exe attempt out
  failed="$build/Testing/Temporary/LastTestsFailed.log"
  [ -f "$failed" ] || return 0
  command -v lldb >/dev/null || { echo "lldb not found; no crash stacks"; return 0; }
  while IFS= read -r line; do
    name="${line#*:}"
    [ -n "$name" ] || continue
    exe=$(find "$build" -type f -name "$name" -perm -u+x | head -1)
    [ -n "$exe" ] || continue
    echo "== crash stack: $name"
    for attempt in $(seq 1 30); do
      out=$(cd "$(dirname "$exe")" && lldb --batch -o run -k "bt all" -k quit -- "$exe" 2>&1) || true
      if printf '%s' "$out" | grep -q "stop reason = "; then
        printf '%s\n' "$out" | tail -150
        break
      fi
      [ "$attempt" -lt 30 ] || echo "no crash in 30 lldb runs of $name"
    done
  done <"$failed"
}

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

# OBS_CTEST_JOBS: ctest parallelism for each suite (default 1); CI sets it
# (#2138). Tests that must not overlap carry RUN_SERIAL or RESOURCE_LOCK.
ctest_jobs=(-j "${OBS_CTEST_JOBS:-1}")

build="build_rust"
echo "== configure (libobs and libobs-c-reference)"
cmake --preset macos-ci -B "$build" \
  -DENABLE_UNIT_TESTS=ON -DOBS_BUILD_C_REFERENCE=ON -DOBS_LIBOBS_BACKEND=HYBRID \
  -DOBS_BUILD_NATIVE_TIER2=ON \
  -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
  -DENABLE_WEBSOCKET=OFF \
  -DCMAKE_PREFIX_PATH="$cmocka_prefix" \
  >"$build.configure.log" 2>&1 || { tail -40 "$build.configure.log"; exit 1; }
echo "== build (complete native Tier 2 graph)"
cmake --build "$build" --config RelWithDebInfo --target tier2-native \
  >"$build.build.log" 2>&1 || { grep -E ": (fatal )?error|BUILD FAILED" "$build.build.log" | sort -u | head -40; exit 1; }

# libobs.framework twice: libobs (C, what the tests link) and its Rust twin
# in $build/libobs-c-reference (#1065), same install name.
c_lib=$(find "$build/libobs-c-reference" -path '*libobs.framework*' -name libobs -type f | head -1)
rust_lib=$(find "$build" -path "$build/libobs-c-reference" -prune -o -path '*libobs.framework*' -name libobs -type f -print | head -1)
if [ -z "$c_lib" ] || [ -z "$rust_lib" ]; then
  echo "FAIL: libobs.framework not found (C: '${c_lib}', with Rust: '${rust_lib}') in $build"
  exit 1
fi
nm -gU "$c_lib" | awk '{print $3}' | sort >exports-OFF.txt
nm -gU "$rust_lib" | awk '{print $3}' | sort >exports-ON.txt
echo "== libobs exported symbols, C vs Rust"
diff exports-OFF.txt exports-ON.txt
echo "IDENTICAL ($(wc -l <exports-ON.txt | tr -d ' ') symbols)"

# libobs in turn loads @rpath/libavcodec.dylib etc. from the pre-built
# obs-deps that the preset unpacked under .deps.
avcodec=$(find "$PWD/.deps" -path '*/lib/libavcodec*.dylib' | head -1)
if [ -z "$avcodec" ]; then
  echo "FAIL: obs-deps libavcodec not found under .deps"
  exit 1
fi

for mode in OFF ON; do
  if [ "$mode" = ON ]; then lib=$rust_lib; else lib=$c_lib; fi
  # The test executables load @rpath/libobs.framework, and their build rpath
  # (@executable_path/../Frameworks) only holds it in an app bundle, so point
  # dyld at the framework's directory (absolute: ctest runs each test from
  # its own directory). That also picks which libobs the tests run against.
  framework_dir="$PWD/${lib%%/libobs.framework/*}"
  dyld=(env DYLD_FRAMEWORK_PATH="$framework_dir" DYLD_LIBRARY_PATH="$(dirname "$avcodec")")

  # Check the loader really took this libobs on one test binary.
  probe=$(find "$build/test/cmocka" -name test_bitstream -type f -perm -u+x | head -1)
  if ! "${dyld[@]}" DYLD_PRINT_LIBRARIES=1 "$probe" 2>&1 | grep -qF "$framework_dir/libobs.framework/"; then
    echo "FAIL: [$mode] tests did not load libobs from $framework_dir"
    exit 1
  fi
  echo "== [$mode] tests load $framework_dir/libobs.framework"

  echo "== [$mode] ctest"
  if ! "${dyld[@]}" ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure "${ctest_jobs[@]}"; then
    DYLD_FRAMEWORK_PATH="$framework_dir" DYLD_LIBRARY_PATH="$(dirname "$avcodec")" \
      show_crash_stacks "$build"
    exit 1
  fi

  if [ "$repeat" -gt 0 ]; then
    echo "== [$mode] ctest --repeat until-fail:$repeat${tests:+ -R $tests}"
    "${dyld[@]}" ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure \
      --repeat "until-fail:$repeat" ${tests:+-R "$tests"}
  fi
done

# Exercise the Cargo native graph on the same macOS machine and dependency
# bundle as the reference. Build the Rust archive outside build.rs, into an
# explicit output directory, so no source-tree or stale archive is selected.
case "$(uname -m)" in
  arm64) native_target=aarch64-apple-darwin ;;
  x86_64) native_target=x86_64-apple-darwin ;;
  *) echo "unsupported macOS native architecture" >&2; exit 1 ;;
esac
native_output="$PWD/$build/cargo-native-core"
native_rust="$PWD/$build/cargo-native-rust"
native_tests="$PWD/$build/cargo-native-tests"
reference_opengl=$(find "$build/libobs-opengl" -name 'libobs-opengl*.dylib' -type f | head -1)
reference_metal=$(find "$build/libobs-metal" -name 'libobs-metal*.dylib' -type f | head -1)
if [ ! -f "$reference_opengl" ] || [ ! -f "$reference_metal" ]; then
  echo 'FAIL: original CMake graphics modules not found' >&2
  exit 1
fi
reference_opengl="$PWD/$reference_opengl"
reference_metal="$PWD/$reference_metal"
capture_reference() {
  local name="$1" directory="$2" binary
  binary=$(find "$build/plugins/$directory" -path "*/Contents/MacOS/$name" -type f | head -1)
  if [ -z "$binary" ] || [ ! -f "$binary" ]; then
    echo "FAIL: original CMake capture module $name not found" >&2
    return 1
  fi
  printf '%s/%s\n' "$PWD" "$binary"
}
reference_capture=$(capture_reference mac-capture mac-capture)
reference_camera=$(capture_reference mac-avcapture mac-avcapture)
reference_legacy_camera=$(capture_reference mac-avcapture-legacy mac-avcapture)
soldr cargo build -p libobs-rust --release --target "$native_target" --target-dir "$native_rust"
OBS_BUILD_MACOS_DEPENDENCIES="$(dirname "$(dirname "$avcodec")")" \
  OBS_BUILD_MACOS_TARGET="$native_target" \
  OBS_BUILD_MACOS_OUTPUT="$native_output" \
  OBS_BUILD_MACOS_RUST_ARCHIVE="$native_rust/$native_target/release/liblibobs_rust.a" \
  OBS_BUILD_MACOS_REFERENCE="$PWD/$c_lib" \
  OBS_BUILD_MACOS_GRAPHICS=1 \
  OBS_BUILD_MACOS_OPENGL_REFERENCE="$reference_opengl" \
  OBS_BUILD_MACOS_METAL_REFERENCE="$reference_metal" \
  OBS_BUILD_MACOS_CAPTURE=1 \
  OBS_BUILD_MACOS_MAC_CAPTURE_REFERENCE="$reference_capture" \
  OBS_BUILD_MACOS_MAC_AVCAPTURE_REFERENCE="$reference_camera" \
  OBS_BUILD_MACOS_MAC_AVCAPTURE_LEGACY_REFERENCE="$reference_legacy_camera" \
  SWIFTC="$(xcrun --find swiftc)" \
  SDKROOT="$(xcrun --sdk macosx --show-sdk-path)" \
  soldr cargo test -p obs-studio-rust --features libobs-integration \
    --target "$native_target" --target-dir "$native_tests" \
    --test obs-tests-app native_libobs::macos_core_builds_from_the_canonical_manifest -- --exact

OBS_SYS_NATIVE_DEPENDENCIES="$(dirname "$(dirname "$avcodec")")" \
  OBS_BUILD_RUST_ARCHIVE="$native_rust/$native_target/release/liblibobs_rust.a" \
  soldr cargo check -p obs-sys --features bundled --target "$native_target" --target-dir "$native_tests"

source rust/tools/tier2-validate/macos-assertions.sh
