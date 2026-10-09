#!/bin/bash
# Tier 2 validation (docs/rust-port/testing-policy.md), run inside the
# container built from ./Dockerfile; see run.sh. Builds libobs, the cmocka
# tests and the decklink plugin with ENABLE_RUST_LIBOBS=OFF and =ON, runs the
# unchanged C tests in both, and fails unless both builds export exactly the
# same dynamic symbols from libobs.
set -euo pipefail

git config --global --add safe.directory /src
mkdir -p /src
rsync -a --delete --exclude /target /ro/ /src/
cd /src
for sub in plugins/obs-browser plugins/obs-websocket; do
  if [ -z "$(ls -A "$sub" 2>/dev/null)" ]; then
    git submodule update --init --depth 1 "$sub"
  fi
done

# Compiler cache: keeps cold builds (a fresh clone or another worktree, whose
# mtimes differ, or a CI runner) from recompiling OBS. CCACHE_DIR defaults to
# the build volume; run.sh points it at a host dir when OBS_CCACHE_DIR is set
# (CI restores and saves that dir with actions/cache).
export CCACHE_DIR="${CCACHE_DIR:-/build/ccache}"
export CCACHE_BASEDIR=/src CCACHE_COMPRESS=1 CCACHE_MAXSIZE="${CCACHE_MAXSIZE:-2G}"
export CCACHE_SLOPPINESS=time_macros,include_file_mtime,include_file_ctime
ccache -z >/dev/null

for mode in OFF ON; do
  build="/build/rust-$mode"
  echo "== [$mode] configure + build"
  cmake -S /src -B "$build" -G Ninja \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DENABLE_UNIT_TESTS=ON \
    -DCMAKE_C_COMPILER_LAUNCHER=ccache -DCMAKE_CXX_COMPILER_LAUNCHER=ccache \
    -DENABLE_RUST_LIBOBS="$mode" \
    -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
    -DENABLE_WEBSOCKET=OFF -DENABLE_AJA=OFF -DENABLE_WEBRTC=OFF \
    -DENABLE_NVENC=OFF -DENABLE_QSV11=OFF -DENABLE_VST=OFF -DENABLE_WAYLAND=OFF \
    ${OBS_VERSION_OVERRIDE:+"-DOBS_VERSION_OVERRIDE=$OBS_VERSION_OVERRIDE"} \
    >"$build.configure.log" 2>&1 || { tail -40 "$build.configure.log"; exit 1; }
  cmake --build "$build" --target libobs test_bitstream test_darray test_serializer test_os_path test_svt_av1 test_avc test_formatted_filename decklink \
    >"$build.build.log" 2>&1 || { grep -E "error|Error" "$build.build.log" | head -40; exit 1; }

  echo "== [$mode] ctest"
  ctest --test-dir "$build" --output-on-failure

  lib=$(ls "$build"/libobs/libobs.so.* | head -1)
  nm -D --defined-only "$lib" | awk '{print $3}' | sort >"/build/exports-$mode.txt"

  # Query the current Ninja build graph, not the filesystem: the /build volume
  # persists across runs, so a stale .o from an older checkout would linger.
  libobs_objs=$(ninja -C "$build" -t inputs libobs | grep -E '\.c\.o$' || true)
  if [ "$mode" = ON ]; then
    for obj in bitstream.c.o array-serializer.c.o path-extension.c.o; do
      if grep -q "/$obj\$" <<<"$libobs_objs"; then
        echo "FAIL: $obj was compiled into libobs with ENABLE_RUST_LIBOBS=ON"
        exit 1
      fi
    done
  fi
  if [ "$mode" = OFF ] && ! grep -q '/path-extension.c.o$' <<<"$libobs_objs"; then
    echo "FAIL: path-extension.c.o was not compiled into libobs with ENABLE_RUST_LIBOBS=OFF"
    exit 1
  fi
  plugin=$(find "$build" -name 'decklink.so' | head -1)
  echo "== [$mode] decklink.so imports:"
  nm -D --undefined-only "$plugin" | grep bitstream_reader
done

echo "== libobs exported symbols, OFF vs ON"
diff /build/exports-OFF.txt /build/exports-ON.txt
echo "IDENTICAL ($(wc -l </build/exports-ON.txt) symbols)"

echo "== ccache"
ccache -s | grep -E "Hits|Misses|Cache size" || true
