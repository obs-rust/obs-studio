#!/usr/bin/env -S uv run --script
"""Installs what sanitize.py needs on Ubuntu 26.04 (the CI runner): the OBS
build dependencies from the shared Tier 2 package list
(rust/tools/linux-validate/apt-packages.txt, which the Tier 2 CI jobs and the
Docker harness install too, so the lists can't drift) plus the tools for a
headless launch.

    uv run --no-project rust/tools/sanitize/install_ubuntu_deps.py

NixOS does not need this: nix-shell rust/tools/sanitize/shell.nix carries the
same packages. Ported off the 16-line shell script in #962; sanitizers.yaml
runs it through `uv run --no-project`. Tested by ci/test_sanitize_deps.py.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
LIST = HERE.parent / "linux-validate" / "apt-packages.txt"
# What a headless launch and valgrind need on top of the build dependencies.
EXTRA = (
    "valgrind",
    "gdb",
    "xvfb",
    "xauth",
    "libgl1-mesa-dri",
    "dbus-daemon",
    "dbus-x11",
    "qt6-image-formats-plugins",
)


def packages(list_path: Path) -> list[str]:
    """The shared package list, comments and blank lines dropped."""
    lines = list_path.read_text(encoding="utf-8").splitlines()
    names = " ".join(line for line in lines if not line.startswith("#")).split()
    if not names:
        sys.stderr.write(f"no packages found in {list_path}\n")
        raise SystemExit(1)
    return names


def run(*args: str) -> None:
    """Run a command, through sudo unless this already runs as root."""
    sudo = [] if os.geteuid() == 0 else ["sudo"]
    subprocess.run([*sudo, *args], check=True)


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        sys.stderr.write("usage: install_ubuntu_deps.py\n")
        return 2
    names = packages(LIST)
    run("apt-get", "update")
    # DEBIAN_FRONTEND=noninteractive through env: apt-get reads it from the
    # environment, and sudo would drop an inline assignment.
    run(
        "env",
        "DEBIAN_FRONTEND=noninteractive",
        "apt-get",
        "install",
        "-y",
        "--no-install-recommends",
        *names,
        *EXTRA,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
