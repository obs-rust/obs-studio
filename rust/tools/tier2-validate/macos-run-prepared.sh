#!/usr/bin/env bash
# Explicit prototype consumer. No compiler or Cargo is invoked here.
set -euo pipefail
: "${OBS_TIER2_BUILD:?set the common prepared build root}"
: "${OBS_TIER2_NATIVE_OUTPUT:?set the common Cargo-native output root}"
: "${OBS_TIER2_DEPS:?set the staged Darwin dependency directory}"
: "${OBS_TIER2_CONTRACT:?set the prebuilt obs-tests-app executable}"
: "${OBS_BUILD_MACOS_RUST_ARCHIVE:?set the prepared archive path for contract selection}"
build=$OBS_TIER2_BUILD
native_output=$OBS_TIER2_NATIVE_OUTPUT
avcodec=$OBS_TIER2_DEPS/libavcodec.dylib
c_lib=$build/cores/reference/libobs.framework/Versions/A/libobs
probe=$build/test/cmocka/test_bitstream
reference_opengl=${OBS_BUILD_MACOS_OPENGL_REFERENCE:?set the CMake OpenGL image}
reference_metal=${OBS_BUILD_MACOS_METAL_REFERENCE:?waiting on Linux Swift and SDK 26.5}
reference_capture=${OBS_BUILD_MACOS_MAC_CAPTURE_REFERENCE:?set the CMake screen/audio image}
reference_camera=${OBS_BUILD_MACOS_MAC_AVCAPTURE_REFERENCE:?set the CMake camera image}
reference_legacy_camera=${OBS_BUILD_MACOS_MAC_AVCAPTURE_LEGACY_REFERENCE:?set the CMake legacy camera image}
ctest_jobs=(-j "${OBS_CTEST_JOBS:-3}")
# Diagnostic replay is optional in the ordinary validator too; CTest failure
# remains the result. Never turn a failed native contract into a successful run.
show_crash_stacks() { return 0; }
export OBS_TIER2_FIXTURE_PREPARED=1
export OBS_BUILD_MACOS_ASSERT_ONLY=1
export OBS_BUILD_MACOS_OUTPUT=$native_output
export OBS_BUILD_MACOS_DEPENDENCIES=$OBS_TIER2_DEPS
export OBS_BUILD_MACOS_CAPTURE=1 OBS_BUILD_MACOS_GRAPHICS=1
export OBS_BUILD_MACOS_REFERENCE=$c_lib
export OBS_BUILD_MACOS_MODULES=$build/tier2-modules.txt
exports=$build/runtime-exports
mkdir -p "$exports"
nm -gUj "$c_lib" | LC_ALL=C sort > "$exports/reference"
nm -gUj "$build/cores/shared/libobs.framework/Versions/A/libobs" | LC_ALL=C sort > "$exports/shared"
test -s "$exports/reference"
diff -u "$exports/reference" "$exports/shared"
for variant in reference shared; do
  framework=$build/cores/$variant
  loader_log=$build/cores/$variant/loader.log
  env DYLD_FRAMEWORK_PATH="$framework" DYLD_LIBRARY_PATH="$OBS_TIER2_DEPS" \
    DYLD_PRINT_LIBRARIES=1 "$probe" > "$loader_log" 2>&1
  grep -qF "$framework/libobs.framework/" "$loader_log"
  env DYLD_FRAMEWORK_PATH="$framework" DYLD_LIBRARY_PATH="$OBS_TIER2_DEPS" \
    ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure "${ctest_jobs[@]}" --no-tests=error
done
"$OBS_TIER2_CONTRACT" native_libobs::macos_core_builds_from_the_canonical_manifest --exact
source "$(dirname "${BASH_SOURCE[0]}")/macos-assertions.sh"
