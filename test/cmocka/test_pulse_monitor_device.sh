#!/bin/sh
# Runs a PulseAudio end-to-end test (test_pulse_monitor_device,
# test_pulse_stream_restart) against a private PipeWire sound server:
# its own runtime dir, config and D-Bus, no hardware devices. Exits 77 (skip)
# when PipeWire, WirePlumber, pipewire-pulse, pactl or dbus-run-session are
# missing.
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

dir=$(mktemp -d "${TMPDIR:-/tmp}/obs-pw-test.XXXXXX") || exit 1
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
i=0
until pactl info >/dev/null 2>&1; do
	i=$((i + 1))
	if [ $i -gt 50 ]; then
		echo "private pipewire-pulse did not start: skipping"
		kill $pp $wp $pw 2>/dev/null
		rm -rf "$dir"
		exit 77
	fi
	sleep 0.1
done

"$test_bin"
status=$?

kill $pp $wp $pw 2>/dev/null
wait $pp $wp $pw 2>/dev/null
rm -rf "$dir"
exit $status
