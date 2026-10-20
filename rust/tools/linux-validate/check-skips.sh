#!/bin/bash
# Usage: check-skips.sh CTEST_LOG
# Fails when a Tier 2 test that must run was Skipped (obs-rust/obs-studio#776).
# The harness image once had no PipeWire daemon, dbus-daemon or X server, so
# those tests exited 77 and ctest reported them Skipped, which counted as a
# pass. Tests matching MUST_RUN may not skip. Tested by ci/test_tier2_skips.py.
set -euo pipefail
[ $# -eq 1 ] || { echo "usage: check-skips.sh CTEST_LOG" >&2; exit 2; }
MUST_RUN='^(test_pipewire_|test_vlc_|test_screencast_portal_|test_inhibit_sleep)'
bad=$(sed -nE 's/^.*Test +#[0-9]+: +([^ ]+) +\.+ *(\*\*\*)?Skipped.*$/\1/p' "$1" | grep -E "$MUST_RUN" | sort -u || true)
if [ -n "$bad" ]; then
  echo "FAIL: Tier 2 skipped tests that must run (obs-rust/obs-studio#776):"
  sed 's/^/  /' <<<"$bad"
  echo "The harness image must provide pipewire, dbus-daemon, xvfb and xauth, and ctest must run under xvfb-run."
  exit 1
fi
