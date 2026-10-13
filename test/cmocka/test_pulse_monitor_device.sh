#!/bin/sh
# Runs a PulseAudio end-to-end test (test_pulse_monitor_device,
# test_pulse_stream_restart) against a private PipeWire sound server:
# its own runtime dir, config and D-Bus, no hardware devices. Exits 77 (skip)
# when PipeWire, WirePlumber, pipewire-pulse, pactl or dbus-run-session are
# missing, and fails when they are installed but the server does not start.
# usage: test_pulse_monitor_device.sh <test binary>
test_bin=$1

if [ -z "$PRIVATE_PIPEWIRE_STARTED" ]; then
	for tool in pipewire wireplumber pipewire-pulse pactl dbus-run-session; do
		command -v "$tool" >/dev/null 2>&1 || {
			echo "$tool not found: skipping"
			exit 77
		}
	done
	PRIVATE_PIPEWIRE_STARTED=1 exec dbus-run-session -- sh "$0" "$@"
fi

# The daemon's longest socket path, <dir>/pipewire-0-manager, must fit a Unix
# socket address (107 bytes); with a deep TMPDIR (a build tree) it does not
# and pipewire exits at once. Use /tmp then.
dir=$(mktemp -d "${TMPDIR:-/tmp}/obs-pw-test.XXXXXX") || exit 1
if [ $((${#dir} + 19)) -gt 107 ]; then
	rm -rf "$dir"
	dir=$(mktemp -d "/tmp/obs-pw-test.XXXXXX") || exit 1
fi
export XDG_RUNTIME_DIR="$dir" PIPEWIRE_RUNTIME_DIR="$dir" PULSE_RUNTIME_PATH="$dir/pulse"
export XDG_CONFIG_HOME="$dir/config" XDG_STATE_HOME="$dir/state" HOME="$dir/home"
mkdir -p "$XDG_CONFIG_HOME/wireplumber/wireplumber.conf.d" "$XDG_STATE_HOME" "$HOME"
cat >"$XDG_CONFIG_HOME/wireplumber/wireplumber.conf.d/90-no-hardware.conf" <<EOF
wireplumber.profiles = {
  main = {
    monitor.alsa = disabled
    monitor.alsa-midi = disabled
    monitor.bluez = disabled
    monitor.bluez-midi = disabled
    monitor.v4l2 = disabled
    monitor.libcamera = disabled
  }
}
EOF

pipewire >"$dir/pipewire.log" 2>&1 &
pw=$!
sleep 1
wireplumber >"$dir/wireplumber.log" 2>&1 &
wp=$!
pipewire-pulse >"$dir/pipewire-pulse.log" 2>&1 &
pp=$!

export PULSE_SERVER="unix:$dir/pulse/native"
# Everything is installed, so a sound server that does not come up is a
# failure, not a reason to skip: ctest counts skipped tests as passed.
i=0
until pactl info >/dev/null 2>&1; do
	i=$((i + 1))
	if ! kill -0 $pw 2>/dev/null || ! kill -0 $pp 2>/dev/null || [ $i -gt 200 ]; then
		echo "private pipewire-pulse did not start:"
		cat "$dir/pipewire.log" "$dir/wireplumber.log" "$dir/pipewire-pulse.log"
		kill $pp $wp $pw 2>/dev/null
		rm -rf "$dir"
		exit 1
	fi
	sleep 0.1
done

"$test_bin"
status=$?

kill $pp $wp $pw 2>/dev/null
wait $pp $wp $pw 2>/dev/null
rm -rf "$dir"
exit $status
