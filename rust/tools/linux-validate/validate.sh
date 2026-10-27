#!/bin/bash
# Tier 2 validation (docs/rust-port/testing-policy.md), run inside the
# container built from ./Dockerfile; see run.sh. The explicit test build makes
# hybrid libobs and libobs-c-reference from shared unported objects. OFF loads
# the C reference; ON loads production hybrid libobs. Both export identical
# public symbols and run the unchanged tests. --repeat catches flakes.
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

# OBS_TIER2_STAGE: all (default) builds once and tests each mode here. CI
# splits it (#937): one job runs "build" (checks, the C/Rust export
# comparison, and OBS_TIER2_PACK=<dir> tars the tree), then "test" jobs unpack
# the tree at the same path and run one OBS_CTEST_SHARD=<i>/<n> of one mode.
stage="${OBS_TIER2_STAGE:-all}"
case "$stage" in
all | build | test) ;;
*) echo "FAIL: OBS_TIER2_STAGE must be all, build or test, got '$stage'"; exit 2 ;;
esac
shard_args=()
if [ -n "${OBS_CTEST_SHARD:-}" ]; then
  shard_index=${OBS_CTEST_SHARD%/*}
  shard_count=${OBS_CTEST_SHARD#*/}
  if ! [[ "$shard_index" =~ ^[1-9][0-9]*$ && "$shard_count" =~ ^[1-9][0-9]*$ ]] || [ "$shard_index" -gt "$shard_count" ]; then
    echo "FAIL: OBS_CTEST_SHARD must be index/count with 1 <= index <= count" >&2
    exit 2
  fi
  shard_args=(-I "$shard_index,,$shard_count")
fi
# OBS_CTEST_PART: run every test (default), only the RUN_SERIAL ones
# ("serial") or all but those ("suite"). CI runs the two parts of a mode on
# separate runners, so the serial tests keep a runner to themselves without
# idling the other cores (#2071).
part="${OBS_CTEST_PART:-}"
case "$part" in
"" | suite | serial) ;;
*) echo "FAIL: OBS_CTEST_PART must be suite or serial, got '$part'"; exit 2 ;;
esac
if [ -n "$part" ] && [ -n "${OBS_CTEST_SHARD:-}" ]; then
  echo "FAIL: OBS_CTEST_PART and OBS_CTEST_SHARD cannot be combined" >&2
  exit 2
fi
# Sets mode_shard_args to the part's tests of build tree $1, mode $2.
select_part() {
  local registry="$1/tier2-registry-$2.json" serial
  ctest --test-dir "$1" --show-only=json-v1 >"$registry"
  # OBS_CTEST_SERIAL_SHARD=<i>/<n>: the serial part's ith of n runners (#2072).
  serial=$(uv run --no-project "$SRC/rust/tools/linux-validate/serial-tests.py" "$registry" ${OBS_CTEST_SERIAL_SHARD:-})
  case "$part" in
  suite) mode_shard_args=(-E "$serial") ;;
  serial) mode_shard_args=(-R "$serial") ;;
  esac
}

# Roots: the container paths run.sh sets up (/ro: the checkout, mounted
# read-only and built out of source; /build: every build output), unless
# OBS_TIER2_SRC names a checkout to build in place, as the CI job does on the
# runner (#966).
SRC="${OBS_TIER2_SRC:-/ro}"
BUILD="${OBS_TIER2_BUILD:-/build}"
EXPORTS="${OBS_TIER2_EXPORTS:-/exports}"
# git runs as root inside the container (Docker harness or CI's job
# container) on a checkout owned by another user; without this, CMake can't
# read the version tag ("dubious ownership").
git config --global --add safe.directory "$SRC"
mkdir -p "$BUILD"
# The build writes nothing into the source tree: run.sh initializes the
# submodules on the host, and a fixed build number keeps
# cmake/common/buildnumber.cmake from writing cmake/.CMakeBuildNumber.
build_number=(-DOBS_BUILD_NUMBER=1)

# The same environment in both harnesses (#982): the container gave the
# tests root, /root and no locale; the native runner its `runner` user, a
# lived-in HOME, C.UTF-8 and its own /usr/local/bin cmake. Pin what can be
# pinned here, give ctest a fresh HOME (the build keeps the real one, which
# holds rustup's and soldr's state), and print what's left to compare.
export LANG=C.UTF-8 LC_ALL=C.UTF-8 TZ=UTC
# OBS_TIER2_KEEP_PATH=1 keeps the caller's PATH: a host run in a Nix shell
# (rust/tools/sanitize/shell.nix) has its tools there, and NixOS's /usr/bin
# (envfs) would shadow them with a cmake that can't find its modules.
if [ -z "${OBS_TIER2_KEEP_PATH:-}" ]; then
  export PATH="/usr/bin:/bin:$PATH"
fi
test_home="$BUILD/test-home"
rm -rf "$test_home"
mkdir -p "$test_home"
test_env=(env HOME="$test_home" XDG_CONFIG_HOME="$test_home/.config"
  XDG_DATA_HOME="$test_home/.local/share" XDG_CACHE_HOME="$test_home/.cache"
  # Eight test processes share the runner's CPUs; each software renderer must
  # not start another full-machine worker pool (#1634). Hardware drivers ignore it.
  LP_NUM_THREADS=1)
echo "== environment: user=$(id -un) nproc=$(nproc) locale=$LC_ALL" \
  "cmake=$(cmake --version | head -1 | awk '{print $3}')" \
  "cc=$(cc --version | head -1)" "kernel=$(uname -r)"
# shellcheck source=stale-build.sh
source "$SRC"/rust/tools/linux-validate/stale-build.sh
# rustc as rust-toolchain.toml selects it, which is what Corrosion configures.
# Only building needs rustc (stale-build detection); the CI test shards
# run an unpacked tree without a Rust toolchain (#937).
rustc_version=
if [ "$stage" != test ]; then
  rustc_version=$(cd "$SRC" && rustc -V)
  # Configure needs these source trees; an unpacked test artifact does not.
  for sub in plugins/obs-browser plugins/obs-websocket; do
    if [ -z "$(ls -A "$SRC/$sub" 2>/dev/null)" ]; then
      [ -w "$SRC" ] || { echo "FAIL: submodule $sub is empty in the read-only checkout; run.sh initializes it"; exit 1; }
      git -C "$SRC" submodule update --init --depth 1 "$sub"
    fi
  done
fi
cd "$SRC"

# Compiler cache: zccache as the CMake compiler launcher keeps cold builds (a
# fresh clone or another worktree, whose mtimes differ, or a CI runner) from
# recompiling OBS. ZCCACHE_CACHE_DIR defaults to the build volume; run.sh
# points it at a host dir (OBS_ZCCACHE_DIR, which CI restores and saves with
# actions/cache) or the checkout's zccache volume. The daemon starts on the first
# compile; stop it on exit so its index is flushed to the cache dir.
export ZCCACHE_CACHE_DIR="${ZCCACHE_CACHE_DIR:-$BUILD/zccache}"
trap 'zccache stop >/dev/null 2>&1 || true' EXIT

# Read the same canonical source sets as CMake and obs-build, then expand the
# selected ported groups into implementation object names. Scraping sources.cmake
# misses groups now that it imports the JSON manifest instead of listing them.
# The build below configures ENABLE_WAYLAND=OFF, so neither Wayland group (with
# or without GIO) is built.
ported_objs=$(uv run --no-project python "$SRC/rust/tools/ported_sources.py" --root "$SRC" --host linux --without-wayland)

if [ "$(wc -w <<<"$ported_objs")" -lt 20 ]; then
  echo "FAIL: derived only $(wc -w <<<"$ported_objs") ported objects from libobs/build/source-sets.json"
  exit 1
fi
echo "== ported C objects excluded from production libobs: $(wc -w <<<"$ported_objs")"

# OBS_TIER2_MODES: which libobs the tests run against (default both): OFF is
# libobs-c-reference, ON is the installed hybrid libobs. CI runs each mode as its own test
# jobs from the one build tree.
modes="${OBS_TIER2_MODES:-OFF ON}"
for mode in $modes; do
  case "$mode" in OFF | ON) ;; *) echo "FAIL: OBS_TIER2_MODES must list OFF and/or ON, got '$modes'"; exit 1 ;; esac
done

build="$BUILD/tree"
rust_libdir="$build/libobs"
c_libdir="$build/libobs-c-reference"
# RUNPATH whatever the toolchain's default: the loader searches a DT_RPATH
# before LD_LIBRARY_PATH, which would keep mode ON from putting
# libobs-c-reference in front of the libobs the tests link.
runpath_flags=(-DCMAKE_EXE_LINKER_FLAGS=-Wl,--enable-new-dtags
  -DCMAKE_SHARED_LINKER_FLAGS=-Wl,--enable-new-dtags -DCMAKE_MODULE_LINKER_FLAGS=-Wl,--enable-new-dtags)

if [ "$stage" = test ]; then
  [ -f "$build/CTestTestfile.cmake" ] || { echo "FAIL: no build tree at $build for OBS_TIER2_STAGE=test"; exit 1; }
else
  reset_stale_build "$build" "$rustc_version"
  echo "== configure + build (libobs and libobs-c-reference)"
  cmake -S "$SRC" -B "$build" -G Ninja \
    -DCMAKE_BUILD_TYPE=RelWithDebInfo \
    -DENABLE_UNIT_TESTS=ON -DOBS_BUILD_C_REFERENCE=ON -DOBS_LIBOBS_BACKEND=HYBRID \
    -DCMAKE_C_COMPILER_LAUNCHER=zccache -DCMAKE_CXX_COMPILER_LAUNCHER=zccache \
    ${OBS_EXTRA_CFLAGS:+"-DCMAKE_C_FLAGS=$OBS_EXTRA_CFLAGS" "-DCMAKE_CXX_FLAGS=$OBS_EXTRA_CFLAGS"} \
    "${runpath_flags[@]}" \
    ${OBS_EXTRA_CMAKE_ARGS:-} \
    -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
    -DENABLE_WEBSOCKET=OFF -DENABLE_AJA=OFF -DENABLE_WEBRTC=OFF \
    -DENABLE_NVENC=ON -DENABLE_QSV11=OFF -DENABLE_VST=OFF -DENABLE_WAYLAND=OFF \
    ${OBS_VERSION_OVERRIDE:+"-DOBS_VERSION_OVERRIDE=$OBS_VERSION_OVERRIDE"} "${build_number[@]}" \
    >"$build.configure.log" 2>&1 || { tail -40 "$build.configure.log"; exit 1; }
  # -k 0: keep going after a failed target, so one run reports every compile
  # error (a warning ratchet otherwise took a round trip per file; #634).
  # A compile the zccache launcher itself fails exits 125 with no compiler
  # output: every in-flight compile failed at once when its daemon died (master
  # run 39002068564). A request the daemon accepted and then lost under load
  # exits 1 instead, with only "zccache[err][R]: daemon response failure"
  # (runs 39376294047, 39380784311): zccache keeps that exit 1 by design
  # (wrap/ipc/lost_request.rs) and leaves the replay to the build tool.
  # Restart the daemon and rebuild once; ninja redoes only the failed edges,
  # and a real compile error fails the second time too.
  tier2_targets=(libobs libobs-c-reference cmocka-tests decklink obs-nvenc obs-nvenc-c-reference)
  uv run --script "$SRC/rust/tools/linux-validate/drop_depless_objects.py" "$build"
  if ! cmake --build "$build" --target "${tier2_targets[@]}" -- -k 0 >"$build.build.log" 2>&1; then
    # Ninja colors its log (\e[31mFAILED: ...), so strip the escapes first.
    launcher_failed=$(sed 's/\x1b\[[0-9;]*m//g' "$build.build.log" | grep -c '^FAILED: \[code=125\]' || true)
    lost_requests=$(grep -c '^zccache\[err\]\[R\]: daemon response failure' "$build.build.log" || true)
    if [ "$launcher_failed" -gt 0 ] || [ "$lost_requests" -gt 0 ]; then
      echo "== zccache launcher failures: $launcher_failed exit 125, $lost_requests requests the daemon lost; restarting it and rebuilding once"
      # Keep the original cause: the retry can report only a recovery cooldown.
      cp "$build.build.log" "$build.build-first-failure.log"
      "$SRC"/rust/tools/linux-validate/build-errors.sh "$build.build-first-failure.log"
      echo "== zccache status before recovery"
      daemon_unavailable=0
      if ! zccache status 2>&1 | head -60; then
        echo "::warning::zccache daemon unavailable; the single recovery build will compile without caching"
        daemon_unavailable=1
        if [ -n "${GITHUB_OUTPUT:-}" ]; then
          echo 'cache-healthy=false' >>"$GITHUB_OUTPUT"
        fi
      fi
      echo "== zccache stop for recovery"
      zccache stop 2>&1 | head -40 || true
      (
        if [ "$daemon_unavailable" = 1 ]; then export ZCCACHE_DISABLE=1; fi
        cmake --build "$build" --target "${tier2_targets[@]}" -- -k 0
      ) >"$build.build.log" 2>&1 ||
        { "$SRC"/rust/tools/linux-validate/build-errors.sh "$build.build.log"; exit 1; }
    else
      "$SRC"/rust/tools/linux-validate/build-errors.sh "$build.build.log"
      exit 1
    fi
  fi
  if [ -n "${OBS_EXTRA_CFLAGS:-}" ]; then
    # Measuring extra flags: show what they report (#516).
    echo "== warnings"
    grep -E ': warning: ' "$build.build.log" | sort -u || true
  fi

  # Through the libobs.so link this build wrote: the /build volume persists,
  # so an older libobs.so.* (another SONAME) can still sit beside it.
  [ -e "$rust_libdir/libobs.so" ] || { echo "FAIL: libobs-c-reference was not built"; exit 1; }
  c_lib=$(realpath "$c_libdir/libobs.so")
  rust_lib=$(realpath "$rust_libdir/libobs.so")
  soname() { readelf -d "$1" | sed -nE 's/.*\(SONAME\).*\[(.*)\]/\1/p'; }
  if [ "$(soname "$c_lib")" != "$(soname "$rust_lib")" ]; then
    echo "FAIL: libobs-c-reference's SONAME $(soname "$rust_lib") is not libobs's $(soname "$c_lib")"
    exit 1
  fi
  nm -D --defined-only "$c_lib" | awk '{print $3}' | sort >"$BUILD/exports-OFF.txt"
  nm -D --defined-only "$rust_lib" | awk '{print $3}' | sort >"$BUILD/exports-ON.txt"
  # $EXPORTS: run.sh mounts OBS_TIER2_EXPORTS_DIR there, or the CI job names a
  # runner dir with OBS_TIER2_EXPORTS, so a job can hand the lists on.
  if [ -d "$EXPORTS" ]; then
    cp "$BUILD/exports-OFF.txt" "$BUILD/exports-ON.txt" "$EXPORTS/"
  fi
  echo "== libobs exported symbols, C vs Rust"
  diff "$BUILD/exports-OFF.txt" "$BUILD/exports-ON.txt"
  echo "IDENTICAL ($(wc -l <"$BUILD/exports-ON.txt") symbols)"

  # Query the current Ninja build graph, not the filesystem: the /build volume
  # persists across runs, so a stale .o from an older checkout would linger.
  # compiles <target> <source> <object>: <source> (a path below the source
  # tree) is compiled into <target>, as its own <object> (the path Ninja
  # names it by below <target>.dir) or, with ENABLE_UNITY_BUILD,
  # inside one of the target's unity batches (<target>.dir/Unity/unity_N_c.c
  # includes it). Queries the current Ninja graph and generated batches, not
  # stale objects on the persistent /build volume.
  # A unity group (BUILD.md) counts when <target> compiles a group whose file
  # includes <source>.
  compiles() {
    local target=$1 src=$2 object=$3 unity group
    ninja -C "$build" -t inputs "$target" 2>/dev/null | grep -qF "$target.dir/$object" && return 0
    for group in $(ninja -C "$build" -t inputs "$target" 2>/dev/null |
      sed -n "s|.*CMakeFiles/$target\.dir/\(build/.*+\.h\.[cm]\)\.o\$|\1|p"); do
      grep -qF "\"$src\"" "$SRC/libobs/$group" 2>/dev/null && return 0
    done
    unity=$(find "$build" -type d -path "*/CMakeFiles/$target.dir/Unity" | head -1)
    [ -n "$unity" ] && sed -n 's/^#include "\(.*\)"$/\1/p' "$unity"/*.c 2>/dev/null | grep -q "/$src\$"
  }
  for obj in $ported_objs; do
    src="libobs/${obj%.o}"
    if compiles libobs "$src" "$obj"; then
      echo "FAIL: $obj was compiled into hybrid libobs"
      exit 1
    fi
    if ! compiles libobs-c-reference "$src" "$obj"; then
      echo "FAIL: $obj was not compiled into libobs-c-reference"
      exit 1
    fi
  done
  plugin=$(find "$build" -name 'decklink.so' | head -1)
  echo "== decklink.so imports:"
  nm -D --undefined-only "$plugin" | grep bitstream_reader

  # Issue #645: obs-nvenc uses cuda-load.c, its never-installed twin
  # obs-nvenc-c-reference cuda-load-rust.c (#592, #1065). Both must link.
  for target in obs-nvenc obs-nvenc-c-reference; do
    # Each loader is a unity group of its own (BUILD.md).
    if [ "$target" = obs-nvenc ]; then want=cuda-load-rust not=cuda-load; else want=cuda-load not=cuda-load-rust; fi
    if ! compiles "$target" - "build/obs-nvenc.$want+.h.c.o" || compiles "$target" - "build/obs-nvenc.$not+.h.c.o"; then
      echo "FAIL: $target must compile $want and not $not"
      exit 1
    fi
  done
  [ -n "$(find "$build/plugins" -name 'obs-nvenc.so' | head -1)" ] || { echo "FAIL: obs-nvenc.so was not built"; exit 1; }
  [ -f "$build/obs-nvenc-c-reference/obs-nvenc.so" ] || { echo "FAIL: obs-nvenc-c-reference was not built"; exit 1; }
  echo "== obs-nvenc.so built with cuda-load-rust.c.h, obs-nvenc-c-reference with cuda-load.c.h"

  # OBS_TIER2_SHARDS=<n>: plan n shards per mode by recorded test duration
  # (ctest-cost-<mode>.txt in the zccache dir, #931) into the tree (#1048).
  if [ -n "${OBS_TIER2_SHARDS:-}" ]; then
    names="$BUILD/tier2-names.txt"
    ctest --test-dir "$build" -N | sed -nE 's/^ *Test +#[0-9]+: +([^ ]+)$/\1/p' >"$names"
    for mode in $modes; do
      costs="${OBS_TIER2_COST_DIR:-${ZCCACHE_CACHE_DIR:-}}/ctest-cost-$mode.txt"
      [ -s "$costs" ] || costs=/dev/null
      echo "== [$mode] shard plan ($(wc -l <"$names") tests, costs from ${costs})"
      uv run --script "$SRC/rust/tools/linux-validate/shard-plan.py" "$costs" "$names" "$OBS_TIER2_SHARDS" "$build/tier2-shards/$mode"
    done
  fi

  if [ -n "${OBS_TIER2_PACK:-}" ]; then
    # What ctest needs at the same path on a test runner: binaries, libs,
    # plugins, CTestTestfile.cmake. Objects, archives and cargo's target dir
    # are build inputs only.
    mkdir -p "$OBS_TIER2_PACK"
    tar --zstd -cf "$OBS_TIER2_PACK/tier2-tree.tar.zst" -C "$BUILD" \
      --exclude='*.o' --exclude='*.a' --exclude='*.rlib' --exclude='*.rmeta' \
      --exclude="tree/cargo" tree
    echo "== packed $(du -h "$OBS_TIER2_PACK/tier2-tree.tar.zst" | cut -f1)"
  fi
fi

if [ "$stage" != build ]; then
  for mode in $modes; do
    # Select the reference or production library through LD_LIBRARY_PATH.
    # The tests' RUNPATH yields to it. Check it on
    # one test binary, so a silent fallback to the C library fails here.
    mode_env=("${test_env[@]}")
    want_dir="$c_libdir"
    if [ "$mode" = ON ]; then want_dir="$rust_libdir"; fi
    mode_env+=(LD_LIBRARY_PATH="$want_dir${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}")
    probe=$(find "$build/test/cmocka" -maxdepth 1 -type f -name 'test_bitstream' | head -1)
    resolved=$("${mode_env[@]}" ldd "$probe" | awk '$1 ~ /^libobs\.so/ {print $3}')
    if [ "$(dirname "$(realpath "$resolved")")" != "$(realpath "$want_dir")" ]; then
      echo "FAIL: [$mode] tests load libobs from $resolved, expected $want_dir"
      exit 1
    fi
    echo "== [$mode] tests load $resolved"

    # ctest starts the longest tests first only when it knows their past run
    # times (Testing/Temporary/CTestCostData.txt). A fresh CI container has
    # none, so the 83 s test_deinterlace_2x_field_order could start last; carry
    # the file per mode in OBS_TIER2_COST_DIR (default: the zccache directory,
    # #931); CI shards upload it and master merges it back (#930).
    cost_dir="${OBS_TIER2_COST_DIR:-${ZCCACHE_CACHE_DIR:-}}"
    cost_file="$build/Testing/Temporary/CTestCostData.txt"
    cost_cache=
    rm -f "$cost_file"
    if [ -n "$cost_dir" ]; then
      mkdir -p "$cost_dir"
      cost_cache="$cost_dir/ctest-cost-$mode.txt"
      if [ -s "$cost_cache" ]; then
        mkdir -p "${cost_file%/*}"
        cp "$cost_cache" "$cost_file"
      fi
    fi

    # A shard runs the build job's duration-balanced test list when the tree
    # carries one (#1048), else ctest -I striping.
    mode_shard_args=("${shard_args[@]}")
    plan="$build/tier2-shards/$mode/${shard_index:-}-of-${shard_count:-}.regex"
    if [ -n "${OBS_CTEST_SHARD:-}" ]; then
      if [ ! -s "$plan" ]; then
        echo "FAIL: missing Tier 2 shard plan: $plan" >&2
        exit 1
      fi
      # Verify the union against this runner's actual CTest registry, not just
      # the build job's names file. Never silently fall back to striped tests.
      registry="$build/tier2-registry-$mode.json"
      ctest --test-dir "$build" --show-only=json-v1 >"$registry"
      uv run "$SRC/rust/tools/linux-validate/check-plan.py" "$registry" "$build/tier2-shards/$mode" "$shard_count"
      mode_shard_args=(-R "$(cat "$plan")")
    fi

    [ -z "$part" ] || select_part "$build" "$mode"
    echo "== [$mode] ctest${OBS_CTEST_SHARD:+ shard $OBS_CTEST_SHARD}${part:+ ($part)}"
    # xvfb-run gives display tests an X server; the PipeWire tests start their
    # own private PipeWire daemon and D-Bus bus (#776).
    # Tests share this display. Do not reset it when its last client exits:
    # another test can be opening its hotkey connection at that boundary.
    xvfb_args=(-a '--server-args=-screen 0 1280x1024x24 -noreset')
    ctest_status=0
    "${mode_env[@]}" xvfb-run "${xvfb_args[@]}" ctest --test-dir "$build" --output-on-failure -j "${OBS_CTEST_JOBS:-1}" "${mode_shard_args[@]}" 2>&1 | tee "$build.ctest-$mode.log" ||
      ctest_status=$?
    # Keep the timings even when a test failed: they are still right.
    if [ -n "$cost_cache" ] && [ -s "$cost_file" ]; then
      cp "$cost_file" "$cost_cache"
    fi
    [ "$ctest_status" -eq 0 ] || exit "$ctest_status"
    # A PipeWire test that skips for lack of a daemon is a hole, not a pass (#776).
    uv run --no-project "$SRC"/rust/tools/linux-validate/check_skips.py "$build.ctest-$mode.log"

    if [ "$repeat" -gt 0 ]; then
      echo "== [$mode] ctest --repeat until-fail:$repeat${tests:+ -R $tests}"
      "${mode_env[@]}" xvfb-run "${xvfb_args[@]}" ctest --test-dir "$build" --output-on-failure \
        --repeat "until-fail:$repeat" ${tests:+-R "$tests"}
    fi
  done
fi

if [ "$stage" = test ]; then
  exit 0
fi

# Issue #80: panic=abort is set only for [profile.release], so
# libobs-c-reference must link the release-profile staticlib even for Debug
# builds; a dev-profile (panic=unwind) archive would let a shim panic unwind
# across the C ABI. Configure-only check: inspect which archive the Debug link
# line uses.
echo "== [Debug] Rust staticlib profile"
dbg="$BUILD/tree-debug"
reset_stale_build "$dbg" "$rustc_version"
cmake -S "$SRC" -B "$dbg" -G Ninja -DCMAKE_BUILD_TYPE=Debug \
  -DENABLE_UNIT_TESTS=OFF \
  -DENABLE_FRONTEND=OFF -DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF \
  -DENABLE_WEBSOCKET=OFF -DENABLE_AJA=OFF -DENABLE_WEBRTC=OFF \
  -DENABLE_NVENC=OFF -DENABLE_QSV11=OFF -DENABLE_VST=OFF -DENABLE_WAYLAND=OFF \
  ${OBS_VERSION_OVERRIDE:+"-DOBS_VERSION_OVERRIDE=$OBS_VERSION_OVERRIDE"} "${build_number[@]}" \
  >"$dbg.configure.log" 2>&1 || { tail -40 "$dbg.configure.log"; exit 1; }
rust_archives=$(grep -ohE '/cargo/build/[^ $]*/liblibobs_rust\.a' "$dbg/build.ninja" | sort -u)
echo "$rust_archives"
if [ -z "$rust_archives" ] || grep -qv '/release/liblibobs_rust\.a$' <<<"$rust_archives"; then
  echo "FAIL: Debug libobs-c-reference links a non-release (panic=unwind) Rust staticlib"
  exit 1
fi

echo "== zccache"
zccache status | grep -E "Compilations|Hit rate|Artifacts" || true
