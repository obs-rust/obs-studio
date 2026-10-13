#!/bin/sh
# Runs test_pipewire_context_fail. When pipewire is installed, starts a
# private daemon for the control case: its own runtime dir and a minimal
# configuration (native protocol and access control only: without the access
# module clients never get permissions and hang; no D-Bus, no realtime or X11
# modules, no session manager, no devices). Without one, the test still runs and only
# the control case skips; a pipewire that is installed but does not start fails.
# usage: test_pipewire_context_fail.sh <test binary>
test_bin=$1

unset DISPLAY WAYLAND_DISPLAY HYPRLAND_INSTANCE_SIGNATURE DBUS_SESSION_BUS_ADDRESS DBUS_SYSTEM_BUS_ADDRESS
unset PIPEWIRE_REMOTE PIPEWIRE_RUNTIME_DIR PIPEWIRE_CONFIG_DIR PIPEWIRE_CONFIG_NAME

# The daemon's longest socket path, <dir>/run/pipewire-0-manager, must fit a
# Unix socket address (107 bytes); with a deep TMPDIR (a build tree) it does
# not and pipewire exits at once. Use /tmp then.
dir=$(mktemp -d "${TMPDIR:-/tmp}/obs-pw-context-test.XXXXXX") || exit 1
if [ $((${#dir} + 23)) -gt 107 ]; then
	rm -rf "$dir"
	dir=$(mktemp -d "/tmp/obs-pw-context-test.XXXXXX") || exit 1
fi
pw=
cleanup() {
	[ -n "$pw" ] && kill "$pw" 2>/dev/null && wait "$pw" 2>/dev/null
	rm -rf "$dir"
}
trap cleanup EXIT
trap 'exit 1' INT TERM

export XDG_RUNTIME_DIR="$dir/run" XDG_CONFIG_HOME="$dir/config" XDG_STATE_HOME="$dir/state" HOME="$dir/home"
mkdir -p "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$HOME" "$dir/daemon-config"

if command -v pipewire >/dev/null 2>&1; then
	cat >"$dir/daemon-config/pipewire.conf" <<'EOF'
context.properties = {
    core.daemon = true
    core.name   = pipewire-0
    support.dbus = false
}
context.spa-libs = {
    support.* = support/libspa-support
}
context.modules = [
    { name = libpipewire-module-protocol-native }
    { name = libpipewire-module-access }
]
EOF
	PIPEWIRE_CONFIG_DIR="$dir/daemon-config" PIPEWIRE_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
		pipewire >"$dir/pipewire.log" 2>&1 &
	pw=$!

	i=0
	until [ -S "$XDG_RUNTIME_DIR/pipewire-0" ]; do
		i=$((i + 1))
		# pipewire is installed, so a daemon that does not come up is a
		# failure: the control case would skip and ctest count it as passed.
		if [ $i -gt 200 ] || ! kill -0 "$pw" 2>/dev/null; then
			echo "private pipewire did not start:"
			cat "$dir/pipewire.log"
			exit 1
		fi
		sleep 0.1
	done
	export PIPEWIRE_RUNTIME_DIR="$XDG_RUNTIME_DIR"
else
	echo "pipewire not found; the control case will skip"
fi

"$test_bin"
exit $?
