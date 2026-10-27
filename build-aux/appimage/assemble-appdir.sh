#!/usr/bin/env bash
# Turn a `cmake --install` tree (prefix /usr) into a self-contained AppDir that
# also bundles glibc, so it runs on distributions older and newer than the one
# that built it. Usage: assemble-appdir.sh <AppDir> <validated-libavutil.so.60>   (AppDir already holds usr/)
set -euo pipefail

# Do not build an image that will fail on older host VA libraries. The second
# input is the glibc 2.28 image's FFmpeg libavutil (build.py's AVUTIL_LIB),
# built against that image's old libva; the checks below audit it.
COMPAT_LIB="${2:-}"
if [ -z "$COMPAT_LIB" ] || [ ! -f "$COMPAT_LIB" ]; then
    echo "::error::provide a validated VA-compatible libavutil (build.py's AVUTIL_LIB)" >&2
    exit 2
fi
COMPAT_DYNAMIC="$(readelf -d "$COMPAT_LIB")"
grep -Fq '[libavutil.so.60]' <<<"$COMPAT_DYNAMIC" || {
    echo "::error::compatibility library has the wrong libavutil SONAME" >&2; exit 2;
}
COMPAT_SYMBOLS="$(readelf --dyn-syms --wide "$COMPAT_LIB")"
if grep -Eq '[[:space:]]UND[[:space:]].*vaMapBuffer2' <<<"$COMPAT_SYMBOLS"; then
    echo "::error::compatibility library still requires vaMapBuffer2" >&2
    exit 2
fi
APPDIR="$(realpath "$1")"
HERE="$(dirname "$(realpath "$0")")"
LIBDIR="$APPDIR/usr/lib"
GUI_NAME=obs
APP_ID=com.obsproject.Studio
DATA_ROOT="$APPDIR/usr/share/obs"
MODULE_ROOT="$LIBDIR/obs-modules"
if [ -f "$APPDIR/usr/bin/obs-studio-rust-gui" ]; then
    GUI_NAME=obs-studio-rust-gui
    APP_ID=io.github.obs_rust.OBSStudioRust
    DATA_ROOT="$APPDIR/usr/share/obs-studio-rust"
    MODULE_ROOT="$LIBDIR/obs-studio-rust/obs-modules"
    # AppDir aliases serve bundled dependency discovery and its test probes.
    # The installed Debian package retains only the private library locations.
    for library in "$LIBDIR/obs-studio-rust"/lib*.so*; do
        [ -e "$library" ] || continue
        ln -s "obs-studio-rust/$(basename "$library")" "$LIBDIR/$(basename "$library")"
    done
fi
MULTIARCH="$(gcc -print-multiarch 2>/dev/null || true)"
if [ -n "$MULTIARCH" ] && [ -d "/usr/lib/$MULTIARCH" ]; then
    SYSLIB="/usr/lib/$MULTIARCH" # Debian/Ubuntu
else
    SYSLIB=/usr/lib64 # AlmaLinux (manylinux_2_28)
fi
# The oldest glibc the AppImage runs on (check-glibc-floor.sh, #926). A build
# host at or below it means every supported host has a newer glibc than the
# one OBS was linked against, so glibc and the C++ runtime come from the host
# and nothing of glibc is bundled (no loader, no interpreter patching, no
# gconv). Above it (the Ubuntu 26.04 build), glibc is bundled as before.
GLIBC_FLOOR=2.28
build_glibc="$(getconf GNU_LIBC_VERSION | sed 's/^glibc //')"
if [ "$(printf '%s\n%s\n' "$build_glibc" "$GLIBC_FLOOR" | sort -V | tail -1)" = "$GLIBC_FLOOR" ]; then
    BUNDLE_GLIBC=0
else
    BUNDLE_GLIBC=1
fi
echo "build glibc $build_glibc, floor $GLIBC_FLOOR: bundle glibc=$BUNDLE_GLIBC"

# Libraries that must come from the host because they are the GPU driver stack
# (the AppImage excludelist): the host's Mesa/NVIDIA/VA-API drivers are built
# against the host's copies of these and look for their backends in the host's
# directories, so a bundled copy would shadow them through the relative RPATH
# (obs-rust/obs-studio#625). Everything else is bundled.
EXCLUDE='^(linux-vdso|libcuda|libnvidia|libnvcuvid|libGL\.so|libGLX|libEGL|libOpenGL|libGLdispatch|libGLESv|libglapi|libgbm|libdrm|libvulkan|libva(\.so|-)|libwayland-(client|egl|server)|libgallium|libLLVM)'
if [ "$BUNDLE_GLIBC" = 0 ]; then
    # glibc and the C++ runtime are the host's. gcc-toolset links the parts of
    # libstdc++ newer than the system's 3.4.25 statically, so the host's
    # libstdc++ is enough (check-glibc-floor.sh checks GLIBCXX too).
    EXCLUDE="${EXCLUDE%)}|ld-linux|libc\.so|libm\.so|libmvec|libpthread|libdl\.so|librt\.so|libresolv|libnss_|libutil\.so|libanl|libBrokenLocale|libthread_db|libstdc\+\+|libgcc_s)"
fi

mkdir -p "$LIBDIR"

# --- Qt plugins -------------------------------------------------------------
# Qt must load plugins from the bundle only (qt.conf below). A plugin that is not
# bundled is not "found on the host instead": it is silently missing, or - worse, when
# the host's Qt differs - loaded from the host and crashing (QIcon -> libqsvgicon.so).
OBS_DEPS="${OBS_DEPS:-/opt/obs-deps}" # the manylinux base's source-built libraries
# Ask the Qt we built against where its plugins are; never search the file
# system for them (a system Qt 5 in /usr/lib64/qt5/plugins matched first once,
# and Qt 5 plugins next to Qt 6 libraries would crash at load).
QT_PLUGINS=""
for qtpaths in "$OBS_DEPS/qt6/bin/qtpaths6" "$OBS_DEPS/qt6/bin/qtpaths" qtpaths6; do
    if command -v "$qtpaths" >/dev/null 2>&1; then
        QT_PLUGINS="$("$qtpaths" --query QT_INSTALL_PLUGINS 2>/dev/null)" && break
    fi
done
[ -d "$QT_PLUGINS" ] || QT_PLUGINS="$SYSLIB/qt6/plugins"
[ -f "$QT_PLUGINS/platforms/libqxcb.so" ] || {
    echo "::error::no Qt 6 plugins (looked at $QT_PLUGINS)" >&2
    exit 1
}
echo "Qt plugins from: $(realpath "$QT_PLUGINS")"
mkdir -p "$LIBDIR/qt6/plugins"
for d in platforms platforminputcontexts platformthemes imageformats iconengines styles generic \
    networkinformation tls xcbglintegrations egldeviceintegrations \
    wayland-decoration-client wayland-graphics-integration-client wayland-shell-integration; do
    [ -d "$QT_PLUGINS/$d" ] && cp -a "$QT_PLUGINS/$d" "$LIBDIR/qt6/plugins/"
done
# The ones OBS cannot live without (SVG icons/themes, X11). Search the whole system if
# the package put them somewhere unexpected, and fail the build if they do not exist.
for need in platforms/libqxcb.so imageformats/libqsvg.so iconengines/libqsvgicon.so; do
    if [ ! -e "$LIBDIR/qt6/plugins/$need" ]; then
        found="$(find "$QT_PLUGINS/.." /usr/lib/*/qt6 /usr/lib64/qt6 -path "*/$need" 2>/dev/null | head -1)"
        if [ -n "$found" ]; then
            mkdir -p "$LIBDIR/qt6/plugins/$(dirname "$need")"
            cp -aL "$found" "$LIBDIR/qt6/plugins/$need"
        else
            echo "::error::Qt plugin $need not found; without it OBS has no SVG icons or loads the host's copy" >&2
            dpkg -L libqt6svg6 2>/dev/null | grep -i plugin >&2 || true
            exit 1
        fi
    fi
done
# Qt's GTK 3 platform theme would pull GTK 3 into the bundle; OBS brings its own
# themes and Qt keeps the xdg-desktop-portal one. (The Ubuntu Qt never had it.)
rm -f "$LIBDIR/qt6/plugins/platformthemes/libqgtk3.so"
find "$LIBDIR/qt6/plugins" -name '*.so' | sed "s|$LIBDIR/||" | sort

# Qt must only ever look at the bundled plugins (see AppRun).
printf '[Paths]\nPrefix = ..\nPlugins = lib/qt6/plugins\n' >"$APPDIR/usr/bin/qt.conf"
# Empty on purpose: GIO modules from the host are linked against the host's GLib.
mkdir -p "$LIBDIR/gio/modules"

# --- dlopen()ed runtime pieces that ldd cannot see ---------------------------
if [ "$BUNDLE_GLIBC" = 1 ]; then
    mkdir -p "$LIBDIR/gconv"
    cp -a "$SYSLIB"/gconv/. "$LIBDIR/gconv/"
    for so in libnss_files.so.2 libnss_dns.so.2 libresolv.so.2; do
        [ -e "$SYSLIB/$so" ] && cp -aL "$SYSLIB/$so" "$LIBDIR/"
    done
fi
[ -e "$SYSLIB/libxkbcommon-x11.so.0" ] && cp -aL "$SYSLIB/libxkbcommon-x11.so.0" "$LIBDIR/"
# The bundled libpipewire loads its modules, SPA plugins and client.conf from
# the directories AppRun points it at. Without them pw_context_new() returns
# NULL and OBS crashes as soon as a PipeWire capture starts
# (obs-rust/obs-studio#426), so a build host that lacks them must fail here.
SPA_DIR="$(pkg-config --variable=plugindir libspa-0.2 2>/dev/null || echo "$SYSLIB/spa-0.2")"
PW_MODULES="$(pkg-config --variable=moduledir libpipewire-0.3 2>/dev/null || echo "$SYSLIB/pipewire-0.3")"
PW_SHARE="$(pkg-config --variable=prefix libpipewire-0.3 2>/dev/null || echo /usr)/share/pipewire"
for need in "$SPA_DIR/support/libspa-support.so:libspa-0.2-modules" \
    "$PW_MODULES/libpipewire-module-protocol-native.so:libpipewire-0.3-modules" \
    "$PW_SHARE/client.conf:pipewire-bin"; do
    if [ ! -e "${need%%:*}" ]; then
        echo "::error::${need%%:*} not found; install ${need##*:} on the build host" >&2
        exit 1
    fi
done
mkdir -p "$LIBDIR/spa-0.2" && cp -a "$SPA_DIR/." "$LIBDIR/spa-0.2/"
mkdir -p "$LIBDIR/pipewire-0.3" && cp -a "$PW_MODULES/." "$LIBDIR/pipewire-0.3/"
mkdir -p "$APPDIR/usr/share/pipewire"
cp -a "$PW_SHARE/." "$APPDIR/usr/share/pipewire/"

# --- the modules the build installed, for test-appimage.sh -------------------
mkdir -p "$DATA_ROOT"
find "$MODULE_ROOT" -name '*.so' -printf '%f\n' | sort >"$DATA_ROOT/appimage-modules.txt"

# --- bundle the shared-library closure of every ELF in the AppDir ------------
# Install the source-built version first, so closure discovery uses it and
# cannot replace it with the build host's library requiring a newer VA API.
install -m 644 "$COMPAT_LIB" "$LIBDIR/libavutil.so.60"
# Prepend: on the glibc 2.28 base the prebuilt libraries (/opt/obs-deps/<tool>/lib)
# are only on the image's LD_LIBRARY_PATH, and the closure must find them to bundle them.
export LD_LIBRARY_PATH="$LIBDIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

is_elf() { [ "$(head -c4 "$1" 2>/dev/null | od -An -c | tr -d ' ')" = '177ELF' ]; }

bundle_deps() {
    local f="$1" out lib
    out="$(ldd "$f" 2>/dev/null || true)"
    echo "$out" | awk '/=> \//{print $3}' | while read -r lib; do
        name="$(basename "$lib")"
        if [[ "$name" =~ $EXCLUDE ]]; then continue; fi
        [ -e "$LIBDIR/$name" ] || { cp -aL "$lib" "$LIBDIR/$name"; echo "$LIBDIR/$name"; }
    done
    if echo "$out" | grep -q 'not found'; then
        echo "::error::unresolved dependency for $f" >&2
        echo "$out" | grep 'not found' >&2
        return 1
    fi
}

queue="$(find "$APPDIR/usr" -type f \( -name '*.so' -o -name '*.so.*' -o -perm -u+x \) | while read -r f; do is_elf "$f" && echo "$f"; done)"
while [ -n "$queue" ]; do
    next=""
    while read -r f; do
        [ -n "$f" ] || continue
        new="$(bundle_deps "$f")"
        next="$next"$'\n'"$new"
    done <<<"$queue"
    queue="$(echo "$next" | sed '/^$/d')"
done

# --- host libraries: shims for optional ones, the preflight for required ones (#959)
# After the closure (the shims replace DT_NEEDED entries it found) and before the
# RPATHs are set (set-runpaths.sh then covers the shims and obs-hostlib-check).
uv run --no-project "$HERE/hostlib/build.py" "$APPDIR"

if [ "$BUNDLE_GLIBC" = 1 ]; then
# --- the rest of glibc ------------------------------------------------------
# The closure holds only the parts of glibc that OBS links. The host's GPU drivers
# run on the bundled loader and libc too and link more of it (the NVIDIA driver's
# libcuda and libEGL_nvidia: librt, libpthread, libdl). Taken from the host, those
# belong to its glibc and need private symbols of its version: on a host with a
# newer glibc libcuda did not load and NVENC was missing. So bundle every library
# of the package the bundled libc.so.6 comes from.
if [ -e "$LIBDIR/libc.so.6" ]; then
    glibc_pkg="$(dpkg-query -S "$SYSLIB/libc.so.6" 2>/dev/null | head -1 | cut -d: -f1-2)" || true
    if [ -z "$glibc_pkg" ]; then
        echo "::error::no package owns $SYSLIB/libc.so.6; cannot bundle the rest of glibc" >&2
        exit 1
    fi
    dpkg-query -L "$glibc_pkg" | grep -E "^$SYSLIB/[^/]+\.so(\.[0-9]+)*\$" | while read -r lib; do
        name="$(basename "$lib")"
        case "$name" in
        ld-linux*) continue ;;     # bundled below, in lib64
        libthread_db*) continue ;; # a plugin of the debugger, which provides its ps_* symbols
        esac
        [ -e "$LIBDIR/$name" ] || { cp -aL "$lib" "$LIBDIR/$name"; echo "glibc ($glibc_pkg): $name"; }
        echo "$name" >>"$PWD/.glibc-names.$$"
    done
fi

# --- the C and C++ runtime goes to its own directory (see AppRun) -------------
# glibc, libstdc++ and libgcc_s must be switchable: a host with a newer glibc
# runs its own Mesa/LLVM (built against that glibc and a newer libstdc++), which
# cannot load against the bundled, older ones (obs-rust/obs-studio#621). AppRun
# chooses at start. They are therefore NOT in usr/lib: the RPATH of every ELF
# file ends with a fixed directory that AppRun fills with the bundled copies,
# or leaves empty so that the host's are found.
GLIBC_DIR="$APPDIR/glibc"
mkdir -p "$GLIBC_DIR"
if [ -e "$LIBDIR/libc.so.6" ]; then
    { cat "$PWD/.glibc-names.$$"; echo libstdc++.so.6; echo libgcc_s.so.1; } | sort -u | while read -r name; do
        if [ -e "$LIBDIR/$name" ]; then mv "$LIBDIR/$name" "$GLIBC_DIR/$name"; fi
    done
    rm -f "$PWD/.glibc-names.$$"
fi
cp -aL "$SYSLIB/ld-linux-x86-64.so.2" "$GLIBC_DIR/ld-linux-x86-64.so.2"
# What AppRun compares the host against: the glibc version, and the newest
# GLIBCXX symbol version libstdc++ provides.
"$GLIBC_DIR/ld-linux-x86-64.so.2" --version | sed -n '1s/.*release version \([0-9][0-9]*\.[0-9][0-9]*\).*/\1/p' >"$GLIBC_DIR/GLIBC_VERSION"
grep -a -o 'GLIBCXX_3\.4\.[0-9]*' "$GLIBC_DIR/libstdc++.so.6" | sed 's/.*\.//' | sort -n | tail -1 >"$GLIBC_DIR/GLIBCXX_MINOR"
if [ ! -s "$GLIBC_DIR/GLIBC_VERSION" ] || [ ! -s "$GLIBC_DIR/GLIBCXX_MINOR" ]; then
    echo "::error::cannot determine the bundled glibc/libstdc++ version" >&2
    exit 1
fi
echo "bundled glibc $(cat "$GLIBC_DIR/GLIBC_VERSION"), GLIBCXX_3.4.$(cat "$GLIBC_DIR/GLIBCXX_MINOR")"

# --- the dynamic loader (see AppRun) ---------------------------------------
mkdir -p "$APPDIR/lib64"
cp -aL "$SYSLIB/ld-linux-x86-64.so.2" "$APPDIR/lib64/ld-linux-x86-64.so.2"
rm -f "$LIBDIR/ld-linux-x86-64.so.2"
# The directory name identifies this exact C/C++ runtime, so that a rebuild with
# a different libstdc++ never reuses a stale copy of it in /tmp.
LOADER_HASH="$(cd "$GLIBC_DIR" && find . -type f | sort | xargs sha256sum | sha256sum | cut -c1-12)"
LOADER_DIR="/tmp/.obs-rust-appimage-$LOADER_HASH"
LOADER="$LOADER_DIR/ld-linux-x86-64.so.2"

# Every executable (not libraries) gets the bundled loader as interpreter.
find "$APPDIR/usr" -type f -perm -u+x ! -name '*.so' ! -name '*.so.*' | while read -r f; do
    is_elf "$f" || continue
    patchelf --print-interpreter "$f" >/dev/null 2>&1 || continue
    patchelf --set-interpreter "$LOADER" "$f"
    echo "interpreter patched: ${f#"$APPDIR"/}"
done

else
    # Every ELF keeps the system interpreter and finds glibc and the C++
    # runtime on the host; AppRun has nothing to install.
    LOADER=""
    LOADER_DIR=""
fi

# --- relative RPATHs instead of an exported LD_LIBRARY_PATH (see set_runpaths.py) ---
unset LD_LIBRARY_PATH # patchelf and friends are host programs
uv run --no-project "$HERE/set_runpaths.py" "$APPDIR" ${LOADER_DIR:+"$LOADER_DIR/lib"}

# --- desktop integration -----------------------------------------------------
sed -e "s|@LOADER@|$LOADER|" -e "s|@BUNDLE_GLIBC@|$BUNDLE_GLIBC|" -e "s|@OBS_GUI_NAME@|$GUI_NAME|" "$HERE/AppRun" >"$APPDIR/AppRun"
chmod +x "$APPDIR/AppRun"
cp "$APPDIR/usr/share/applications/$APP_ID.desktop" "$APPDIR/"
cp "$APPDIR/usr/share/icons/hicolor/256x256/apps/$APP_ID.png" "$APPDIR/"
ln -sf "$APP_ID.png" "$APPDIR/.DirIcon"

# --- everything AppRun points OBS at at runtime must be in the AppDir --------
uv run --no-project "$HERE/check_appdir.py" "$APPDIR"

# --- nothing in the AppDir may need a glibc newer than the floor (#926) -------
uv run --script "$HERE/check-glibc-floor.py" "$APPDIR" 2.28

# --- sanity: nothing may resolve outside the AppDir except the excluded host libs
echo "::group::Libraries still resolved from the host"
# (the binary's interpreter now points at the not-yet-installed loader, so ask the bundled one)
if [ "$BUNDLE_GLIBC" = 1 ]; then
    "$APPDIR/lib64/ld-linux-x86-64.so.2" --library-path "$GLIBC_DIR:$LIBDIR" --list "$APPDIR/usr/bin/$GUI_NAME" |
        grep -v "$APPDIR" | grep '=> /' || true
else
    ldd "$APPDIR/usr/bin/$GUI_NAME" | grep -v "$APPDIR" | grep '=> /' || true
fi
echo "::endgroup::"
du -sh "$APPDIR"
