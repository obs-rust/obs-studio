#!/usr/bin/env bash
# Portability and runtime test for the OBS Studio AppImage. Run inside a stock
# distribution container that has NO OBS dependencies installed, one scenario
# per container, so no scenario can see what another one left behind:
#   test-appimage.sh <path-to.AppImage> <output-dir> <path-to-obs-appimage-testkit> \
#       <bundle|first-start|probes|saved-profile>
# Only generic test infrastructure (X server, window manager, screenshot tools,
# fonts) and a stand-in for the host GPU driver (Mesa) are installed at first;
# a stand-in desktop session (D-Bus, PipeWire, WirePlumber, pipewire-pulse,
# GStreamer test sources) only after the bundle has been checked. Every library
# OBS needs must come out of the AppImage.
#
# Scenarios:
#   1. bundle: every path AppRun points OBS at exists (check-appdir.sh), and
#      every bundled ELF file resolves all of its libraries and symbols with
#      the bundled loader and its own RPATH. With glibc bundled, the glibc
#      libraries host GPU drivers link (librt, libpthread, libdl, ...) must
#      come from the bundle too.
#   2. first-start: a new user's OBS opens a window that renders.
#   3. probes: the AppImage's own PipeWire, PulseAudio, gconv and D-Bus
#      libraries, in AppRun's environment, work against a private session.
#   4. saved-profile: OBS starts with a returning user's scene that holds a
#      PipeWire screen capture (through a fake ScreenCast portal), a PipeWire
#      camera (through a fake Camera portal) and a PulseAudio microphone, all
#      fed by test producers, and records them. It must stay up, map PipeWire,
#      PulseAudio and GLib from the AppImage, load every module it ships, and
#      start every source. The recording is left in <output-dir> for
#      verify-recording.sh. (Every continuous build before
#      obs-rust/obs-studio#426 crashed here on start.) Only on Ubuntu 22.04,
#      whose PipeWire 0.3.x cannot run the test producers, it runs without
#      them: OBS must still get its PipeWire connection from the portal, stay
#      up and quit cleanly; there is no recording to check.
#
# Every scenario ends by stopping what it started and failing if any of it is
# still running or an OBS instance socket is still open.
set -euo pipefail

# Nothing from the caller's session may leak into the test.
unset DISPLAY WAYLAND_DISPLAY DBUS_SESSION_BUS_ADDRESS PIPEWIRE_REMOTE PIPEWIRE_RUNTIME_DIR \
    PIPEWIRE_CONFIG_DIR PIPEWIRE_MODULE_DIR SPA_PLUGIN_DIR PULSE_SERVER XDG_RUNTIME_DIR \
    HYPRLAND_INSTANCE_SIGNATURE

APPIMAGE="$(realpath "$1")"
OUT="$(realpath -m "${2:-.}")"
USAGE="usage: test-appimage.sh <AppImage> <output-dir> <obs-appimage-testkit> <bundle|first-start|probes|saved-profile|saved-profile-custom|saved-profile-existing|gpu>"
TESTKIT="$(realpath "${3:?$USAGE}")"
SCENARIO="${4:?$USAGE}"
case "$SCENARIO" in
bundle | first-start | probes | saved-profile | saved-profile-custom | saved-profile-existing | gpu) ;;
*)
    echo "::error::unknown scenario $SCENARIO; $USAGE"
    exit 1
    ;;
esac
HERE_DIR="$(dirname "$(realpath "$0")")"
mkdir -p "$OUT"
cd "$(mktemp -d)"
FAILED=()
GUI_NAME=obs
CONFIG_NAME=obs-studio
INSTANCE_PREFIX=@/com/obsproject
fail() {
    echo "::error::$*"
    FAILED+=("$*")
}

# Every process this script starts is recorded and stopped on exit, with its
# descendants. Nothing else is ever signalled.
OWN_PIDS=()
descendants() {
    local p
    for p in $(ps -o pid= --ppid "$1" 2>/dev/null); do
        descendants "$p"
        echo "$p"
    done
}
stop_tree() {
    local p
    for p in $(descendants "$1") "$1"; do kill "${2:--TERM}" "$p" 2>/dev/null || true; done
}
cleanup() {
    local p
    for p in "${OWN_PIDS[@]}"; do stop_tree "$p"; done
    sleep 1
    for p in "${OWN_PIDS[@]}"; do stop_tree "$p" -KILL; done
}
trap cleanup EXIT
# Exit status of an OBS run, decoded: AppRun passes the status of obs through.
describe_exit() {
    case "$1" in
    0) echo "exited normally" ;;
    13[0-9] | 14[0-9]) echo "crashed with SIG$(kill -l $(($1 - 128)))" ;;
    *) echo "exited with status $1" ;;
    esac
}
# Process $1 exists and is not a zombie.
alive() {
    [ -r "/proc/$1/stat" ] && [ "$(awk '{print $3}' "/proc/$1/stat" 2>/dev/null)" != Z ]
}
# Ends the scenario: stops everything it started, fails if any of it (or any
# OBS) is still running 10 s later or an OBS instance socket is still open, and
# reports. $1: an optional note for the summary line.
finish() {
    local p left
    for p in "${OWN_PIDS[@]}"; do stop_tree "$p"; done
    for _ in $(seq 1 20); do
        left=""
        for p in "${OWN_PIDS[@]}"; do alive "$p" && left="$left $p"; done
        for p in $(obs_processes); do alive "$p" && left="$left $p"; done
        [ -z "$left" ] && break
        sleep 0.5
    done
    if [ -n "$left" ]; then
        # shellcheck disable=SC2086 # a list of PIDs
        ps -o pid,stat,comm,args -p "$(echo $left | tr ' ' ,)" || true
        fail "processes of the $SCENARIO scenario were still running 10 s after SIGTERM"
    fi
    if grep -F "$INSTANCE_PREFIX" /proc/net/unix 2>/dev/null; then
        fail "an OBS instance socket was still open after the $SCENARIO scenario"
    fi
    if [ "${#FAILED[@]}" -ne 0 ]; then
        echo "OBS AppImage FAILED $SCENARIO on $PRETTY_NAME:"
        printf '  - %s\n' "${FAILED[@]}"
        exit 1
    fi
    echo "OBS AppImage passed $SCENARIO on $PRETTY_NAME${1:+ ($1)}"
    exit 0
}
# Quits OBS like a user closing its window (SIGINT), then requires it to exit
# by itself, with status 0, within 30 s. $1: the AppRun PID, $2: what is being
# quit (for messages), $3: the file a backtrace goes to if it hangs.
quit_obs() {
    local app="$1" what="$2" out="$3" obs rc
    obs="$(obs_pid_of "$app")"
    [ -n "$obs" ] && kill -INT "$obs"
    for _ in $(seq 1 30); do
        alive "$app" || break
        sleep 1
    done
    if alive "$app"; then
        fail "$what did not quit within 30 s of SIGINT (backtrace in its stdout/stderr group)"
        obs="$(obs_pid_of "$app")"
        [ -n "$obs" ] && gdb -q -batch -p "$obs" -ex 'thread apply all bt 25' >>"$out" 2>&1
        stop_tree "$app" -KILL
    fi
    set +e
    wait "$app"
    rc=$?
    set -e
    [ "$rc" -eq 0 ] || fail "$what $(describe_exit "$rc") while quitting"
}
# AppRun hides a crash from its caller: it writes a report and, when gdb is
# installed, starts OBS again under gdb, which may then run fine. So any crash
# report fails the scenario, whatever OBS did afterwards.
require_no_crash_report() {
    local report="${XDG_CACHE_HOME:-$HOME/.cache}/obs-studio-appimage-crash.txt"
    if [ -f "$report" ]; then
        echo "::group::AppRun's crash report"
        cat "$report"
        echo "::endgroup::"
        fail "OBS crashed during the $SCENARIO scenario (AppRun saved a crash report and restarted it)"
    fi
}
# An OBS already running here would make the next one stop at its "already
# running" dialog (it counts the "@/com/obsproject" sockets in /proc/net/unix).
require_no_obs_running() {
    if grep -F "$INSTANCE_PREFIX" /proc/net/unix 2>/dev/null; then
        ps -eo pid,ppid,stat,comm,args 2>&1 | head -40
        fail "an OBS is already running in this container, so the $SCENARIO scenario cannot start one"
        finish
    fi
}
obs_processes() {
    local p
    for p in $(ps -eo pid=); do
        [ "$(readlink "/proc/$p/exe" 2>/dev/null)" = "$APPDIR/usr/bin/$GUI_NAME" ] && echo "$p"
    done
    return 0
}
# The obs process among the descendants of $1 (AppRun runs it as a child).
obs_pid_of() {
    local p
    for p in $(descendants "$1"); do
        [ "$(readlink "/proc/$p/exe" 2>/dev/null)" = "$APPDIR/usr/bin/$GUI_NAME" ] && { echo "$p"; return; }
    done
    return 0
}

echo "::group::Container: $(. /etc/os-release && echo "$PRETTY_NAME"), $(ldd --version 2>&1 | head -1)"
echo "::endgroup::"

echo "::group::Install test infrastructure"
. /etc/os-release
case "${ID_LIKE:-} ${ID}" in
*debian* | *ubuntu*)
    FAMILY=debian
    export DEBIAN_FRONTEND=noninteractive
    # Keep downloaded .debs (the images delete them) so the CI package cache works.
    rm -f /etc/apt/apt.conf.d/docker-clean
    echo 'Binary::apt::APT::Keep-Downloaded-Packages "true"; APT::Keep-Downloaded-Packages "true";' \
        >/etc/apt/apt.conf.d/99keep
    apt-get update -qq
    apt-get install -y -qq --no-install-recommends \
        ca-certificates xvfb icewm x11-utils imagemagick fonts-dejavu-core gdb procps dbus diffutils \
        libgl1-mesa-dri libglx-mesa0 libegl-mesa0 libgl1 libegl1 libgbm1 libdrm2 libva2 libva-drm2 libva-x11-2 libopengl0 \
        libwayland-client0 libwayland-egl1 libvulkan1 >/dev/null
    # HOSTILE_QT=1: a host that has its own, different Qt plugins - the usual desktop - which
    # a bundled Qt must not load (this crashed OBS in QIcon via the host's libqsvgicon.so).
    if [ -n "${HOSTILE_QT:-}" ]; then
        # (the plugins live in libqt6svg6 on older releases, in qt6-svg-plugins on newer ones)
        apt-get install -y -qq --no-install-recommends libqt6svg6 qt6-svg-plugins >/dev/null ||
            apt-get install -y -qq --no-install-recommends libqt6svg6 >/dev/null
    fi
    ;;
*fedora* | *rhel*)
    FAMILY=fedora
    dnf install -y -q --setopt=install_weak_deps=False --setopt=keepcache=1 \
        xorg-x11-server-Xvfb icewm xwininfo xprop ImageMagick dbus-daemon diffutils \
        dejavu-sans-fonts gdb procps-ng mesa-dri-drivers mesa-libGL mesa-libEGL mesa-libgbm libdrm libva \
        libglvnd-glx libglvnd-egl libglvnd-opengl libwayland-client libwayland-egl vulkan-loader >/dev/null
    ;;
*arch*)
    FAMILY=arch
    # A rolling distribution: its glibc is newer than the one the AppImage is
    # built with, and its Mesa (libgallium -> libLLVM) needs that newer glibc
    # and libstdc++ (obs-rust/obs-studio#621).
    pacman -Syu --noconfirm --needed \
        xorg-server-xvfb icewm xorg-xwininfo xorg-xprop imagemagick ttf-dejavu gdb procps-ng dbus diffutils \
        mesa libglvnd libdrm libva wayland vulkan-icd-loader >/dev/null
    ;;
*)
    echo "::error::unsupported test distribution: $PRETTY_NAME"
    exit 1
    ;;
esac
echo "::endgroup::"

# Make sure the container really does lack what the AppImage must bring along.
# (With HOSTILE_QT the host deliberately has its own Qt, to prove the bundle ignores it.)
# (PipeWire and PulseAudio client libraries may come with the infrastructure;
# part 4 checks that OBS maps the AppImage's own copies.)
absent="libavcodec.so libobs.so.30 libx264.so"
[ -z "${HOSTILE_QT:-}" ] && absent="libQt6Core.so.6 $absent"
for lib in $absent; do
    if ldconfig -p | grep -q "$lib"; then
        echo "::error::test container unexpectedly provides $lib"
        exit 1
    fi
done

chmod +x "$APPIMAGE"

echo "::group::AppImage runs through its own runtime (no FUSE in containers)"
VERSION_OUT="$(APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGE" --version 2>&1)" || {
    echo "$VERSION_OUT"
    echo "::error::obs --version failed"
    exit 1
}
echo "$VERSION_OUT"
echo "::endgroup::"
# A build with the Rust libobs ports names itself OBS-Studio-Rust (obs-rust/obs-studio#112).
if [ -n "${EXPECT_RUST:-}" ] && [[ "$VERSION_OUT" != OBS-Studio-Rust* ]]; then
    echo "::error::this AppImage was built with ENABLE_RUST_LIBOBS but does not report OBS-Studio-Rust"
    exit 1
fi
# The daily beta carries its full release string, so betas can be told apart (#631).
if [ -n "${EXPECT_RUST_VERSION:-}" ] && [[ "$VERSION_OUT" != "OBS-Studio-Rust $EXPECT_RUST_VERSION (based on OBS "* ]]; then
    echo "::error::expected 'OBS-Studio-Rust $EXPECT_RUST_VERSION (based on OBS ...)' from --version, got: $VERSION_OUT"
    exit 1
fi

# Release candidates must retain updater information in the actual runtime.
# The workflow's asset_metadata.py check verifies its zsync against this file.
if [ -n "${EXPECT_APPIMAGE_UPDATE:-}" ]; then
    UPDATE_INFO="$("$APPIMAGE" --appimage-updateinformation)"
    [[ "$UPDATE_INFO" == gh-releases-zsync\|*\|*\|continuous\|OBS-Studio-Rust-\*-linux-x86_64.AppImage.zsync ]] ||
        { echo "::error::missing or stale AppImage update target: $UPDATE_INFO"; exit 1; }
fi

"$APPIMAGE" --appimage-extract >/dev/null
APPDIR="$PWD/squashfs-root"
LOADER="$APPDIR/lib64/ld-linux-x86-64.so.2"
PACKAGE_DATA="$APPDIR/usr/share/obs"
PACKAGE_MODULES="$APPDIR/usr/lib/obs-modules"
if [[ "$VERSION_OUT" == OBS-Studio-Rust* ]]; then
    GUI_NAME=obs-studio-rust-gui
    CONFIG_NAME=obs-studio-rust
    INSTANCE_PREFIX=@/io/github/obs_rust/OBSStudioRust
    PACKAGE_DATA="$APPDIR/usr/share/obs-studio-rust"
    PACKAGE_MODULES="$APPDIR/usr/lib/obs-studio-rust/obs-modules"
    APP_ID=io.github.obs_rust.OBSStudioRust
    [ -f "$APPDIR/$APP_ID.desktop" ] || fail "Rust desktop identity is missing"
    [ -f "$APPDIR/usr/bin/$GUI_NAME" ] || fail "Rust GUI executable is missing"
    grep -Fxq "Exec=$GUI_NAME" "$APPDIR/$APP_ID.desktop" || fail "Rust desktop starts the wrong executable"
    grep -Fxq "Icon=$APP_ID" "$APPDIR/$APP_ID.desktop" || fail "Rust desktop shares its icon identity"
    grep -Fxq "StartupWMClass=$GUI_NAME" "$APPDIR/$APP_ID.desktop" || fail "Rust desktop shares its window class"
    [ -f "$APPDIR/usr/share/metainfo/$APP_ID.metainfo.xml" ] || fail "Rust metainfo identity is missing"
    [ "${#FAILED[@]}" -eq 0 ] || finish
fi
# The C and C++ runtime is bundled in glibc/, outside usr/lib, so that AppRun can
# use the host's newer one instead (obs-rust/obs-studio#621). Probes that run a
# bundled ELF file with the bundled loader directly are given it explicitly.
LOADER_ARGS=()
[ -d "$APPDIR/glibc" ] && LOADER_ARGS=(--library-path "$APPDIR/glibc")
# Which C library AppRun picked for this run, written by AppRun (bundled|host);
# empty before AppRun has run, or for an AppImage that always bundles it.
DRIVER_STACK_RE='libGL\.|libGLX|libEGL|libOpenGL|libGLdispatch|libGLESv|libglapi|libgbm|libdrm|libvulkan|libva(\.so|-)|libwayland-(client|egl|server)|libgallium|libLLVM|libcuda|libnvidia'
runtime_mode() { cat /tmp/.obs-rust-appimage-*/mode 2>/dev/null | head -1; }
# What this host should get: its own glibc and libstdc++ when they are at least
# as new as the bundled ones (so it can load the host's Mesa/LLVM), else the bundle.
host_glibc_is_newer() {
    local host bundled
    host="$(ldd --version | sed -n '1s/.* \([0-9][0-9]*\.[0-9][0-9]*\)$/\1/p')"
    bundled="$("$LOADER" --version | sed -n '1s/.*release version \([0-9][0-9]*\.[0-9][0-9]*\).*/\1/p')"
    [ "$(printf '%s\n%s\n' "$bundled" "$host" | sort -V | tail -1)" = "$host" ] && [ "$host" != "$bundled" ]
}
# The helper is built with RPATH $ORIGIN/../lib, so placed next to obs it finds
# the bundled libraries exactly the way obs does (no --library-path).
install -m755 "$TESTKIT" "$APPDIR/usr/bin/obs-appimage-testkit"

# The modules the build installed (assemble-appdir.sh records them).
INVENTORY="$PACKAGE_DATA/appimage-modules.txt"
if [ ! -f "$INVENTORY" ]; then
    echo "::warning::no module inventory (built before obs-rust/obs-studio#426); using the modules present"
    find "$PACKAGE_MODULES" -name '*.so' -printf '%f\n' | sort >"$PWD/modules.txt"
    INVENTORY="$PWD/modules.txt"
fi

# ============================================================================
# No desktop-session package is installed in this scenario, so nothing in the
# container can stand in for a library the AppImage lacks.
if [ "$SCENARIO" = bundle ]; then
    echo "::group::1. Bundle: runtime paths, libraries and symbols"
    "$HERE_DIR/check-appdir.sh" "$APPDIR" || fail "the AppImage lacks files that AppRun points OBS at (see above)"
    # The bundled loader resolves every bundled ELF file the way it will at runtime
    # (each file's own RPATH into the AppDir), binding every symbol now.
    # The GPU driver stack must come from the host (the AppImage excludelist): the
    # host's Mesa, NVIDIA and VA-API drivers are built against the host's copies and
    # look for their backends in the host's directories, and a bundled copy wins
    # over them through the relative RPATH (obs-rust/obs-studio#625).
    stack="$(find "$APPDIR" -path '*/qt6/plugins' -prune -o \( -type f -o -type l \) \
        \( -name 'libGL.so*' -o -name 'libGLX*' -o -name 'libEGL*' -o -name 'libOpenGL*' -o -name 'libGLdispatch*' \
        -o -name 'libGLESv*' -o -name 'libglapi*' -o -name 'libgbm*' -o -name 'libdrm*' \
        -o -name 'libva.so*' -o -name 'libva-*.so*' -o -name 'libvulkan*' -o -name 'libwayland-client*' -o -name 'libwayland-egl*' -o -name 'libwayland-server*' \
        -o -name 'libgallium*' -o -name 'libLLVM*' -o -name 'libcuda*' -o -name 'libnvidia*' \) \
        -printf '%P\n' | sort)"
    if [ -n "$stack" ]; then
        echo "$stack"
        fail "the AppImage bundles the GPU driver stack above; it must come from the host"
    fi
    unresolved=0
    checked=0
    while IFS= read -r -d '' f; do
        [ "$(head -c4 "$f" | od -An -c | tr -d ' ')" = '177ELF' ] || continue
        case "$(basename "$f")" in ld-linux*) continue ;; esac
        checked=$((checked + 1))
        if ! out="$(LD_TRACE_LOADED_OBJECTS=1 LD_WARN=yes LD_BIND_NOW=yes "$LOADER" "${LOADER_ARGS[@]}" "$f" 2>&1)" ||
            grep -qE 'not found|undefined symbol|error while loading' <<<"$out"; then
            echo "${f#"$APPDIR"/}:"
            grep -E 'not found|undefined symbol|error while loading' <<<"$out" | sed "s|$APPDIR/||g" | head -5
            unresolved=$((unresolved + 1))
        elif grep '=> /' <<<"$out" | grep -v "$APPDIR" | grep -v "=> /tmp/.obs-rust-appimage-[0-9a-f]*/lib/" | grep -vE "=> [^ ]*/($DRIVER_STACK_RE)" | grep .; then
            # (the host's GPU driver stack is the only thing allowed outside the bundle)
            echo "  ^ ${f#"$APPDIR"/} resolves the libraries above from the host"
            unresolved=$((unresolved + 1))
        fi
    done < <(find "$APPDIR/usr" -type f \( -name '*.so' -o -name '*.so.*' -o -perm -u+x \) -print0)
    echo "$checked ELF files checked, $unresolved resolve libraries or symbols wrongly"
    [ "$unresolved" -eq 0 ] || fail "$unresolved bundled ELF files do not load from the bundle (see above)"
    # The host's GPU drivers are loaded into OBS with the bundled loader and libc,
    # and they link more of glibc than OBS does (the NVIDIA driver's libcuda and
    # libEGL_nvidia need librt, libpthread and libdl). Such a library resolves
    # through obs's RPATH, so it must be in the bundle: the host's copy belongs to
    # the host's glibc and needs private symbols of that version, which the
    # bundled loader lacks wherever the host's glibc is newer (libcuda failed to
    # load, so NVENC was missing). LD_PRELOAD by name resolves each one the way a
    # host driver's DT_NEEDED entry does.
    if [ -e "$APPDIR/usr/lib/libc.so.6" ] || [ -e "$APPDIR/glibc/libc.so.6" ]; then
        for lib in libm.so.6 libmvec.so.1 librt.so.1 libpthread.so.0 libdl.so.2 libutil.so.1 \
            libanl.so.1 libresolv.so.2 libnss_files.so.2 libnss_dns.so.2; do
            out="$(LD_TRACE_LOADED_OBJECTS=1 LD_WARN=yes LD_BIND_NOW=yes LD_PRELOAD="$lib" \
                "$LOADER" "${LOADER_ARGS[@]}" "$APPDIR/usr/bin/$GUI_NAME" 2>&1 || true)"
            where="$(awk -v l="$lib" '$1 == l && $2 == "=>" {print $3}' <<<"$out")"
            [ "$where" = not ] && where="nothing (not found)"
            if [[ "$where" != "$APPDIR"/* && "$where" != /tmp/.obs-rust-appimage-*/lib/* ]]; then
                fail "glibc's $lib is not bundled with libc.so.6: a host driver that links it gets ${where:-nothing}"
            elif grep -E 'undefined symbol|error while loading' <<<"$out"; then
                fail "the bundled $lib does not load with the bundled loader (see above)"
            fi
        done
    fi
    # The modules the build installed are all there.
    while read -r m; do
        [ -n "$(find "$PACKAGE_MODULES" -name "$m" -print -quit)" ] || fail "module $m from the build is not in the AppImage"
    done <"$INVENTORY"
    echo "$(wc -l <"$INVENTORY") modules in the build inventory"
    echo "::endgroup::"
    finish
fi

# Use the driver's actual production setup, including user-selected overrides.
APPRUN_DRIVER_ENV="$(sed -n '/^# Host driver search paths/,/^# gconv modules/p' "$APPDIR/AppRun" | sed '$d')"

# This scenario requires a real render node supplied by the caller. Stock CI
# runners without GPUs do not count as physical validation.
if [ "$SCENARIO" = gpu ]; then
    GPU_TEST="$(realpath "${GPU_TEST:?gpu requires the compiled gpu-test helper}")"
    GPU_INPUT="$(realpath "${GPU_INPUT:?gpu requires a 12-frame 128x72 H264 fixture}")"
    GPU_REFERENCE="$(realpath "${GPU_REFERENCE:?gpu requires its software-decoded YUV420P reference}")"
    GPU_RENDER_NODE="${GPU_RENDER_NODE:?gpu requires a physical render node}"
    [ -c "$GPU_RENDER_NODE" ] || { echo "::error::no physical render node: $GPU_RENDER_NODE"; exit 1; }
    [ -n "$APPRUN_DRIVER_ENV" ] || { echo "::error::package lacks host driver setup"; exit 1; }
    install -m755 "$GPU_TEST" "$APPDIR/usr/bin/obs-appimage-gpu-test"
    GPU_LOADER="$(sed -n 's/^LOADER="\([^"]*\)"$/\1/p' "$APPDIR/AppRun")"
    GPU_LOADER_DIR="$(dirname "$GPU_LOADER")"
    [ -x "$GPU_LOADER" ] && [ -r "$GPU_LOADER_DIR/mode" ] ||
        { echo "::error::AppRun did not initialize its selected interpreter"; exit 1; }
    eval "$APPRUN_DRIVER_ENV"
    echo "GPU runtime mode: $(cat "$GPU_LOADER_DIR/mode")"
    # Select the same C runtime as AppRun. In host mode this private lib
    # directory is empty; in bundled mode it holds the chosen C runtime.
    for mode in gbm vaapi; do
        args=("$GPU_RENDER_NODE")
        [ "$mode" != vaapi ] || args+=("$GPU_INPUT" "$OUT/vaapi.yuv")
        if ! timeout --kill-after=5s 120s "$GPU_LOADER" --library-path "$GPU_LOADER_DIR/lib:$APPDIR/usr/lib" \
            "$APPDIR/usr/bin/obs-appimage-gpu-test" "$mode" "${args[@]}" >"$OUT/gpu-$mode.log" 2>&1; then
            fail "physical $mode frames failed"
        fi
        cat "$OUT/gpu-$mode.log"
        if grep -E "/($DRIVER_STACK_RE)" "$OUT/gpu-$mode.log" | grep -F "$APPDIR/"; then
            fail "$mode loaded a driver library from the AppImage"
        fi
    done
    if ! cmp "$GPU_REFERENCE" "$OUT/vaapi.yuv"; then
        fail "hardware-decoded VAAPI pixels differ from the software oracle"
    fi
    grep -F "$APPDIR/usr/lib/libavcodec.so" "$OUT/gpu-vaapi.log" >/dev/null || fail "VAAPI did not use packaged libavcodec"
    grep -F "$APPDIR/usr/lib/libavutil.so" "$OUT/gpu-vaapi.log" >/dev/null || fail "VAAPI did not use packaged libavutil"
    [ ! -f "$OUT/vaapi.yuv" ] || sha256sum "$OUT/vaapi.yuv"
    finish
fi

# ============================================================================
if [ "$SCENARIO" != probes ]; then
    # A display number nobody uses, so a shared machine's X server is never touched.
    DISPLAY_NUM="${TEST_DISPLAY:-99}"
    while [ -e "/tmp/.X11-unix/X$DISPLAY_NUM" ] || [ -e "/tmp/.X$DISPLAY_NUM-lock" ]; do
        DISPLAY_NUM=$((DISPLAY_NUM + 1))
    done
    Xvfb ":$DISPLAY_NUM" -screen 0 1280x800x24 -nolisten tcp &
    OWN_PIDS+=($!)
    for _ in $(seq 1 50); do [ -e "/tmp/.X11-unix/X$DISPLAY_NUM" ] && break; sleep 0.1; done
    export DISPLAY=":$DISPLAY_NUM"
    # Some toolkits only paint after a window manager completes the map handshake.
    icewm >/dev/null 2>&1 &
    OWN_PIDS+=($!)
    sleep 2
fi

export QT_QPA_PLATFORM=xcb
export QT_ACCESSIBILITY=0
# Behave like a real desktop session: a session bus, and an LD_LIBRARY_PATH full of host
# libraries (GNUstep/Nix/vendor-driver sessions export one). The AppImage must not let
# either one change which libraries it uses or crash the programs it spawns.
HOSTILE_LD=/usr/lib:/usr/lib64:/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu
export LIBGL_ALWAYS_SOFTWARE=1 # no GPU in CI; Mesa llvmpipe from the host stands in for the driver

# Libraries $2.. must be mapped into process $1 from inside the AppImage.
require_mapped_from_appimage() {
    local pid="$1" lib
    shift
    for lib in "$@"; do
        if ! grep -o '/[^ ]*' "/proc/$pid/maps" | grep "/$lib" | grep -q "$APPDIR"; then
            fail "$lib was not loaded from inside the AppImage"
        fi
    done
}

# ============================================================================
if [ "$SCENARIO" = first-start ]; then
    require_no_obs_running
    echo "::group::2. First start with a new profile"
    export HOME="$PWD/home-new"
    mkdir -p "$HOME/.config/$CONFIG_NAME" # OBS only creates one directory level at a time
    LD_LIBRARY_PATH="$HOSTILE_LD" dbus-run-session -- "$APPDIR/AppRun" --disable-shutdown-check --verbose >"$OUT/obs-stdout.log" 2>&1 &
    APP_PID=$!
    OWN_PIDS+=("$APP_PID")

    WINDOW_FOUND=0
    RENDERED=0
    for _ in $(seq 1 15); do
        sleep 2
        alive "$APP_PID" || break
        # A real window owned by OBS (not just the window manager's own chrome).
        # (capture first: `grep -q` closing the pipe early fails the pipeline under pipefail)
        WINDOWS="$(xwininfo -tree -root 2>/dev/null || true)"
        if grep -qE '"OBS-Studio-Rust|\"OBS [0-9]|"obs"' <<<"$WINDOWS"; then
            # Qt derives WM_CLASS from the real GUI executable name. Verify
            # the owned native window rather than accepting a matching title.
            CLASS_FOUND=0
            while read -r window_id; do
                PROPERTIES="$(xprop -id "$window_id" _NET_WM_PID WM_CLASS 2>/dev/null || true)"
                WINDOW_PID="$(sed -n 's/^_NET_WM_PID(CARDINAL) = //p' <<<"$PROPERTIES")"
                [[ "$WINDOW_PID" =~ ^[0-9]+$ ]] || continue
                [ "$(readlink "/proc/$WINDOW_PID/exe" 2>/dev/null || true)" = "$APPDIR/usr/bin/$GUI_NAME" ] || continue
                if grep -Fq "WM_CLASS(STRING) = \"$GUI_NAME\", " <<<"$PROPERTIES"; then
                    CLASS_FOUND=1
                    break
                fi
            done < <(awk '$1 ~ /^0x[0-9a-fA-F]+$/ {print $1}' <<<"$WINDOWS")
            [ "$CLASS_FOUND" -eq 1 ] || continue
            WINDOW_FOUND=1
            import -window root "$OUT/screenshot.png"
            STDDEV="$(identify -format '%[fx:standard_deviation]' "$OUT/screenshot.png")"
            echo "OBS window found, frame stddev: $STDDEV"
            if awk -v v="$STDDEV" 'BEGIN{exit !(v > 0.02)}'; then
                RENDERED=1
                break
            fi
        fi
    done

    xwininfo -tree -root 2>&1 | head -40 || true
    OBS_PID="$(obs_pid_of "$APP_PID")"
    if [ -n "$OBS_PID" ]; then
        # Libraries actually mapped into the running process: the core stack must be bundled.
        echo "Libraries mapped from outside the AppImage:"
        grep -o '/[^ ]*\.so[^ ]*' "/proc/$OBS_PID/maps" | sort -u | grep -v "$APPDIR" | grep -v '/tmp/.obs-rust-appimage-' || true
        # No Qt plugin may come from the host, and the SVG ones must come from the bundle.
        if grep -o '/[^ ]*qt6/plugins/[^ ]*' "/proc/$OBS_PID/maps" | grep -v "$APPDIR" | grep .; then
            fail "Qt plugins above were loaded from outside the AppImage"
        fi
        # (Qt loads the SVG plugins when the first icon is drawn, which can be after the first frame)
        for _ in $(seq 1 10); do
            grep -o '/[^ ]*' "/proc/$OBS_PID/maps" | grep '/libqsvg' | grep -q "$APPDIR" && break
            sleep 1
        done
        require_mapped_from_appimage "$OBS_PID" libqsvgicon.so libqsvg.so \
            libQt6Core.so libQt6Widgets.so libavcodec.so libobs.so libX11.so libxcb.so
        # The C and C++ runtime: the bundled one on a host with an older glibc, but the
        # host's own on a host with a newer one - the host's Mesa/LLVM cannot load
        # against an older glibc (obs-rust/obs-studio#621).
        MODE="$(runtime_mode)"
        echo "C library mode: ${MODE:-bundled (AppRun without modes)} (host: $(ldd --version | head -1))"
        if host_glibc_is_newer && [ -d "$APPDIR/glibc" ]; then
            [ "$MODE" = host ] || fail "this host's glibc is newer than the bundled one but OBS runs on the bundled C library (mode: ${MODE:-none})"
        fi
        if [ "$MODE" = host ]; then
            for lib in libc.so.6 libstdc++.so.6 libgcc_s.so.1; do
                if grep -o '/[^ ]*' "/proc/$OBS_PID/maps" | grep "/$lib" | grep -q "$APPDIR\|/tmp/.obs-rust-appimage-"; then
                    fail "$lib comes from the AppImage although the host's is newer"
                fi
            done
        else
            for lib in libc.so.6 libstdc++.so.6; do
                grep -o '/[^ ]*' "/proc/$OBS_PID/maps" | grep "/$lib" | grep -q "$APPDIR\|/tmp/.obs-rust-appimage-" ||
                    fail "$lib was not loaded from inside the AppImage"
            done
        fi
        # The GPU driver stack comes from the host, never from the AppImage.
        if grep -o '/[^ ]*' "/proc/$OBS_PID/maps" | grep -E "/($DRIVER_STACK_RE)" | grep "$APPDIR" | grep .; then
            fail "GPU driver stack libraries above were loaded from inside the AppImage"
        fi
    else
        fail "OBS exited during its first start"
    fi
    if alive "$APP_PID"; then
        quit_obs "$APP_PID" "OBS on its first start" "$OUT/obs-stdout.log"
    fi
    LOG="$(find "$HOME/.config/$CONFIG_NAME/logs" -type f 2>/dev/null | sort | tail -1)"
    [ -n "$LOG" ] && cp "$LOG" "$OUT/obs.log"
    echo "::endgroup::"
    if [ -n "$LOG" ]; then
        echo "::group::OBS log (first start)"
        cat "$LOG"
        echo "::endgroup::"
    fi
    echo "::group::OBS stdout/stderr (first start)"
    cat "$OUT/obs-stdout.log"
    echo "::endgroup::"
    [ "$WINDOW_FOUND" -eq 1 ] || fail "no OBS window ever appeared"
    [ "$RENDERED" -eq 1 ] || fail "OBS window exists but the screen stayed blank"
    if [ -n "$LOG" ] && grep -qiE 'Failed to initialize video|Failed to load.*libobs-opengl|eglGetDisplay failed|Failed to create (OpenGL|EGL)' "$LOG" "$OUT/obs-stdout.log"; then
        grep -iE 'Failed to|eglGetDisplay' "$LOG" "$OUT/obs-stdout.log"
        fail "OBS reported a fatal initialisation failure"
    fi
    require_no_crash_report
    finish
fi

# ============================================================================
# The stand-in desktop session: installed only now (part 1 must not see it), and
# started privately: its own runtime directory, session bus, PipeWire,
# WirePlumber and pipewire-pulse.
echo "::group::Install and start a private desktop session (PipeWire, WirePlumber, pipewire-pulse, GStreamer)"
case "$FAMILY" in
debian)
    apt-get install -y -qq --no-install-recommends \
        pipewire wireplumber pipewire-pulse pulseaudio-utils \
        gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-pipewire >/dev/null
    ;;
fedora)
    dnf install -y -q --setopt=install_weak_deps=False --setopt=keepcache=1 \
        pipewire wireplumber pipewire-pulseaudio pulseaudio-utils pipewire-utils \
        gstreamer1 gstreamer1-plugins-base pipewire-gstreamer >/dev/null
    ;;
*arch*)
    pacman -S --noconfirm --needed pipewire wireplumber pipewire-pulse libpulse \
        gstreamer gst-plugins-base gst-plugin-pipewire >/dev/null
    ;;
esac
pipewire --version | tail -1
# The test producers need PipeWire 1.0 or newer: on 0.3.x (Ubuntu 22.04) a
# GStreamer "provide" stream never delivers a frame, to OBS or to anyone else.
PW_VERSION="$(pipewire --version | sed -n 's/.*libpipewire \([0-9]*\)\..*/\1/p' | tail -1)"
export HOME="$PWD/home-profile"
mkdir -p "$HOME"
export XDG_RUNTIME_DIR="$PWD/run"
mkdir -m 700 "$XDG_RUNTIME_DIR"
export XDG_CONFIG_HOME="$HOME/.config" XDG_STATE_HOME="$HOME/.local/state" XDG_DATA_HOME="$HOME/.local/share"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$XDG_DATA_HOME"
DBUS_SESSION_BUS_ADDRESS="$(dbus-daemon --session --fork --print-address=1 --print-pid=3 3>"$PWD/dbus.pid")"
export DBUS_SESSION_BUS_ADDRESS
OWN_PIDS+=("$(cat "$PWD/dbus.pid")")
pipewire >"$OUT/pipewire.log" 2>&1 &
OWN_PIDS+=($!)
for _ in $(seq 1 50); do [ -S "$XDG_RUNTIME_DIR/pipewire-0" ] && break; sleep 0.1; done
wireplumber >"$OUT/wireplumber.log" 2>&1 &
OWN_PIDS+=($!)
pipewire-pulse >"$OUT/pipewire-pulse.log" 2>&1 &
OWN_PIDS+=($!)
for _ in $(seq 1 50); do [ -S "$XDG_RUNTIME_DIR/pulse/native" ] && break; sleep 0.1; done
[ -S "$XDG_RUNTIME_DIR/pipewire-0" ] || fail "the test session's PipeWire daemon did not start"
[ -S "$XDG_RUNTIME_DIR/pulse/native" ] || fail "the test session's pipewire-pulse did not start"
pactl info | grep -E 'Server Name|Server Version' || true
echo "::endgroup::"

# The helper runs in exactly the environment AppRun gives OBS, with the bundled
# loader and (through its RPATH) the bundled libraries.
APPRUN_ENV="$(grep -E '^export [A-Z_]+="\$HERE' "$APPDIR/AppRun")"
in_appimage() {
    (
        # shellcheck disable=SC2034 # used by $APPRUN_ENV
        HERE="$APPDIR"
        eval "$APPRUN_ENV"
        eval "$APPRUN_DRIVER_ENV"
        exec "$LOADER" "${LOADER_ARGS[@]}" "$APPDIR/usr/bin/obs-appimage-testkit" "$@"
    )
}

# ============================================================================
if [ "$SCENARIO" = probes ]; then
    echo "::group::3. Runtime probes with the AppImage's own libraries"
    for probe in probe-pipewire probe-pulse probe-gconv probe-dbus; do
        set +e
        in_appimage "$probe" 2>&1
        rc=$?
        set -e
        [ "$rc" -eq 0 ] || fail "$probe failed ($(describe_exit "$rc"))"
    done
    echo "::endgroup::"
    finish
fi

# ============================================================================
# Only saved-profile is left. The test producers need PipeWire 1.0. Only Ubuntu
# 22.04 (0.3.x) may run without them; anything else, or a version that cannot
# be read, is a failure, never a silent skip.
LAUNCH_ONLY=""
if [ -z "$PW_VERSION" ]; then
    fail "could not read the test session's PipeWire version ($(pipewire --version 2>&1 | tail -1))"
    finish
elif [ "$PW_VERSION" -lt 1 ]; then
    if [ "$ID" = ubuntu ] && [ "$VERSION_ID" = 22.04 ]; then
        LAUNCH_ONLY=1
        echo "::notice::$PRETTY_NAME's PipeWire $PW_VERSION.x cannot run the test producers: OBS starts the saved profile without them and there is no recording"
        echo "PipeWire older than 1.0 cannot run the test producers; OBS started the saved profile without them" >"$OUT/recording-skipped.txt"
    else
        fail "the test session's PipeWire $PW_VERSION.x cannot run the test producers on $PRETTY_NAME (only Ubuntu 22.04 may run without them)"
        finish
    fi
fi

echo "::group::4. Saved profile: PipeWire screen capture, PipeWire camera, PulseAudio microphone"
SCREEN_NODE=""
if [ -z "$LAUNCH_ONLY" ]; then
    # Test producers stand in for a monitor, a camera and a microphone: scrolling
    # colour bars, a bouncing ball and a 440 Hz tone, which verify-recording.sh
    # looks for.
    gst-launch-1.0 -q videotestsrc pattern=smpte horizontal-speed=4 is-live=true \
        ! video/x-raw,format=BGRx,width=320,height=240,framerate=30/1 \
        ! pipewiresink mode=provide client-name=obs-test-screen \
        stream-properties="props,media.class=Video/Source,node.name=obs-test-screen,node.description=obs-test-screen" \
        >"$OUT/producer-screen.log" 2>&1 &
    OWN_PIDS+=($!)
    gst-launch-1.0 -q videotestsrc pattern=ball is-live=true background-color=0xff204080 \
        ! video/x-raw,format=YUY2,width=160,height=120,framerate=30/1 \
        ! pipewiresink mode=provide client-name=obs-test-camera \
        stream-properties="props,media.class=Video/Source,media.role=Camera,node.name=obs-test-camera,node.description=obs-test-camera" \
        >"$OUT/producer-camera.log" 2>&1 &
    OWN_PIDS+=($!)
    # The microphone is a native PipeWire stream playing a file (a GStreamer audio
    # source in "provide" mode never gets a graph driver without a sound card).
    # PIPEWIRE_PROPS works on every PipeWire version (pw-play -P needs 0.3.50+).
    in_appimage write-tone "$PWD/tone.wav" 60
    PIPEWIRE_PROPS='{ media.class = "Audio/Source" node.name = "obs-test-mic" node.description = "obs-test-mic" }' \
        pw-play "$PWD/tone.wav" >"$OUT/producer-mic.log" 2>&1 &
    OWN_PIDS+=($!)

    node_id() {
        pw-cli ls Node 2>/dev/null | awk -v want="\"$1\"" '
            /^[[:space:]]*id [0-9]+,/ { id = $2; sub(",", "", id) }
            $1 == "node.name" && $3 == want { print id; exit }'
    }
    for _ in $(seq 1 50); do
        SCREEN_NODE="$(node_id obs-test-screen)"
        [ -n "$SCREEN_NODE" ] && [ -n "$(node_id obs-test-camera)" ] && [ -n "$(node_id obs-test-mic)" ] && break
        sleep 0.2
    done
    pw-cli ls Node | grep -E 'id [0-9]+,|node.name' || true
    for n in obs-test-screen obs-test-camera obs-test-mic; do
        [ -n "$(node_id "$n")" ] || fail "the test producer $n did not appear in PipeWire (see $OUT/producer-*.log)"
    done
fi

OBS_TEST_SCREEN_NODE="$SCREEN_NODE" OBS_TEST_SCREEN_SIZE=320x240 \
    in_appimage fake-portal >"$OUT/fake-portal.log" 2>&1 &
OWN_PIDS+=($!)
for _ in $(seq 1 50); do grep -q serving "$OUT/fake-portal.log" && break; sleep 0.1; done
cat "$OUT/fake-portal.log"

# A saved profile and scene collection, as a returning user has them.
REC_DIR="$PWD/recordings"
mkdir -p "$REC_DIR" "$XDG_CONFIG_HOME/obs-studio/basic/profiles/Test" "$XDG_CONFIG_HOME/obs-studio/basic/scenes"
cat >"$XDG_CONFIG_HOME/obs-studio/user.ini" <<EOF
[General]
FirstRun=true
ConfirmOnExit=false

[Basic]
Profile=Test
ProfileDir=Test
SceneCollection=Test
SceneCollectionFile=Test

[BasicWindow]
WarnBeforeStoppingRecord=false
EOF
cat >"$XDG_CONFIG_HOME/obs-studio/basic/profiles/Test/basic.ini" <<EOF
[General]
Name=Test

[Output]
Mode=Simple

[SimpleOutput]
FilePath=$REC_DIR
RecFormat2=mkv
RecQuality=Small
RecEncoder=x264
FileNameWithoutSpace=true

[Video]
BaseCX=320
BaseCY=240
OutputCX=320
OutputCY=240
FPSType=0
FPSCommon=30

[Audio]
SampleRate=48000
ChannelSetup=Stereo
EOF
cat >"$XDG_CONFIG_HOME/obs-studio/basic/scenes/Test.json" <<'EOF'
{
    "name": "Test",
    "current_scene": "Scene",
    "current_program_scene": "Scene",
    "scene_order": [{"name": "Scene"}],
    "sources": [
        {
            "id": "scene", "versioned_id": "scene", "name": "Scene",
            "settings": {"id_counter": 3, "items": [
                {"name": "Screen", "id": 1, "visible": true, "pos": {"x": 0, "y": 0},
                 "scale": {"x": 1, "y": 1}, "bounds_type": 2, "bounds": {"x": 320, "y": 240},
                 "bounds_align": 0, "align": 5},
                {"name": "Camera", "id": 2, "visible": true, "pos": {"x": 240, "y": 180},
                 "scale": {"x": 1, "y": 1}, "bounds_type": 2, "bounds": {"x": 80, "y": 60},
                 "bounds_align": 0, "align": 5},
                {"name": "Mic", "id": 3, "visible": true, "pos": {"x": 0, "y": 0},
                 "scale": {"x": 1, "y": 1}, "align": 5}
            ]}
        },
        {
            "id": "pipewire-screen-capture-source", "versioned_id": "pipewire-screen-capture-source",
            "name": "Screen", "settings": {"ShowCursor": false}
        },
        {
            "id": "pipewire-camera-source", "versioned_id": "pipewire-camera-source",
            "name": "Camera", "settings": {"device_id": "obs-test-camera",
                "format": "{\"encoded\": false, \"video_format\": 4, \"width\": 160, \"height\": 120}",
                "framerate": "{\"framerate\": {\"numerator\": 30, \"denominator\": 1}}"}
        },
        {
            "id": "pulse_input_capture", "versioned_id": "pulse_input_capture",
            "name": "Mic", "settings": {"device_id": "obs-test-mic"}
        }
    ]
}
EOF

# Rust starts from an owned upstream fixture, exercising migration before
# recording. The other two saved-profile scenarios cover independently selected
# Locations roots and an existing Rust config that must win over upstream.
LEGACY_CONFIG="$XDG_CONFIG_HOME/obs-studio"
CUSTOM_SOURCE_ROOTS=()
if [ "$CONFIG_NAME" = obs-studio-rust ]; then
    cat >"$LEGACY_CONFIG/global.ini" <<EOF
[General]
LastVersion=4294967295
EOF
    mkdir -p "$LEGACY_CONFIG/plugin_manager"
    printf 'preserved plugin settings\n' >"$LEGACY_CONFIG/plugin_manager/migration-proof.txt"
    if [ "$SCENARIO" = saved-profile-custom ]; then
        for area in configuration profiles scenes plugins; do
            mkdir -p "$PWD/locations/$area/obs-studio"
            CUSTOM_SOURCE_ROOTS+=("$PWD/locations/$area/obs-studio")
        done
        mkdir -p "$PWD/locations/profiles/obs-studio/basic" "$PWD/locations/scenes/obs-studio/basic"
        mv "$LEGACY_CONFIG/user.ini" "$PWD/locations/configuration/obs-studio/user.ini"
        mv "$LEGACY_CONFIG/basic/profiles" "$PWD/locations/profiles/obs-studio/basic/profiles"
        mv "$LEGACY_CONFIG/basic/scenes" "$PWD/locations/scenes/obs-studio/basic/scenes"
        mv "$LEGACY_CONFIG/plugin_manager" "$PWD/locations/plugins/obs-studio/plugin_manager"
        cat >>"$LEGACY_CONFIG/global.ini" <<EOF
[Locations]
Configuration=$PWD/locations/configuration
Profiles=$PWD/locations/profiles
SceneCollections=$PWD/locations/scenes
PluginManagerSettings=$PWD/locations/plugins
EOF
    elif [ "$SCENARIO" = saved-profile-existing ]; then
        cp -a "$LEGACY_CONFIG" "$XDG_CONFIG_HOME/obs-studio-rust"
        printf '{"name":"Upstream empty","sources":[]}\n' >"$LEGACY_CONFIG/basic/scenes/Test.json"
    fi
fi
source_snapshot() {
    local root file
    for root in "$LEGACY_CONFIG" "${CUSTOM_SOURCE_ROOTS[@]}"; do
        find "$root" -printf '%y %p\n' | sort
        while IFS= read -r -d '' file; do sha256sum "$file"; done < <(find "$root" -type f -print0 | sort -z)
    done
}
source_snapshot >"$OUT/upstream-before.txt"

require_no_obs_running
LD_LIBRARY_PATH="$HOSTILE_LD" "$APPDIR/AppRun" --disable-shutdown-check --verbose \
    --profile Test --collection Test --startrecording >"$OUT/obs-profile-stdout.log" 2>&1 &
APP_PID=$!
OWN_PIDS+=("$APP_PID")
RECORD_SECONDS=10
MAPS_CHECKED=0
for i in $(seq 1 $((RECORD_SECONDS + 5))); do
    sleep 1
    alive "$APP_PID" || break
    OBS_PID="$(obs_pid_of "$APP_PID")"
    if [ "$i" -ge 5 ] && [ "$MAPS_CHECKED" -eq 0 ] && [ -n "$OBS_PID" ]; then
        # Once the captures run: their client libraries come from the AppImage.
        require_mapped_from_appimage "$OBS_PID" libpipewire-0.3.so libpulse.so libglib-2.0.so libgio-2.0.so
        MAPS_CHECKED=1
    fi
done
echo "PipeWire links and PulseAudio streams while OBS records:"
pw-link -l 2>&1 || true
pactl list short sources 2>&1 || true
pactl list short source-outputs 2>&1 || true
if alive "$APP_PID"; then
    [ "$MAPS_CHECKED" -eq 1 ] || fail "OBS's process was not found while it ran"
    # Quit like a user would (SIGINT closes the main window); recording stops.
    quit_obs "$APP_PID" "OBS with a saved PipeWire profile" "$OUT/obs-profile-stdout.log"
else
    set +e
    wait "$APP_PID"
    PROFILE_RC=$?
    set -e
    fail "OBS with a saved PipeWire profile $(describe_exit "$PROFILE_RC") during the first $RECORD_SECONDS s"
fi
echo "::endgroup::"

if [ "$CONFIG_NAME" = obs-studio-rust ]; then
    source_snapshot >"$OUT/upstream-after.txt"
    cmp "$OUT/upstream-before.txt" "$OUT/upstream-after.txt" || fail "the Rust frontend changed original settings"
    RUST_CONFIG="$XDG_CONFIG_HOME/obs-studio-rust"
    PROFILE_CONFIG="$RUST_CONFIG"
    SCENE_CONFIG="$RUST_CONFIG"
    PLUGIN_CONFIG="$RUST_CONFIG"
    if [ "$SCENARIO" = saved-profile-custom ]; then
        PROFILE_CONFIG="$PWD/locations/profiles/obs-studio-rust"
        SCENE_CONFIG="$PWD/locations/scenes/obs-studio-rust"
        PLUGIN_CONFIG="$PWD/locations/plugins/obs-studio-rust"
    fi
    [ -f "$PROFILE_CONFIG/basic/profiles/Test/basic.ini" ] || fail "the selected profile was not imported"
    [ -f "$SCENE_CONFIG/basic/scenes/Test.json" ] || fail "the selected scenes were not imported"
    grep -Fxq 'preserved plugin settings' "$PLUGIN_CONFIG/plugin_manager/migration-proof.txt" || fail "plugin-manager settings were not imported"
    if [ "$SCENARIO" != saved-profile-existing ]; then
        grep -Fxq "RustConfigImportedFrom=$LEGACY_CONFIG" "$RUST_CONFIG/global.ini" || fail "first-run migration was not recorded"
        for key in Configuration Profiles SceneCollections PluginManagerSettings; do
            expected="$XDG_CONFIG_HOME"
            if [ "$SCENARIO" = saved-profile-custom ]; then
                case "$key" in
                    Configuration) expected="$PWD/locations/configuration" ;;
                    Profiles) expected="$PWD/locations/profiles" ;;
                    SceneCollections) expected="$PWD/locations/scenes" ;;
                    PluginManagerSettings) expected="$PWD/locations/plugins" ;;
                esac
            fi
            grep -Fxq "$key=$expected" "$RUST_CONFIG/global.ini" || fail "the copied $key location lost its storage root"
        done
    else
        grep -q '^RustConfigImportedFrom=' "$RUST_CONFIG/global.ini" && fail "existing Rust settings were imported over"
    fi
    if [ "${#FAILED[@]}" -eq 0 ]; then
        echo "Rust config isolation verified: $SCENARIO; original settings unchanged"
    fi
fi

LOG="$(find "$XDG_CONFIG_HOME/$CONFIG_NAME/logs" -type f 2>/dev/null | sort | tail -1)"
if [ -n "$LOG" ]; then
    cp "$LOG" "$OUT/obs-profile.log"
    echo "::group::OBS log (saved profile)"
    cat "$LOG"
    echo "::endgroup::"
fi
echo "::group::OBS stdout/stderr (saved profile)"
cat "$OUT/obs-profile-stdout.log"
echo "::endgroup::"
require_no_crash_report

# The portal handed OBS its PipeWire connection: the point where every
# continuous build before obs-rust/obs-studio#426 crashed.
grep -q 'OpenPipeWireRemote' "$OUT/fake-portal.log" || fail "OBS never asked the portal for its PipeWire connection"
if [ -z "$LOG" ]; then
    fail "OBS with a saved profile wrote no log"
elif [ -n "$LAUNCH_ONLY" ]; then
    grep -q '\[pipewire\] Screencast session created' "$LOG" || fail "the screen capture never created a portal session"
else
    # Every source must have reached the point where frames and samples flow.
    grep -q '\[pipewire\] Screencast session created' "$LOG" || fail "the screen capture never created a portal session"
    [ "$(grep -c 'state: "streaming"' "$LOG" || true)" -ge 2 ] ||
        fail "the PipeWire screen capture and camera did not both reach the streaming state"
    grep -q "\[camera-portal\] Found device obs-test-camera" "$LOG" || fail "the PipeWire camera did not find the test camera"
    grep -q "Started recording from 'obs-test-mic'" "$LOG" || fail "the PulseAudio microphone did not start"
    grep -q 'Recording Start' "$LOG" || fail "the recording did not start"
    # Every module the AppImage ships must load. Allowed exceptions need
    # hardware or drivers no container has.
    loaded="$(sed -n '/Loaded Modules:/,/^[0-9:.]*: *-\{10\}/p' "$LOG")"
    while read -r m; do
        case "$m" in decklink.so | obs-nvenc.so) continue ;; esac
        grep -q " $m\$" <<<"$loaded" || fail "OBS did not load its module $m"
    done <"$INVENTORY"
fi
if [ -z "$LAUNCH_ONLY" ]; then
    REC="$(find "$REC_DIR" -name '*.mkv' 2>/dev/null | head -1)"
    if [ -n "$REC" ]; then
        cp "$REC" "$OUT/recording.mkv"
        echo "Recording: $(stat -c %s "$REC") bytes"
    else
        fail "OBS produced no recording"
    fi
    finish "verify-recording.sh checks $OUT/recording.mkv"
fi
finish "without test producers: no recording"
