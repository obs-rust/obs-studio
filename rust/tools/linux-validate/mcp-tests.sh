#!/bin/bash
# Linux Tier 2: the in-process MCP server over the native libobs (#3319, epic
# #3283). obs-tests-app's mcp_stdio, mcp_libobs_backend and mcp_libobs_video
# cases exist only under the bundled-native feature, which needs the release
# libobs-rust staticlib (OBS_BUILD_RUST_ARCHIVE) and, for the screenshot and
# record cases, a graphics module, an X display and the plugins that provide
# color_source_v3, ffmpeg_muxer, obs_x264 and ffmpeg_aac. No other lane builds
# them, so this step runs them after validate.sh's native build, with
# OBS_MCP_REQUIRE_VIDEO=1 so a missing piece fails instead of skipping.
#
# usage: mcp-tests.sh <cmake build tree> <source tree>
set -euo pipefail

build=$(realpath "$1")
src=$(realpath "$2")

echo "== MCP: build the graphics module and plugins the video cases load"
cmake --build "$build" --target libobs-opengl image-source obs-ffmpeg obs-x264

# Everything below is located in the tree rather than assumed, so a CMake
# layout change fails here by name.
find_one() {
  local found
  found=$(find "$build" -type f -name "$1" -not -path '*/CMakeFiles/*' | head -1)
  [ -n "$found" ] || { echo "FAIL: $1 was not built under $build" >&2; exit 1; }
  realpath "$found"
}
archive=$(find "$build" -type f -name liblibobs_rust.a -path '*/release/*' | head -1)
[ -n "$archive" ] || { echo "FAIL: no release liblibobs_rust.a under $build" >&2; exit 1; }
archive=$(realpath "$archive")
opengl=$(find_one libobs-opengl.so)

# One flat plugin directory: obs_add_module_path loads every module in it.
modules="$build.mcp-modules"
rm -rf "$modules"
mkdir -p "$modules"
for plugin in image-source obs-ffmpeg obs-x264; do
  ln -s "$(find_one "$plugin.so")" "$modules/$plugin.so"
done
echo "== MCP: archive=$archive graphics=$opengl modules=$(ls "$modules" | tr '\n' ' ')"

echo "== MCP: obs-tests-app mcp cases under xvfb-run"
OBS_BUILD_RUST_ARCHIVE="$archive" \
  OBS_MCP_REQUIRE_VIDEO=1 \
  OBS_MCP_GRAPHICS_MODULE="$opengl" \
  OBS_MCP_MODULE_BIN="$modules" \
  OBS_MCP_MODULE_DATA="$src/plugins/%module%/data" \
  OBS_MCP_DATA_PATH="$src/libobs/data" \
  LD_LIBRARY_PATH="$build/libobs${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  LP_NUM_THREADS=1 \
  xvfb-run -a '--server-args=-screen 0 1280x1024x24 -noreset' \
  soldr cargo test -p obs-studio-rust --features bundled-native \
    --target-dir "$build.mcp-tests" --test obs-tests-app -- mcp
