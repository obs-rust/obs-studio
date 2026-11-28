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
# Wine unpacked by ci/install-wine.sh into a workspace-local root (the Dell root
# filesystem is too small for apt to install it, #5867); else the system wine.
wine_root="${OBS_WINE_ROOT:-${RUNNER_TEMP:+${RUNNER_TEMP}/obs-wine-root}}"
if [[ -n "${wine_root}" && -f "${wine_root}/.obs-wine-ready" ]]; then
  export LD_LIBRARY_PATH="${wine_root}/usr/lib/x86_64-linux-gnu:${wine_root}/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
  export WINEDLLPATH="${wine_root}/usr/lib/x86_64-linux-gnu/wine"
  for c in "${wine_root}/usr/lib/wine/wine64" "${wine_root}/usr/lib/wine/wine"; do
    [[ -x "$c" ]] && export WINELOADER="$c" && break
  done
  for c in "${wine_root}/usr/lib/wine/wineserver64" "${wine_root}/usr/lib/wine/wineserver"; do
    [[ -x "$c" ]] && export WINESERVER="$c" && break
  done
  wine() { "${WINELOADER}" "$@"; }
  wineserver() { "${WINESERVER}" "$@"; }
fi
# The prefix holds Windows' TEMP (drive_c/users/<user>/Temp). Keep it under
# $RUNNER_TEMP, beside the workspace, not in $HOME or /tmp (#5867; a harmless
# default on hosted runners).
if [[ -z "${WINEPREFIX:-}" ]]; then
  if [[ -n "${RUNNER_TEMP:-}" ]]; then
    WINEPREFIX="${RUNNER_TEMP}/obs-wine-prefix"
  else
    WINEPREFIX="${HOME}/.cache/obs-wine-prefix"
  fi
fi
export WINEPREFIX
if [[ ! -f "${WINEPREFIX}/.obs-prefix-ready" ]]; then
  mkdir -p "${WINEPREFIX}"
  command -v wine >&2 || echo "wine-runner: wine not on PATH" >&2
  wine wineboot -u >&2 || echo "wine-runner: wineboot -u failed" >&2
  # Pin TEMP/TMP to a directory beside the prefix (same filesystem as it).
  wine_temp="${WINEPREFIX}/obs-temp"
  mkdir -p "${wine_temp}"
  wine_temp_win="Z:${wine_temp//\//\\}"
  for name in TEMP TMP; do
    wine reg add "HKCU\\Environment" /v "${name}" /t REG_SZ /d "${wine_temp_win}" /f >&2 || true
  done
  # Evidence for #5867: filesystem type of the prefix, temp, workspace and HOME.
  for label_path in "prefix:${WINEPREFIX}" "temp:${wine_temp}" "workspace:${GITHUB_WORKSPACE:-${PWD}}" \
    "runner_temp:${RUNNER_TEMP:-unset}" "home:${HOME}" "tmp:/tmp" "root:/"; do
    label="${label_path%%:*}"
    path="${label_path#*:}"
    [[ -e "${path}" ]] || continue
    echo "wine-runner: fs ${label}=${path} type=$(stat -f -c %T "${path}" 2> /dev/null || echo ?) dev=$(stat -c %d "${path}" 2> /dev/null || echo ?) $(df -T "${path}" 2> /dev/null | tail -1)" >&2
  done
  echo "wine-runner: wine TEMP=$(wine cmd /c "echo %TEMP%" 2> /dev/null | tr -d '\r')" >&2
  wine reg add "HKCU\\Software\\Wine\\WineDbg" /v ShowCrashDialog /t REG_DWORD /d 0 /f >&2 || true
  wine reg delete "HKLM\\Software\\Microsoft\\Windows NT\\CurrentVersion\\AeDebug" /v Debugger /f >&2 || true
  wineserver -w 2> /dev/null || true
  mkdir -p "${WINEPREFIX}" && touch "${WINEPREFIX}/.obs-prefix-ready"
fi
exec "${WINELOADER:-wine}" "$@"
