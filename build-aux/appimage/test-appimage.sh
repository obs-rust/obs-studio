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
USAGE="usage: test-appimage.sh <AppImage> <output-dir> <obs-appimage-testkit> <bundle|first-start|probes|saved-profile>"
TESTKIT="$(realpath "${3:?$USAGE}")"
SCENARIO="${4:?$USAGE}"
case "$SCENARIO" in
bundle | first-start | probes | saved-profile) ;;
*)
    echo "::error::unknown scenario $SCENARIO; $USAGE"
    exit 1
    ;;
esac
HERE_DIR="$(dirname "$(realpath "$0")")"
mkdir -p "$OUT"
cd "$(mktemp -d)"
FAILED=()
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
        for p in $(ps -eo pid=,comm= | awk '$2 == "obs" { print $1 }'); do alive "$p" && left="$left $p"; done
        [ -z "$left" ] && break
        sleep 0.5
    done
    if [ -n "$left" ]; then
        # shellcheck disable=SC2086 # a list of PIDs
        ps -o pid,stat,comm,args -p "$(echo $left | tr ' ' ,)" || true
        fail "processes of the $SCENARIO scenario were still running 10 s after SIGTERM"
    fi
    if grep '@/com/obsproject' /proc/net/unix 2>/dev/null; then
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
    if grep '@/com/obsproject' /proc/net/unix 2>/dev/null; then
        ps -eo pid,ppid,stat,comm,args 2>&1 | head -40
        fail "an OBS is already running in this container, so the $SCENARIO scenario cannot start one"
        finish
    fi
}
# The obs process among the descendants of $1 (AppRun runs it as a child).
obs_pid_of() {
    local p
    for p in $(descendants "$1"); do
        [ "$(cat "/proc/$p/comm" 2>/dev/null)" = obs ] && { echo "$p"; return; }
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
        ca-certificates xvfb icewm x11-utils imagemagick fonts-dejavu-core gdb procps dbus \
        libgl1-mesa-dri libglx-mesa0 libegl-mesa0 >/dev/null
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
        xorg-x11-server-Xvfb icewm xwininfo ImageMagick dbus-daemon \
        dejavu-sans-fonts gdb procps-ng mesa-dri-drivers mesa-libGL mesa-libEGL >/dev/null
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

"$APPIMAGE" --appimage-extract >/dev/null
APPDIR="$PWD/squashfs-root"
LOADER="$APPDIR/lib64/ld-linux-x86-64.so.2"
# The helper is built with RPATH $ORIGIN/../lib, so placed next to obs it finds
# the bundled libraries exactly the way obs does (no --library-path).
install -m755 "$TESTKIT" "$APPDIR/usr/bin/obs-appimage-testkit"

# The modules the build installed (assemble-appdir.sh records them).
INVENTORY="$APPDIR/usr/share/obs/appimage-modules.txt"
if [ ! -f "$INVENTORY" ]; then
    echo "::warning::no module inventory (built before obs-rust/obs-studio#426); using the modules present"
    find "$APPDIR/usr/lib/obs-modules" -name '*.so' -printf '%f\n' | sort >"$PWD/modules.txt"
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
    unresolved=0
    checked=0
    while IFS= read -r -d '' f; do
        [ "$(head -c4 "$f" | od -An -c | tr -d ' ')" = '177ELF' ] || continue
        case "$(basename "$f")" in ld-linux*) continue ;; esac
        checked=$((checked + 1))
        if ! out="$(LD_TRACE_LOADED_OBJECTS=1 LD_WARN=yes LD_BIND_NOW=yes "$LOADER" "$f" 2>&1)" ||
            grep -qE 'not found|undefined symbol|error while loading' <<<"$out"; then
            echo "${f#"$APPDIR"/}:"
            grep -E 'not found|undefined symbol|error while loading' <<<"$out" | sed "s|$APPDIR/||g" | head -5
            unresolved=$((unresolved + 1))
        elif grep '=> /' <<<"$out" | grep -v "$APPDIR" | grep -vE '=> [^ ]*/(libGL|libEGL|libGLX|libgbm|libdrm|libvulkan|libcuda|libnvidia)' | grep .; then
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
    if [ -e "$APPDIR/usr/lib/libc.so.6" ]; then
        for lib in libm.so.6 libmvec.so.1 librt.so.1 libpthread.so.0 libdl.so.2 libutil.so.1 \
            libanl.so.1 libresolv.so.2 libnss_files.so.2 libnss_dns.so.2; do
            out="$(LD_TRACE_LOADED_OBJECTS=1 LD_WARN=yes LD_BIND_NOW=yes LD_PRELOAD="$lib" \
                "$LOADER" "$APPDIR/usr/bin/obs" 2>&1 || true)"
            where="$(awk -v l="$lib" '$1 == l && $2 == "=>" {print $3}' <<<"$out")"
            [ "$where" = not ] && where="nothing (not found)"
            if [[ "$where" != "$APPDIR"/* ]]; then
                fail "glibc's $lib is not bundled with libc.so.6: a host driver that links it gets ${where:-nothing}"
            elif grep -E 'undefined symbol|error while loading' <<<"$out"; then
                fail "the bundled $lib does not load with the bundled loader (see above)"
            fi
        done
    fi
    # The modules the build installed are all there.
    while read -r m; do
        [ -n "$(find "$APPDIR/usr/lib/obs-modules" -name "$m" -print -quit)" ] || fail "module $m from the build is not in the AppImage"
    done <"$INVENTORY"
    echo "$(wc -l <"$INVENTORY") modules in the build inventory"
    echo "::endgroup::"
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
    mkdir -p "$HOME/.config/obs-studio" # OBS only creates one directory level at a time
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
        if grep -qE '"OBS [0-9]|"obs"' <<<"$WINDOWS"; then
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
        require_mapped_from_appimage "$OBS_PID" libqsvgicon.so libqsvg.so libc.so.6 libstdc++.so.6 \
            libQt6Core.so libQt6Widgets.so libavcodec.so libobs.so libX11.so libxcb.so
    else
        fail "OBS exited during its first start"
    fi
    if alive "$APP_PID"; then
        quit_obs "$APP_PID" "OBS on its first start" "$OUT/obs-stdout.log"
    fi
    LOG="$(find "$HOME/.config/obs-studio/logs" -type f 2>/dev/null | sort | tail -1)"
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
    if [ -n "$LOG" ] && grep -qiE 'Failed to initialize video|Failed to load.*libobs-opengl' "$LOG"; then
        grep -iE 'Failed to' "$LOG"
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
        exec "$LOADER" "$APPDIR/usr/bin/obs-appimage-testkit" "$@"
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

LOG="$(find "$XDG_CONFIG_HOME/obs-studio/logs" -type f 2>/dev/null | sort | tail -1)"
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
