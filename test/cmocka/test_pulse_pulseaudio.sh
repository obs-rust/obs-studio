#!/bin/sh
# Runs a PulseAudio end-to-end test against a private PulseAudio daemon (not
# pipewire-pulse): its own runtime dir, HOME and D-Bus, no hardware, session
# or Bluetooth modules, so there is no audio output; the test loads null sinks
# itself. test_pulse_monitor_device.sh is the same for PipeWire.
# The daemon is "pulseaudio" from PATH, or $OBS_TEST_PULSEAUDIO (e.g. a script
# that runs an unpacked build). Exits 77 (skip) when it, pactl or
# dbus-run-session are missing, and fails when they are installed but the
# daemon does not start.
# usage: test_pulse_pulseaudio.sh <test binary> [args]
if [ -z "$PRIVATE_PULSEAUDIO_STARTED" ]; then
	pulseaudio=${OBS_TEST_PULSEAUDIO:-pulseaudio}
	for tool in "$pulseaudio" pactl dbus-run-session; do
		command -v "$tool" >/dev/null 2>&1 || {
			echo "$tool not found: skipping (needs a PulseAudio daemon, not pipewire-pulse)"
			exit 77
		}
	done
	PRIVATE_PULSEAUDIO_STARTED=1 OBS_TEST_PULSEAUDIO=$pulseaudio exec dbus-run-session -- sh "$0" "$@"
fi
test_bin=$1
shift

# The socket path, <dir>/pulse/native, must fit a Unix socket address; with a
# deep TMPDIR (a build tree) it may not. Use /tmp then.
dir=$(mktemp -d "${TMPDIR:-/tmp}/obs-pa-test.XXXXXX") || exit 1
if [ $((${#dir} + 13)) -gt 107 ]; then
	rm -rf "$dir"
	dir=$(mktemp -d "/tmp/obs-pa-test.XXXXXX") || exit 1
fi
chmod 700 "$dir"
export XDG_RUNTIME_DIR="$dir" PULSE_RUNTIME_PATH="$dir/pulse" PULSE_STATE_PATH="$dir/state"
export XDG_CONFIG_HOME="$dir/config" XDG_STATE_HOME="$dir/state" HOME="$dir/home"
mkdir -p "$XDG_CONFIG_HOME" "$XDG_STATE_HOME" "$HOME"

# Only the native protocol. Moving and rescuing streams is done by the core
# (PulseAudio 15 and later), not by a module.
echo "load-module module-native-protocol-unix" >"$dir/test.pa"
"$OBS_TEST_PULSEAUDIO" -n -F "$dir/test.pa" --daemonize=no --exit-idle-time=-1 --use-pid-file=no \
	--system=no --disallow-exit --log-target=stderr >"$dir/pulseaudio.log" 2>&1 &
pa=$!

export PULSE_SERVER="unix:$dir/pulse/native"
# Everything is installed, so a daemon that does not come up is a failure,
# not a reason to skip: ctest counts skipped tests as passed.
i=0
until pactl info >/dev/null 2>&1; do
	i=$((i + 1))
	if ! kill -0 $pa 2>/dev/null || [ $i -gt 200 ]; then
		echo "private pulseaudio did not start:"
		cat "$dir/pulseaudio.log"
		kill $pa 2>/dev/null
		wait $pa 2>/dev/null
		rm -rf "$dir"
		exit 1
	fi
	sleep 0.1
done

"$test_bin" "$@"
status=$?

kill $pa 2>/dev/null
wait $pa 2>/dev/null
rm -rf "$dir"
exit $status
