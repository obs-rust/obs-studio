#!/bin/bash
# Tier 2 validation (docs/rust-port/testing-policy.md), run inside the
# container built from ./Dockerfile; see run.sh. Builds libobs, the cmocka
# tests and the decklink plugin with ENABLE_RUST_LIBOBS=OFF and =ON, runs the
# unchanged C tests in both, and fails unless both builds export exactly the
# same dynamic symbols from libobs. With --repeat N it then reruns the tests
# (or the --tests REGEX subset) up to N times per build to catch flakes, as
# testing-policy.md requires for threading and task.
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

git config --global --add safe.directory /src
mkdir -p /src
rsync -a --delete --exclude /target /ro/ /src/
# shellcheck source=stale-build.sh
source /src/rust/tools/linux-validate/stale-build.sh
# rustc as rust-toolchain.toml selects it, which is what Corrosion configures.
rustc_version=$(cd /src && rustc -V)
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

# Every libobs C file the Rust build replaces, derived from the
# ENABLE_RUST_LIBOBS guards (a NOT around the option means "left out when
# ON") instead of a hand-kept list (obs-rust/obs-studio#137). Included-when-ON
# files such as util/base-variadic.c have no NOT and are skipped. Paths stay
# relative to libobs/ and are matched under libobs.dir/, because libobs also
# links same-named files from deps (deps/libcaption/src/utf8.c).
ported_objs=$(grep -hE 'ENABLE_RUST_LIBOBS' /src/libobs/CMakeLists.txt /src/libobs/cmake/os-linux.cmake |
  grep 'NOT:' | grep -oE ':[A-Za-z0-9_./-]+\.c>' | sed -E 's|^:||; s|>$||; s|$|.o|' | sort -u)
if [ "$(wc -w <<<"$ported_objs")" -lt 20 ]; then
  echo "FAIL: derived only $(wc -w <<<"$ported_objs") ported objects from the ENABLE_RUST_LIBOBS guards"
  exit 1
fi
echo "== ported objects checked with ENABLE_RUST_LIBOBS=ON: $(wc -w <<<"$ported_objs")"

for mode in OFF ON; do
  build="/build/rust-$mode"
  reset_stale_build "$build" "$rustc_version"
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
  cmake --build "$build" --target libobs cmocka-tests decklink \
    >"$build.build.log" 2>&1 || { grep -E ": (fatal )?error|FAILED:" "$build.build.log" | sort -u | head -40; exit 1; }

  echo "== [$mode] ctest"
  ctest --test-dir "$build" --output-on-failure

  if [ "$repeat" -gt 0 ]; then
    echo "== [$mode] ctest --repeat until-fail:$repeat${tests:+ -R $tests}"
    ctest --test-dir "$build" --output-on-failure \
      --repeat "until-fail:$repeat" ${tests:+-R "$tests"}
  fi

  lib=$(ls "$build"/libobs/libobs.so.* | head -1)
  nm -D --defined-only "$lib" | awk '{print $3}' | sort >"/build/exports-$mode.txt"

  # Query the current Ninja build graph, not the filesystem: the /build volume
  # persists across runs, so a stale .o from an older checkout would linger.
  libobs_objs=$(ninja -C "$build" -t inputs libobs | grep -E '\.c\.o$' || true)
  if [ "$mode" = ON ]; then
    for obj in $ported_objs; do
      if grep -q "libobs\.dir/$obj\$" <<<"$libobs_objs"; then
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

# Issue #80: panic=abort is set only for [profile.release], so libobs must
# link the release-profile staticlib even for Debug builds; a dev-profile
# (panic=unwind) archive would let a shim panic unwind across the C ABI.
# Configure-only check: inspect which archive the Debug link line uses.
echo "== [ON/Debug] Rust staticlib profile"
dbg=/build/rust-ON-debug
reset_stale_build "$dbg" "$rustc_version"
cmake -S /src -B "$dbg" -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DENABLE_UNIT_TESTS=OFF -DENABLE_RUST_LIBOBS=ON \
  -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
  -DENABLE_WEBSOCKET=OFF -DENABLE_AJA=OFF -DENABLE_WEBRTC=OFF \
  -DENABLE_NVENC=OFF -DENABLE_QSV11=OFF -DENABLE_VST=OFF -DENABLE_WAYLAND=OFF \
  ${OBS_VERSION_OVERRIDE:+"-DOBS_VERSION_OVERRIDE=$OBS_VERSION_OVERRIDE"} \
  >"$dbg.configure.log" 2>&1 || { tail -40 "$dbg.configure.log"; exit 1; }
rust_archives=$(grep -ohE '/cargo/build/[^ $]*/liblibobs_rust\.a' "$dbg/build.ninja" | sort -u)
echo "$rust_archives"
if [ -z "$rust_archives" ] || grep -qv '/release/liblibobs_rust\.a$' <<<"$rust_archives"; then
  echo "FAIL: Debug libobs links a non-release (panic=unwind) Rust staticlib"
  exit 1
fi

echo "== libobs exported symbols, OFF vs ON"
diff /build/exports-OFF.txt /build/exports-ON.txt
echo "IDENTICAL ($(wc -l </build/exports-ON.txt) symbols)"

echo "== ccache"
ccache -s | grep -E "Hits|Misses|Cache size" || true
