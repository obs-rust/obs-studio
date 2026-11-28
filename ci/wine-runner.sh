#!/usr/bin/env bash
# Cargo runner for x86_64-pc-windows-msvc test binaries built on Linux (#5845).
# Wine never touches a desktop: DISPLAY and WAYLAND_DISPLAY are unset, Mono,
# Gecko, winedbg and winemenubuilder are disabled, the crash dialog and the
# AeDebug debugger are off (a test that traps on purpose fails with an exit
# code instead of attaching a debugger), and the prefix is created
# non-interactively.
set -euo pipefail
unset DISPLAY WAYLAND_DISPLAY
export WINEDEBUG="${WINEDEBUG:--all}" WINEDLLOVERRIDES="mscoree,mshtml=;winedbg.exe=d;winemenubuilder.exe=d"
export WINEPREFIX="${WINEPREFIX:-${HOME}/.cache/obs-wine-prefix}"
if [[ ! -f "${WINEPREFIX}/.obs-prefix-ready" ]]; then
  mkdir -p "${WINEPREFIX}"
  command -v wine >&2 || echo "wine-runner: wine not on PATH" >&2
  wine wineboot -u >&2 || echo "wine-runner: wineboot -u failed" >&2
  wine reg add "HKCU\\Software\\Wine\\WineDbg" /v ShowCrashDialog /t REG_DWORD /d 0 /f >&2 || true
  wine reg delete "HKLM\\Software\\Microsoft\\Windows NT\\CurrentVersion\\AeDebug" /v Debugger /f >&2 || true
  wineserver -w 2> /dev/null || true
  mkdir -p "${WINEPREFIX}" && touch "${WINEPREFIX}/.obs-prefix-ready"
fi
exec wine "$@"
