#!/usr/bin/env bash
# Install Wine for the headless Windows lanes WITHOUT touching the root filesystem (#5867).
#
# The Dell pool lanes have a small root filesystem: `apt-get install wine` died with
# "No space left on device" while unpacking libwine. Download the debs (and the
# dependencies the image lacks) into a workspace directory, unpack them with
# `dpkg -x` into a workspace-local root, and let ci/wine-runner.sh run Wine from
# there. Run after `apt-get update`. Exports OBS_WINE_ROOT, WINESERVER and
# LD_LIBRARY_PATH to later steps through $GITHUB_ENV.
set -euo pipefail
root="${OBS_WINE_ROOT:-${RUNNER_TEMP:-${PWD}/target}/obs-wine-root}"
debs="${root}.debs"
log_fs() {
  for p in / "${GITHUB_WORKSPACE:-${PWD}}" "${root}" "${debs}"; do
    [[ -e "${p}" ]] && df -T "${p}" | tail -1 | sed "s|^|install-wine: df ${1}: |"
  done
}
mkdir -p "${root}" "${debs}/partial"
log_fs before
if [[ ! -f "${root}/.obs-wine-ready" ]]; then
  # Download only: apt writes the debs to ${debs}, not /var/cache/apt/archives.
  sudo apt-get -o Acquire::Retries=3 -o "Dir::Cache::archives=${debs}" install --download-only -y \
    --no-install-recommends wine wine64 libwine
  sudo chown -R "$(id -u):$(id -g)" "${debs}"
  for deb in "${debs}"/*.deb; do
    dpkg -x "${deb}" "${root}"
    rm -f "${deb}" # keep the footprint to one copy
  done
  touch "${root}/.obs-wine-ready"
fi
loader=""
for c in "${root}/usr/lib/wine/wine64" "${root}/usr/lib/wine/wine"; do
  [[ -x "${c}" ]] && loader="${c}" && break
done
[[ -n "${loader}" ]] || { echo "install-wine: no wine loader under ${root}/usr/lib/wine" >&2; exit 1; }
server="${root}/usr/lib/wine/wineserver64"
[[ -x "${server}" ]] || server="${root}/usr/lib/wine/wineserver"
libs="${root}/usr/lib/x86_64-linux-gnu:${root}/lib/x86_64-linux-gnu"
log_fs after
echo "install-wine: loader=${loader} server=${server}"
if [[ -n "${GITHUB_ENV:-}" ]]; then
  {
    echo "OBS_WINE_ROOT=${root}"
    echo "WINESERVER=${server}"
    echo "LD_LIBRARY_PATH=${libs}${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
  } >> "${GITHUB_ENV}"
fi
