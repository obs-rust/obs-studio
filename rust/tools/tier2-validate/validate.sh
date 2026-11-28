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
phase=
while [ $# -gt 0 ]; do
  case "$1" in
  --phase)
    # Internal entry points used by ci/tier2_macos_tail.py after the build.
    phase="${2:-}"
    case "$phase" in cmake-tests|cargo-build|cargo-tests) ;; *) exit 2 ;; esac
    shift 2
    ;;
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

# OBS_CTEST_JOBS: ctest parallelism for each suite (default 1); CI sets it
# (#2138). Tests that must not overlap carry RUN_SERIAL or RESOURCE_LOCK.
ctest_jobs=(-j "${OBS_CTEST_JOBS:-1}")

# OBS_TIER2_RESIDUE=1: the cmocka/CTest executables were cross-built and signed on
# Linux (ci/macos_tier2_bundle.py). Build only what needs a native toolchain and
# run the prebuilt registry against the Cargo-native cores (#5009, #5848).
residue="${OBS_TIER2_RESIDUE:-0}"
bundle_work_name="${OBS_TIER2_BUNDLE_WORK:-build_cmocka_link_mac-arm64}"
build_target=tier2-native
if [ "$residue" = 1 ]; then build_target=tier2-residue; fi

build="build_rust"
if [ -z "$phase" ]; then
  cmocka_prefix="$(brew --prefix cmocka)"
  echo "== configure (libobs and libobs-c-reference)"
  cmake --preset macos-ci -B "$build" \
    -DENABLE_UNIT_TESTS=ON -DOBS_BUILD_C_REFERENCE=ON -DOBS_LIBOBS_BACKEND=HYBRID \
    -DOBS_BUILD_NATIVE_TIER2=ON \
    -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
    -DENABLE_WEBSOCKET=OFF \
    -DCMAKE_PREFIX_PATH="$cmocka_prefix" \
    >"$build.configure.log" 2>&1 || { tail -40 "$build.configure.log"; exit 1; }
  echo "== build (complete native Tier 2 graph)"
  cmake --build "$build" --config RelWithDebInfo --target "$build_target" \
    >"$build.build.log" 2>&1 || { grep -E ": (fatal )?error|BUILD FAILED" "$build.build.log" | sort -u | head -40; exit 1; }
fi

# libobs.framework twice: libobs (C, what the tests link) and its Rust twin
# in $build/libobs-c-reference (#1065), same install name.
c_lib=$(find "$build/libobs-c-reference" -path '*libobs.framework*' -name libobs -type f | head -1)
# Phase reentry must select CMake's hybrid, never a Cargo framework left by
# this run or a previous one under cargo-native-core.
rust_lib=$(find "$build/libobs" -path '*libobs.framework*' -name libobs -type f | head -1)
if [ -z "$c_lib" ] || [ -z "$rust_lib" ]; then
  echo "FAIL: libobs.framework not found (C: '${c_lib}', with Rust: '${rust_lib}') in $build"
  exit 1
fi
if [ -z "$phase" ]; then
  nm -gU "$c_lib" | awk '{print $3}' | sort >exports-OFF.txt
  nm -gU "$rust_lib" | awk '{print $3}' | sort >exports-ON.txt
  echo "== libobs exported symbols, C vs Rust"
  diff exports-OFF.txt exports-ON.txt
  echo "IDENTICAL ($(wc -l <exports-ON.txt | tr -d ' ') symbols)"
fi

# libobs in turn loads @rpath/libavcodec.dylib etc. from the pre-built
# obs-deps that the preset unpacked under .deps.
avcodec=$(find "$PWD/.deps" -path '*/lib/libavcodec*.dylib' | head -1)
if [ -z "$avcodec" ]; then
  echo "FAIL: obs-deps libavcodec not found under .deps"
  exit 1
fi

probe=$(find "$build/test/cmocka" -name test_bitstream -type f -perm -u+x 2>/dev/null | head -1)
if [ -z "$phase" ]; then
  # Only preparation overlaps: multiple CTest processes would race on their
  # shared logs, fixtures, fixed ports/temp names and process-local locks.
  uv run --no-project ci/tier2_macos_tail.py \
    --validator "$PWD/rust/tools/tier2-validate/validate.sh" \
    --log-dir "$build/tail-logs" --repeat "$repeat" --tests "$tests"
  exit 0
fi

if [ "$phase" = cmake-tests ] && [ "$residue" = 1 ]; then
  echo "== cmake-tests: covered by the prebuilt rust-tier2-macos-ctest bundle run"
  exit 0
fi
if [ "$phase" = cmake-tests ]; then
  for mode in OFF ON; do
    if [ "$mode" = ON ]; then lib=$rust_lib; else lib=$c_lib; fi
    # The test executables load @rpath/libobs.framework, and their build rpath
    # (@executable_path/../Frameworks) only holds it in an app bundle, so point
    # dyld at the framework's directory (absolute: ctest runs each test from
    # its own directory). That also picks which libobs the tests run against.
    framework_dir="$PWD/${lib%%/libobs.framework/*}"
    dyld=(env DYLD_FRAMEWORK_PATH="$framework_dir" DYLD_LIBRARY_PATH="$(dirname "$avcodec")")

    suite_logs="$PWD/$build/tail-logs/$phase/$mode"
    mkdir -p "$suite_logs"
    echo "== [$mode] ctest"
    if ! "${dyld[@]}" ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure "${ctest_jobs[@]}" \
      --output-log "$suite_logs/ctest.log"; then
      DYLD_FRAMEWORK_PATH="$framework_dir" DYLD_LIBRARY_PATH="$(dirname "$avcodec")" \
        show_crash_stacks "$build"
      exit 1
    fi

    # Produce and verify this suite's loader evidence here, after CTest.
    # Never pipe the probe into grep -q under pipefail: an early match closes
    # the pipe while dyld/the test is still writing and can cause SIGPIPE.
    probe_log="$suite_logs/loader.log"
    if ! "${dyld[@]}" DYLD_PRINT_LIBRARIES=1 "$probe" >"$probe_log" 2>&1; then
      tail -40 "$probe_log"
      echo "FAIL: [$mode] loader probe failed; see $probe_log"
      exit 1
    fi
    if ! grep -qF "$framework_dir/libobs.framework/" "$probe_log"; then
      tail -40 "$probe_log"
      echo "FAIL: [$mode] tests did not load libobs from $framework_dir; see $probe_log"
      exit 1
    fi
    echo "== [$mode] tests load $framework_dir/libobs.framework"

    if [ "$repeat" -gt 0 ]; then
      echo "== [$mode] ctest --repeat until-fail:$repeat${tests:+ -R $tests}"
      "${dyld[@]}" ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure \
        --output-log "$suite_logs/ctest-repeat.log" --repeat "until-fail:$repeat" ${tests:+-R "$tests"}
    fi
  done
  exit 0
fi

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
if [ "$phase" = cargo-build ]; then
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
  exit 0
fi

# Check each generated development framework contains the newly linked core.
# This is not release packaging. Existing test binaries and their
# assertions remain unchanged; dyld must prove they loaded the selected core.
for native_variant in obs-c-reference obs; do
  native_framework="$native_output/$native_variant/libobs.framework"
  cmp "$native_output/$native_variant/libobs.dylib" "$native_framework/Versions/A/libobs"
  cmp "$native_output/$native_variant/libobs.dylib" "$native_framework/libobs"
  native_dyld=(env DYLD_FRAMEWORK_PATH="$native_output/$native_variant" DYLD_LIBRARY_PATH="$(dirname "$avcodec")")
  suite_logs="$PWD/$build/tail-logs/$phase/$native_variant"
  mkdir -p "$suite_logs"
  graphics_count=0
  while IFS=$'\t' read -r renderer renderer_path graphics_probe; do
    case "$renderer" in
      OPENGL) reference_renderer=$reference_opengl; wrong_renderer=METAL ;;
      METAL) reference_renderer=$reference_metal; wrong_renderer=OPENGL ;;
      *) echo "FAIL: unexpected renderer $renderer" >&2; exit 1 ;;
    esac
    echo "== Cargo [$native_variant] $renderer module/core identity"
    "${native_dyld[@]}" "$graphics_probe" "$renderer_path" "$renderer" "$native_framework/Versions/A/libobs"
    "${native_dyld[@]}" "$graphics_probe" "$reference_renderer" "$renderer" "$native_framework/Versions/A/libobs"
    if "${native_dyld[@]}" "$graphics_probe" "$renderer_path" "$wrong_renderer" "$native_framework/Versions/A/libobs"; then
      echo 'FAIL: renderer negative control unexpectedly passed' >&2; exit 1
    else
      status=$?
      [ "$status" -eq 14 ] || { echo "FAIL: renderer negative control returned $status" >&2; exit 1; }
    fi
    if "${native_dyld[@]}" "$graphics_probe" "$renderer_path" "$renderer" "$PWD/$c_lib"; then
      echo 'FAIL: core negative control unexpectedly passed' >&2; exit 1
    else
      status=$?
      [ "$status" -eq 12 ] || { echo "FAIL: core negative control returned $status" >&2; exit 1; }
    fi
    graphics_count=$((graphics_count + 1))
  done <"$native_output/macos-graphics-paths.txt"
  [ "$graphics_count" -eq 2 ] || { echo 'FAIL: both graphics modules must be checked' >&2; exit 1; }
  capture_count=0
  while IFS=$'\t' read -r capture capture_path capture_data capture_probe; do
    case "$capture" in
      mac-capture) reference_module=$reference_capture; wrong_capture=mac-avcapture ;;
      mac-avcapture) reference_module=$reference_camera; wrong_capture=mac-avcapture-legacy ;;
      mac-avcapture-legacy) reference_module=$reference_legacy_camera; wrong_capture=mac-capture ;;
      *) echo "FAIL: unexpected capture module $capture" >&2; exit 1 ;;
    esac
    reference_data="$(dirname "$(dirname "$reference_module")")/Resources"
    [ -f "$capture_data/locale/en-US.ini" ] && [ -f "$reference_data/locale/en-US.ini" ] || {
      echo "FAIL: $capture locale data missing" >&2; exit 1;
    }
    diff -r "$capture_data" "$reference_data"
    echo "== Cargo [$native_variant] $capture module/core identity and source registration"
    "${native_dyld[@]}" "$capture_probe" "$capture_path" "$capture" "$capture_data" "$native_framework/Versions/A/libobs"
    "${native_dyld[@]}" "$capture_probe" "$reference_module" "$capture" "$reference_data" "$native_framework/Versions/A/libobs"
    if "${native_dyld[@]}" "$capture_probe" "$capture_path" "$wrong_capture" "$capture_data" "$native_framework/Versions/A/libobs"; then
      echo 'FAIL: capture negative control unexpectedly passed' >&2; exit 1
    else
      status=$?
      [ "$status" -eq 14 ] || { echo "FAIL: capture negative control returned $status" >&2; exit 1; }
    fi
    if "${native_dyld[@]}" "$capture_probe" "$capture_path" "$capture" "$capture_data" "$PWD/$c_lib"; then
      echo 'FAIL: capture core negative control unexpectedly passed' >&2; exit 1
    else
      status=$?
      [ "$status" -eq 12 ] || { echo "FAIL: capture core negative control returned $status" >&2; exit 1; }
    fi
    capture_count=$((capture_count + 1))
  done <"$native_output/macos-capture-paths.txt"
  [ "$capture_count" -eq 3 ] || { echo 'FAIL: all three capture modules must be checked' >&2; exit 1; }
  if [ "$residue" = 1 ]; then
    echo "== Cargo [$native_variant] prebuilt ctest suite"
    uv run --no-project ci/macos_tier2_bundle.py ctest-core --root "$(pwd -P)" --work-name "$bundle_work_name" \
      --core "$native_output/$native_variant/libobs.dylib" --variant "$native_variant" --jobs "${OBS_CTEST_JOBS:-1}" || exit 1
    continue
  fi
  echo "== Cargo [$native_variant] unchanged ctest suite"
  if ! "${native_dyld[@]}" ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure "${ctest_jobs[@]}" \
    --output-log "$suite_logs/ctest.log"; then
    DYLD_FRAMEWORK_PATH="$native_output/$native_variant" DYLD_LIBRARY_PATH="$(dirname "$avcodec")" \
      show_crash_stacks "$build"
    exit 1
  fi
  native_probe_log="$suite_logs/loader.log"
  if ! "${native_dyld[@]}" DYLD_PRINT_LIBRARIES=1 "$probe" >"$native_probe_log" 2>&1; then
    tail -40 "$native_probe_log"
    echo "FAIL: Cargo [$native_variant] loader probe failed; see $native_probe_log" >&2
    exit 1
  fi
  if ! grep -qF "$native_framework/" "$native_probe_log"; then
    tail -40 "$native_probe_log"
    echo "FAIL: Cargo [$native_variant] did not load $native_framework; see $native_probe_log" >&2
    exit 1
  fi
done
