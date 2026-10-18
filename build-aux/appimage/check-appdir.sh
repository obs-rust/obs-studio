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

if [ "$fail" -ne 0 ]; then
    echo "check-appdir: FAILED" >&2
    exit 1
fi
echo "check-appdir: all $count AppDir paths that AppRun exports exist; PipeWire, gconv and Qt X11/Wayland runtime files are bundled"
