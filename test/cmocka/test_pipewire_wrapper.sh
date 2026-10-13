#!/bin/sh
# Checks the wrappers that start a private PipeWire daemon for the PipeWire
# and PulseAudio tests (test_pipewire_cursor_hdr.sh,
# test_pipewire_context_fail.sh, test_pulse_monitor_device.sh).
#
# They used to give up after waiting 5 s for the daemon's socket and report
# the test as skipped (77), which ctest counts as passed. With TMPDIR in a
# deep directory (a build tree, a worktree) the daemon's socket path is longer
# than a Unix socket allows (108 bytes): pipewire exits at once ("socket path
# ... exceeds 108 bytes"), and every PipeWire and PulseAudio test was
# silently skipped.
#
#   - a long TMPDIR: the daemon must still start and the test must run;
#   - a daemon that exits at once: a failure, not a skip;
#   - control: no pipewire binary at all (as in CI) still skips.
#
# The test binary is a stub that checks it can reach the daemon's socket.
# Skips (77) when pipewire is not installed.
# usage: test_pipewire_wrapper.sh <dir with the wrapper scripts>
scripts=$1

command -v pipewire >/dev/null 2>&1 || {
	echo "pipewire not found: skipping"
	exit 77
}

work=$(mktemp -d "/tmp/obs-pw-wrapper-test.XXXXXX") || exit 1
trap 'rm -rf "$work"' EXIT
failed=0

# Reports whether the daemon socket the wrapper set up is there: exit 0 if
# so, 3 if the test ran without a daemon (as the real tests then skip their
# cases and pass).
cat >"$work/stub" <<'STUB'
#!/bin/sh
for d in "$PIPEWIRE_RUNTIME_DIR" "$XDG_RUNTIME_DIR"; do
	[ -n "$d" ] && [ -S "$d/pipewire-0" ] && exit 0
done
echo "stub: no pipewire-0 socket in PIPEWIRE_RUNTIME_DIR/XDG_RUNTIME_DIR"
exit 3
STUB
chmod +x "$work/stub"

# A TMPDIR deep enough that <TMPDIR>/<wrapper dir>/pipewire-0-manager is over
# 108 bytes.
long="$work/a-build-tree-in-a-deeply-nested-worktree-directory/build-tmp"
mkdir -p "$long"

# A pipewire that exits at once, as one that cannot open its socket does.
mkdir -p "$work/bin-dying"
cat >"$work/bin-dying/pipewire" <<'PW'
#!/bin/sh
echo "fake pipewire: cannot start"
exit 1
PW
chmod +x "$work/bin-dying/pipewire"

# A PATH with the shell tools but no pipewire.
mkdir -p "$work/bin-none"
for t in sh mktemp mkdir rm sleep kill cat; do
	p=$(command -v $t) && ln -s "$p" "$work/bin-none/$t"
done

check() {
	name=$1
	want=$2
	got=$3
	if [ "$got" = "$want" ]; then
		echo "ok:   $name (exit $got)"
	else
		echo "FAIL: $name: exit $got, expected $want"
		failed=1
	fi
}

for wrapper in test_pipewire_cursor_hdr.sh test_pipewire_context_fail.sh; do
	TMPDIR="$long" timeout 60 sh "$scripts/$wrapper" "$work/stub" >"$work/out" 2>&1
	check "$wrapper, long TMPDIR: the daemon starts and the test runs" 0 $?

	PATH="$work/bin-dying:$PATH" timeout 60 sh "$scripts/$wrapper" "$work/stub" >"$work/out" 2>&1
	status=$?
	if [ $status -ne 0 ] && [ $status -ne 77 ] && [ $status -ne 3 ]; then
		echo "ok:   $wrapper, daemon exits at once: fails (exit $status)"
	else
		echo "FAIL: $wrapper, daemon exits at once: exit $status, expected the wrapper to fail (3: the test ran without a daemon)"
		failed=1
	fi
done

# The PulseAudio wrapper also needs WirePlumber, pipewire-pulse, pactl and
# dbus-run-session; its stub checks the private sound server answers.
if command -v wireplumber >/dev/null 2>&1 && command -v pipewire-pulse >/dev/null 2>&1 &&
	command -v pactl >/dev/null 2>&1 && command -v dbus-run-session >/dev/null 2>&1; then
	cat >"$work/pulse-stub" <<'STUB'
#!/bin/sh
pactl info >/dev/null 2>&1 && exit 0
echo "stub: the private sound server does not answer"
exit 3
STUB
	chmod +x "$work/pulse-stub"
	wrapper=test_pulse_monitor_device.sh

	TMPDIR="$long" timeout 60 sh "$scripts/$wrapper" "$work/pulse-stub" >"$work/out" 2>&1
	check "$wrapper, long TMPDIR: the sound server starts and the test runs" 0 $?

	PATH="$work/bin-dying:$PATH" timeout 60 sh "$scripts/$wrapper" "$work/pulse-stub" >"$work/out" 2>&1
	status=$?
	if [ $status -ne 0 ] && [ $status -ne 77 ] && [ $status -ne 3 ]; then
		echo "ok:   $wrapper, daemon exits at once: fails (exit $status)"
	else
		echo "FAIL: $wrapper, daemon exits at once: exit $status, expected the wrapper to fail"
		failed=1
	fi
else
	echo "skipped: test_pulse_monitor_device.sh checks (WirePlumber, pipewire-pulse, pactl or dbus-run-session missing)"
fi

# Control: no pipewire binary, as in CI.
PATH="$work/bin-none" sh "$scripts/test_pipewire_cursor_hdr.sh" "$work/stub" >"$work/out" 2>&1
check "test_pipewire_cursor_hdr.sh, no pipewire binary: skips" 77 $?

exit $failed
