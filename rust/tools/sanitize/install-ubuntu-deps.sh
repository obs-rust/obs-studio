#!/bin/bash
# Installs what sanitize.sh needs on Ubuntu 26.04 (the CI runner): the OBS
# build dependencies from the shared Tier 2 package list
# (rust/tools/linux-validate/apt-packages.txt, which the Tier 2 CI jobs and
# the Docker harness install too, so the lists can't drift) plus the tools
# for a headless launch.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
list=$here/../linux-validate/apt-packages.txt
pkgs=$(grep -v '^#' "$list" | tr -s ' \n' ' ')
[ -n "${pkgs// /}" ] || { echo "no packages found in $list" >&2; exit 1; }
sudo=; [ "$(id -u)" = 0 ] || sudo=sudo
$sudo apt-get update
# shellcheck disable=SC2086 # word-splitting the package list is intended
$sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends $pkgs \
  valgrind gdb xvfb xauth libgl1-mesa-dri dbus-daemon dbus-x11 qt6-image-formats-plugins
