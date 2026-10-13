#!/bin/sh
# Runs test_pipewire_cursor_hdr against a private PipeWire daemon: its own
# runtime dir and config, no session manager, no devices, and never the
# user's session bus. Exits 77 (skip) when pipewire is not installed, and
# fails when it is installed but the daemon does not start.
# usage: test_pipewire_cursor_hdr.sh <test binary>
test_bin=$1

command -v pipewire >/dev/null 2>&1 || {
	echo "pipewire not found: skipping"
	exit 77
}

# The daemon's longest socket path, <dir>/pipewire-0-manager, must fit a Unix
# socket address (107 bytes); with a deep TMPDIR (a build tree) it does not
# and pipewire exits at once. Use /tmp then.
dir=$(mktemp -d "${TMPDIR:-/tmp}/obs-pw-cursor-test.XXXXXX") || exit 1
if [ $((${#dir} + 19)) -gt 107 ]; then
	rm -rf "$dir"
	dir=$(mktemp -d "/tmp/obs-pw-cursor-test.XXXXXX") || exit 1
fi
export XDG_RUNTIME_DIR="$dir" PIPEWIRE_RUNTIME_DIR="$dir"
export XDG_CONFIG_HOME="$dir/config" XDG_STATE_HOME="$dir/state" HOME="$dir/home"
unset DBUS_SESSION_BUS_ADDRESS PIPEWIRE_REMOTE
mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$HOME"

pipewire >"$dir/pipewire.log" 2>&1 &
pw=$!

# pipewire is installed, so a daemon that does not come up is a failure, not
# a reason to skip: ctest counts skipped tests as passed.
i=0
until [ -S "$dir/pipewire-0" ]; do
	i=$((i + 1))
	if ! kill -0 $pw 2>/dev/null || [ $i -gt 200 ]; then
		echo "private pipewire did not start:"
		cat "$dir/pipewire.log"
		kill $pw 2>/dev/null
		rm -rf "$dir"
		exit 1
	fi
	sleep 0.1
done

"$test_bin"
status=$?

kill $pw 2>/dev/null
wait $pw 2>/dev/null
rm -rf "$dir"
exit $status
