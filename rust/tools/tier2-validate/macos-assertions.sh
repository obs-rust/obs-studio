#!/usr/bin/env bash
# Shared assertion-only phase. Caller supplies prepared paths and ctest_jobs.
# Check each generated development framework contains the newly linked core.
# This is not release packaging. Existing test binaries and their
# assertions remain unchanged; dyld must prove they loaded the selected core.
for native_variant in obs-c-reference obs; do
  native_framework="$native_output/$native_variant/libobs.framework"
  cmp "$native_output/$native_variant/libobs.dylib" "$native_framework/Versions/A/libobs"
  cmp "$native_output/$native_variant/libobs.dylib" "$native_framework/libobs"
  native_dyld=(env DYLD_FRAMEWORK_PATH="$native_output/$native_variant" DYLD_LIBRARY_PATH="$(dirname "$avcodec")")
  native_probe_log="$native_output/$native_variant/loader.log"
  if ! "${native_dyld[@]}" DYLD_PRINT_LIBRARIES=1 "$probe" >"$native_probe_log" 2>&1; then
    tail -40 "$native_probe_log"
    exit 1
  fi
  if ! grep -qF "$native_framework/" "$native_probe_log"; then
    echo "FAIL: Cargo [$native_variant] did not load $native_framework" >&2
    exit 1
  fi
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
  echo "== Cargo [$native_variant] unchanged ctest suite"
  if ! "${native_dyld[@]}" ctest --test-dir "$build" -C RelWithDebInfo --output-on-failure "${ctest_jobs[@]}"; then
    DYLD_FRAMEWORK_PATH="$native_output/$native_variant" DYLD_LIBRARY_PATH="$(dirname "$avcodec")" \
      show_crash_stacks "$build"
    exit 1
  fi
done
