#!/usr/bin/env bash
# Checks that everything AppRun points OBS at is really inside the AppDir:
#   check-appdir.sh <AppDir>
# assemble-appdir.sh runs it to fail the build, and test-appimage.sh runs it
# again on the extracted AppImage. Libraries that the dynamic loader can see are
# covered by assemble-appdir.sh; this is about what is found at runtime through
# environment variables and configuration files instead (PipeWire modules, SPA
# plugins, client.conf, gconv, Qt's X11 and Wayland plugins), where a missing file is not noticed
# until OBS uses it (obs-rust/obs-studio#426: no client.conf, so
# pw_context_new() returned NULL and OBS crashed on start with a PipeWire
# screen capture in the scene).
set -euo pipefail

APPDIR="$(realpath "$1")"
fail=0
error() {
    echo "::error::$*" >&2
    fail=1
}

# --- every path AppRun exports into the AppDir must exist -------------------
# (top-level `export NAME="$HERE/..."` lines; the probes in test-appimage.sh
# run with the same environment)
env_block="$(grep -E '^export [A-Z_]+="\$HERE' "$APPDIR/AppRun" || true)"
# shellcheck disable=SC2016 # expanded by the inner shell
exported="$(env -i HERE="$APPDIR" PATH="$PATH" sh -c "$env_block"'
env')"
count=0
while IFS='=' read -r name value; do
    first="${value%%:*}"
    case "$first" in
    "$APPDIR"/*) ;;
    *) continue ;;
    esac
    count=$((count + 1))
    if [ ! -e "$first" ]; then
        error "AppRun exports $name=$first, which is not in the AppImage"
    elif [ -d "$first" ] && [ -z "$(ls -A "$first")" ] && [ "$name" != GIO_MODULE_DIR ]; then
        # (GIO_MODULE_DIR is empty on purpose: see AppRun)
        error "AppRun exports $name=$first, which is empty"
    fi
done <<<"$exported"
[ "$count" -gt 0 ] || error "found no AppDir paths in AppRun's environment block"
var() { sed -n "s/^$1=//p" <<<"$exported" | head -1; }

# --- PipeWire: client.conf and everything it loads --------------------------
PW_CONF="$(var PIPEWIRE_CONFIG_DIR)/client.conf"
PW_MODULES="$(var PIPEWIRE_MODULE_DIR)"
SPA_PLUGINS="$(var SPA_PLUGIN_DIR)"
if [ ! -f "$PW_CONF" ]; then
    error "PipeWire client.conf is not bundled (expected $PW_CONF); pw_context_new() fails without it"
else
    # context.modules: name = libpipewire-module-...; optional ones carry "ifexists".
    while read -r line; do
        module="$(sed -n 's/.*name *= *\(libpipewire-module-[A-Za-z0-9_-]*\).*/\1/p' <<<"$line")"
        [ -n "$module" ] || continue
        if [ ! -e "$PW_MODULES/$module.so" ]; then
            if grep -q ifexists <<<"$line"; then
                echo "optional PipeWire module $module is not bundled"
            else
                error "client.conf loads $module, which is not in $PW_MODULES"
            fi
        fi
    done < <(tr -d '\n' <"$PW_CONF" | sed 's/}/}\n/g' | grep -o '{[^{}]*name *= *libpipewire-module-[^{}]*' || true)
    # context.spa-libs: <factory glob> = <plugin path below SPA_PLUGIN_DIR>
    while read -r lib; do
        [ -e "$SPA_PLUGINS/$lib.so" ] || error "client.conf maps a SPA factory to $lib, which is not in $SPA_PLUGINS"
    done < <(sed -n '/context.spa-libs/,/}/p' "$PW_CONF" | sed -n 's/^[^#]*= *\([a-z0-9_-]*\/libspa-[a-z0-9_-]*\).*/\1/p')
fi
for f in "$PW_MODULES/libpipewire-module-protocol-native.so" "$PW_MODULES/libpipewire-module-client-node.so" \
    "$PW_MODULES/libpipewire-module-adapter.so" "$SPA_PLUGINS/support/libspa-support.so"; do
    [ -e "$f" ] || error "${f#"$APPDIR"/} is not bundled; OBS cannot connect a PipeWire stream without it"
done

# --- gconv and Qt -----------------------------------------------------------
GCONV="$(var GCONV_PATH)"
# gconv belongs to glibc: checked only when the AppImage bundles glibc (built
# above the 2.28 floor). Without it, the host's glibc brings its own (#926).
if [ -d "$APPDIR/glibc" ]; then
    ls "$GCONV"/gconv-modules* >/dev/null 2>&1 || error "no gconv-modules index in $GCONV"
fi
# X11 and Wayland: the platform plugins and what the Wayland one needs to show
# a window (xdg-shell) and draw into it (wayland-egl).
QT_PLUGINS="$(var QT_PLUGIN_PATH)"
for plugin in platforms/libqxcb.so platforms/libqwayland.so wayland-shell-integration/libxdg-shell.so \
    wayland-graphics-integration-client/libqt-plugin-wayland-egl.so; do
    [ -e "$QT_PLUGINS/$plugin" ] || error "the Qt plugin $plugin is not bundled"
done

# --- libsrt: one copy, at least 1.5.7 ---------------------------------------
# The SRT output (obs-ffmpeg.so) and FFmpeg's srt:// protocol (libavformat)
# must share one libsrt. With two copies in one process (the Ubuntu-built
# AppImages bundled libsrt.so.1.5 and libsrt-gnutls.so.1.5) both ran the
# destructors of the same interposed C++ globals, and OBS aborted on quit
# after any SRT use (obs-rust/obs-studio#1082). Releases before 1.5.7 have
# public advisories (GHSA-6xg9-784j-24rm, GHSA-4mc6-qmpp-g7gw,
# GHSA-m96h-3rc9-xx6h).
SRT_MIN=1.5.7
# Symlinks count too: the AppDir holds only the soname file today, so a kept
# libsrt.so.1.5 -> libsrt.so.1.5.7 chain would need this count adjusted.
srt_libs="$(find "$APPDIR" \( -type f -o -type l \) -name 'libsrt*.so*' -printf '%P\n' | sort)"
srt_count="$(grep -c . <<<"$srt_libs" || true)"
if [ "$srt_count" -ne 1 ]; then
    error "expected exactly one libsrt in the AppImage, found $srt_count: $(tr '\n' ' ' <<<"$srt_libs")"
else
    SRT_LIB="$APPDIR/$srt_libs"
    SRT_NAME="$(basename "$SRT_LIB")"
    # libsrt keeps its version string (SRT_VERSION, which it parses into
    # SRT_DEF_VERSION at load time) in .rodata; read it without binutils,
    # which the test containers do not have. This assumes SRT_VERSION is the
    # only bare x.y.z string in the library, which holds while mbedTLS is a
    # shared library. A statically linked TLS library could add its own; then
    # the check fails with "cannot read the version" (more than one match), it
    # never passes a wrong version.
    srt_version="$(tr '\0' '\n' <"$SRT_LIB" | grep -xE '[0-9]+\.[0-9]+\.[0-9]+' || true)"
    if [ "$(grep -c . <<<"$srt_version" || true)" -ne 1 ]; then
        error "cannot read the version of $srt_libs (version strings found: $(tr '\n' ' ' <<<"$srt_version"))"
    elif [ "$(printf '%s\n%s\n' "$srt_version" "$SRT_MIN" | sort -V | head -1)" != "$SRT_MIN" ]; then
        error "the bundled $srt_libs is libsrt $srt_version; at least $SRT_MIN is required"
    fi
    # No bundled ELF file may ask for another libsrt (libsrt-gnutls.so.1.5,
    # libsrt-openssl.so.1.5, ...): the loader would map a second copy, from the
    # host if not from the AppImage. DT_NEEDED names are plain strings in .dynstr.
    while IFS= read -r -d '' f; do
        while read -r soname; do
            [ "$soname" = "$SRT_NAME" ] ||
                error "${f#"$APPDIR"/} needs $soname, a second libsrt next to the bundled $SRT_NAME"
        done < <(grep -aoE 'libsrt[A-Za-z0-9_-]*\.so(\.[0-9]+)*' "$f" | sort -u)
    done < <(grep -rlaZ 'libsrt[A-Za-z0-9_-]*\.so' "$APPDIR/usr" || true)
fi

if [ "$fail" -ne 0 ]; then
    echo "check-appdir: FAILED" >&2
    exit 1
fi
echo "check-appdir: all $count AppDir paths that AppRun exports exist; PipeWire, gconv and Qt X11/Wayland runtime files are bundled; one libsrt ($srt_version)"
